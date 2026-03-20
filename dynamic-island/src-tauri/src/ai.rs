use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use crate::{IslandState, shared_http_client};
use crate::settings::{build_settings_data, save_settings_to_file};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

#[tauri::command]
pub fn ai_get_settings(state: tauri::State<'_, IslandState>) -> serde_json::Value {
    let api_url = state.ai_api_url.lock().unwrap().clone();
    let api_key = state.ai_api_key.lock().unwrap().clone();
    let model = state.ai_model.lock().unwrap().clone();
    let is_reasoning = state.is_reasoning_model.load(Ordering::Relaxed);

    // 掩码 API Key，只显示前 4 位和后 4 位
    let masked_key = if api_key.len() > 8 {
        format!("{}...{}", &api_key[..4], &api_key[api_key.len()-4..])
    } else if !api_key.is_empty() {
        "****".to_string()
    } else {
        String::new()
    };

    serde_json::json!({
        "api_url": api_url,
        "api_key": masked_key,
        "model": model,
        "is_reasoning_model": is_reasoning
    })
}

#[tauri::command]
pub fn ai_save_settings(
    state: tauri::State<'_, IslandState>,
    api_url: String,
    api_key: String,
    model: String,
) -> Result<(), String> {
    *state.ai_api_url.lock().unwrap() = api_url.clone();
    *state.ai_api_key.lock().unwrap() = api_key.clone();
    *state.ai_model.lock().unwrap() = model.clone();

    // 检查是否已配置
    let enabled = !api_url.is_empty() && !api_key.is_empty() && !model.is_empty();
    state.ai_enabled.store(enabled, Ordering::Relaxed);

    // 持久化到文件
    let mut settings_data = build_settings_data(&state);
    settings_data.ai_api_url = api_url;
    settings_data.ai_api_key = api_key;
    settings_data.ai_model = model;

    save_settings_to_file(&settings_data)?;
    Ok(())
}

