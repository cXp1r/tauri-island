use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use crate::IslandState;
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
    *state.ai_model.lock().unwrap() = model.clone();

    // 如果前端传回的是掩码 key（包含 "..."），不覆盖真实 key
    let real_key = if api_key.contains("...") || api_key == "****" {
        state.ai_api_key.lock().unwrap().clone()
    } else {
        *state.ai_api_key.lock().unwrap() = api_key.clone();
        api_key.clone()
    };

    // 检查是否已配置
    let enabled = !api_url.is_empty() && !real_key.is_empty() && !model.is_empty();
    state.ai_enabled.store(enabled, Ordering::Relaxed);

    // 持久化到文件
    let mut settings_data = build_settings_data(&state);
    settings_data.ai_api_url = api_url;
    settings_data.ai_api_key = real_key;
    settings_data.ai_model = model;

    save_settings_to_file(&settings_data)?;
    Ok(())
}

/// 模型名称启发式检测：已知的思考模型关键字
fn is_reasoning_model_by_name(model: &str) -> bool {
    let lower = model.to_lowercase();
    // DeepSeek R1 系列
    if lower.contains("deepseek-r1") || lower.contains("deepseek_r1") || lower.contains("deepseek-reasoner") {
        return true;
    }
    // OpenAI o 系列
    if lower.contains("o1") || lower.contains("o3") || lower.contains("o4") {
        // 排除 "model-v01" 之类的误匹配
        for pat in ["o1-", "o1", "-o1", "o3-", "o3", "-o3", "o4-", "o4", "-o4"] {
            if lower.contains(pat) { return true; }
        }
    }
    // QwQ 系列
    if lower.contains("qwq") {
        return true;
    }
    false
}

#[tauri::command]
pub fn ai_detect_model_type(
    app: tauri::AppHandle,
    state: tauri::State<'_, IslandState>,
) -> Result<(), String> {
    let api_url = state.ai_api_url.lock().unwrap().clone();
    let api_key = state.ai_api_key.lock().unwrap().clone();
    let model = state.ai_model.lock().unwrap().clone();
    let is_reasoning_model = state.is_reasoning_model.clone();

    if api_url.is_empty() || api_key.is_empty() || model.is_empty() {
        return Err("AI 配置不完整".to_string());
    }

    // 在新线程中执行检测，不阻塞 command
    thread::spawn(move || {
        let name_detected = is_reasoning_model_by_name(&model);

        let request_body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "Hi"}],
            "stream": false,
            "max_tokens": 10
        });

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

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

        println!("[AI Detect] URL: {}, Model: {}", url, model);

        let is_reasoning = match client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
        {
            Ok(response) => {
                if !response.status().is_success() {
                    name_detected
                } else {
                    let response_json: serde_json::Value = response.json().unwrap_or(serde_json::Value::Null);
                    let api_detected = response_json
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("message"))
                        .map(|m| {
                            let has_reasoning = m.get("reasoning_content")
                                .map(|v| !v.is_null() && v.as_str().map(|s| !s.is_empty()).unwrap_or(true))
                                .unwrap_or(false);
                            let has_thinking = m.get("thinking")
                                .map(|v| !v.is_null())
                                .unwrap_or(false);
                            has_reasoning || has_thinking
                        })
                        .unwrap_or(false);
                    api_detected || name_detected
                }
            }
            Err(_) => name_detected,
        };

        is_reasoning_model.store(is_reasoning, Ordering::Relaxed);
        println!("[AI Detect] Final result: is_reasoning={}", is_reasoning);

        // 通过事件通知前端（settings 窗口）
        if let Some(win) = app.get_webview_window("settings") {
            let _ = win.emit("ai-model-type-detected", serde_json::json!({
                "is_reasoning_model": is_reasoning
            }));
        }

        // 持久化（在线程内完成）
        // 注意：无法在线程中访问 tauri::State，所以使用 settings 模块的直接 IO
        let mut settings_data = crate::settings::load_settings_from_file();
        settings_data.is_reasoning_model = is_reasoning;
        let _ = crate::settings::save_settings_to_file(&settings_data);
    });

    Ok(())
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

        // 已知思考模型先进入 thinking，否则进入 generating（稍后流中自动检测）
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
        let mut ever_had_reasoning = false; // 流中是否出现过 reasoning_content

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
                            if !reasoning.is_empty() {
                                // 自动检测：首次收到 reasoning_content 时，切换到 thinking 状态
                                if !ever_had_reasoning {
                                    ever_had_reasoning = true;
                                    in_thinking_phase = true;
                                    let _ = window.emit("ai-status", serde_json::json!({
                                        "status": "thinking"
                                    }));
                                }
                                reasoning_content.push_str(reasoning);
                                let _ = window.emit("ai-thinking-token", serde_json::json!({
                                    "text": reasoning
                                }));
                            }
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