#[tauri::command]
pub fn ai_detect_model_type(state: tauri::State<'_, IslandState>) -> Result<serde_json::Value, String> {
    let api_url = state.ai_api_url.lock().unwrap().clone();
    let api_key = state.ai_api_key.lock().unwrap().clone();
    let model = state.ai_model.lock().unwrap().clone();

    if api_url.is_empty() || api_key.is_empty() || model.is_empty() {
        return Err("AI 配置不完整".to_string());
    }

    // 构建测试请求
    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": "Hi"
            }
        ],
        "stream": false,
        "max_tokens": 10
    });

    let client = shared_http_client();
    let url = if api_url.ends_with("/chat/completions") {
        api_url.clone()
    } else if api_url.ends_with('/') {
        format!("{}chat/completions", api_url)
    } else {
        format!("{}/chat/completions", api_url)
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().unwrap_or_default();
        return Err(format!("API 返回错误 {}: {}", status, error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .map_err(|e| format!("解析响应失败: {}", e))?;

    // 检测是否为思考模型
    let is_reasoning = response_json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| {
            // 检查是否有 reasoning_content 或 thinking 字段
            if m.get("reasoning_content").is_some() {
                Some(true)
            } else if m.get("thinking").is_some() {
                Some(true)
            } else {
                None
            }
        })
        .unwrap_or(false);

    // 更新状态
    state.is_reasoning_model.store(is_reasoning, Ordering::Relaxed);

    // 持久化
    let mut settings_data = build_settings_data(&state);
    settings_data.is_reasoning_model = is_reasoning;
    save_settings_to_file(&settings_data)?;

    Ok(serde_json::json!({
        "is_reasoning_model": is_reasoning
    }))
}

#[tauri::command]
pub fn ai_stop_generation(state: tauri::State<'_, IslandState>) {
    state.ai_generating.store(false, Ordering::Relaxed);
}

#[tauri::command]
pub fn ai_clear_history(state: tauri::State<'_, IslandState>) {
    state.ai_history.lock().unwrap().clear();
}

#[tauri::command]
pub fn ai_send_message(
    app: tauri::AppHandle,
    state: tauri::State<'_, IslandState>,
    content: String,
) -> Result<(), String> {
    let api_url = state.ai_api_url.lock().unwrap().clone();
    let api_key = state.ai_api_key.lock().unwrap().clone();
    let model = state.ai_model.lock().unwrap().clone();
    let is_reasoning = state.is_reasoning_model.load(Ordering::Relaxed);

    if api_url.is_empty() || api_key.is_empty() || model.is_empty() {
        return Err("AI 配置不完整".to_string());
    }

    // 添加用户消息到历史
    {
        let mut history = state.ai_history.lock().unwrap();
        history.push(ChatMessage {
            role: "user".to_string(),
            content: content.clone(),
            reasoning_content: None,
        });

        // 限制历史长度为最近 20 轮对话（40 条消息）
        if history.len() > 40 {
            let excess = history.len() - 40;
            history.drain(0..excess);
        }
    }

    // 设置生成状态
    state.ai_generating.store(true, Ordering::Relaxed);

    // 在新线程中执行流式请求
    let ai_history = state.ai_history.clone();
    let ai_generating = state.ai_generating.clone();

    thread::spawn(move || {
        // 发送开始状态
        let window = if let Some(win) = app.get_webview_window("main") {
            win
        } else {
            return;
        };

        let _ = window.emit("ai-status", serde_json::json!({
            "status": if is_reasoning { "thinking" } else { "generating" }
        }));

        // 构建请求
        let messages: Vec<serde_json::Value> = {
            let history = ai_history.lock().unwrap();
            history.iter().map(|msg| {
                let mut obj = serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                });
                if let Some(ref reasoning) = msg.reasoning_content {
                    obj["reasoning_content"] = serde_json::Value::String(reasoning.clone());
                }
                obj
            }).collect()
        };

        let request_body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true
        });

        let url = if api_url.ends_with("/chat/completions") {
            api_url.clone()
        } else if api_url.ends_with("/v1") || api_url.ends_with("/v1/") {
            let base = api_url.trim_end_matches('/');
            format!("{}/chat/completions", base)
        } else if api_url.ends_with('/') {
            format!("{}v1/chat/completions", api_url)
        } else {
            format!("{}/v1/chat/completions", api_url)
        };

        println!("[AI] Requesting URL: {}", url);
        println!("[AI] Model: {}, Messages: {}", model, messages.len());

        // 发起流式请求 — 不设置总超时，只设置连接超时
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .unwrap();

        let response = match client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
        {
            Ok(resp) => resp,
            Err(e) => {
                println!("[AI] Request failed: {}", e);
                let _ = window.emit("ai-status", serde_json::json!({
                    "status": "error",
                    "error": format!("请求失败: {}", e)
                }));
                ai_generating.store(false, Ordering::Relaxed);
                return;
            }
        };

        println!("[AI] Response status: {}", response.status());

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            println!("[AI] API error {}: {}", status, error_text);
            let _ = window.emit("ai-status", serde_json::json!({
                "status": "error",
                "error": format!("API 返回错误 {}: {}", status, error_text)
            }));
            ai_generating.store(false, Ordering::Relaxed);
            return;
        }

        // 解析 SSE 流
        let mut assistant_content = String::new();
        let mut reasoning_content = String::new();
        let mut in_thinking_phase = is_reasoning;

        use std::io::BufRead;
        let reader = std::io::BufReader::new(response);

        for line in reader.lines() {
            // 检查是否被停止
            if !ai_generating.load(Ordering::Relaxed) {
                break;
            }

            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    let _ = window.emit("ai-status", serde_json::json!({
                        "status": "error",
                        "error": format!("读取响应失败: {}", e)
                    }));
                    break;
                }
            };

            let line = line.trim();
            if line.is_empty() || line == "data: [DONE]" {
                continue;
            }

            if !line.starts_with("data: ") {
                continue;
            }

            let json_str = &line[6..];
            let chunk: serde_json::Value = match serde_json::from_str(json_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // 提取 delta
            if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
                if let Some(choice) = choices.get(0) {
                    if let Some(delta) = choice.get("delta") {
                        // 检查思考内容
                        if let Some(reasoning) = delta.get("reasoning_content").and_then(|r| r.as_str()) {
                            reasoning_content.push_str(reasoning);
                            let _ = window.emit("ai-thinking-token", serde_json::json!({
                                "text": reasoning
                            }));
                        }

                        // 检查普通内容
                        if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                            // 如果之前在思考阶段，现在切换到生成阶段
                            if in_thinking_phase && !content.is_empty() {
                                in_thinking_phase = false;
                                let _ = window.emit("ai-status", serde_json::json!({
                                    "status": "generating"
                                }));
                            }

                            assistant_content.push_str(content);
                            let _ = window.emit("ai-token", serde_json::json!({
                                "text": content
                            }));
                        }
                    }
                }
            }
        }

        // 保存完整的 assistant 回复到历史
        if !assistant_content.is_empty() || !reasoning_content.is_empty() {
            let mut history = ai_history.lock().unwrap();
            history.push(ChatMessage {
                role: "assistant".to_string(),
                content: assistant_content,
                reasoning_content: if reasoning_content.is_empty() {
                    None
                } else {
                    Some(reasoning_content)
                },
            });

            // 限制历史长度
            if history.len() > 40 {
                let excess = history.len() - 40;
                history.drain(0..excess);
            }
        }

        // 发送完成状态
        let _ = window.emit("ai-status", serde_json::json!({
            "status": "completed"
        }));

        ai_generating.store(false, Ordering::Relaxed);
    });

    Ok(())
}
