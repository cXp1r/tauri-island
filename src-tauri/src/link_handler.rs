use std::path::Path;
use std::process::Command;
use std::os::windows::process::CommandExt;
use serde::{Deserialize, Serialize};
use crate::{IslandState, CREATE_NO_WINDOW};

/// 链接处理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkHandler {
    /// 处理器唯一标识
    pub id: String,
    /// 处理器名称（显示用，如"夸克网盘"）
    pub name: String,
    /// URL 匹配正则表达式
    pub pattern: String,
    /// 应用启动路径
    pub app_path: String,
    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool { true }

impl LinkHandler {
    /// 检查 URL 是否匹配此处理器
    pub fn matches(&self, url: &str) -> bool {
        if !self.enabled || self.app_path.is_empty() {
            return false;
        }
        match regex::Regex::new(&self.pattern) {
            Ok(re) => re.is_match(url),
            Err(_) => false,
        }
    }
}

pub(crate) fn get_default_link_handlers() -> Vec<LinkHandler> {
    vec![
        LinkHandler {
            id: "quark-default".to_string(),
            name: "夸克网盘".to_string(),
            pattern: r"https://pan\.quark\.cn/s/[a-zA-Z0-9]+".to_string(),
            app_path: String::new(),
            enabled: false,
        },
    ]
}

/// 在所有处理器中查找匹配项
pub(crate) fn find_matching_handler(url: &str, handlers: &[LinkHandler]) -> Option<LinkHandler> {
    handlers.iter().find(|h| h.matches(url)).cloned()
}

/// 验证 URL 是否安全（协议白名单 + 可选域名白名单）
pub(crate) fn validate_url(url: &str, allowed_domains: &[String]) -> Result<String, String> {
    // 解析 URL
    let parsed = url::Url::parse(url)
        .map_err(|e| format!("无效的 URL: {}", e))?;

    // 协议白名单
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => return Err(format!("不允许的协议: {}", scheme)),
    }

    // 必须有 host
    let host = parsed.host_str()
        .ok_or_else(|| "URL 缺少主机名".to_string())?;

    // 可选域名白名单
    if !allowed_domains.is_empty() {
        let host_lower = host.to_lowercase();
        let is_allowed = allowed_domains.iter().any(|domain| {
            let d = domain.to_lowercase();
            host_lower == d || host_lower.ends_with(&format!(".{}", d))
        });
        if !is_allowed {
            return Err(format!("域名 {} 不在白名单中", host));
        }
    }

    Ok(url.to_string())
}

/// 解析 .lnk 快捷方式获取目标路径
fn resolve_lnk_target(lnk_path: &Path) -> Result<std::path::PathBuf, String> {
    let lnk_path_str = lnk_path.to_string_lossy();
    let ps_script = format!(
        r#"(New-Object -ComObject WScript.Shell).CreateShortcut('{}').TargetPath"#,
        lnk_path_str.replace("'", "''")
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("解析快捷方式失败: {}", e))?;

    if !output.status.success() {
        return Err("无法解析快捷方式".to_string());
    }

    let target = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if target.is_empty() {
        return Err("快捷方式目标路径为空".to_string());
    }

    Ok(std::path::PathBuf::from(target))
}

/// 启动应用并传递 URL（通用函数）
fn launch_app_with_url(app_path: &str, url: &str) -> Result<(), String> {
    let app_path_buf = Path::new(app_path);

    // 安全检查1: 必须是绝对路径
    if !app_path_buf.is_absolute() {
        return Err("应用路径必须是绝对路径".to_string());
    }

    // 安全检查2: 路径规范化，防止路径遍历攻击
    let canonical_path = app_path_buf
        .canonicalize()
        .map_err(|e| format!("路径规范化失败: {}", e))?;

    // 安全检查3: 确保路径存在
    if !canonical_path.exists() {
        return Err(format!("应用不存在: {}", canonical_path.display()));
    }

    // 获取最终的可执行文件路径（处理 .lnk 快捷方式）
    let final_path = if canonical_path
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("lnk"))
        .unwrap_or(false)
    {
        // 解析 .lnk 快捷方式获取目标路径
        resolve_lnk_target(&canonical_path)?
    } else {
        // 非 .lnk 文件，验证必须是 .exe
        let is_valid_executable = canonical_path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("exe"))
            .unwrap_or(false);
        if !is_valid_executable {
            return Err("仅支持 .exe 可执行文件或 .lnk 快捷方式".to_string());
        }
        canonical_path
    };

    // 安全检查4: 最终路径必须是 .exe
    if final_path
        .extension()
        .map(|ext| !ext.eq_ignore_ascii_case("exe"))
        .unwrap_or(true)
    {
        return Err("快捷方式目标必须是 .exe 可执行文件".to_string());
    }

    // 启动应用
    let mut cmd = Command::new(&final_path);
    if !url.is_empty() {
        cmd.arg(url);
    }
    cmd.creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("启动应用失败: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn open_url(url: String) {
    // 默认允许所有 http/https URL（无域名白名单）
    match validate_url(&url, &[]) {
        Ok(valid_url) => {
            let _ = open::that(&valid_url);
        }
        Err(e) => {
            eprintln!("[open_url] URL 验证失败: {}", e);
        }
    }
}

#[tauri::command]
pub fn open_url_with_whitelist(
    state: tauri::State<'_, IslandState>,
    url: String,
) -> Result<(), String> {
    let allowed_domains = state.url_whitelist.lock().unwrap().clone();
    let valid_url = validate_url(&url, &allowed_domains)?;
    let _ = open::that(&valid_url);
    Ok(())
}

#[tauri::command]
pub fn open_link_with_handler(
    state: tauri::State<'_, IslandState>,
    url: String,
) -> Result<(), String> {
    let handlers = state.link_handlers.lock().unwrap();

    if let Some(handler) = find_matching_handler(&url, &handlers) {
        launch_app_with_url(&handler.app_path, &url)
    } else {
        // 没有匹配的处理器，使用系统默认方式打开
        let _ = open::that(&url);
        Ok(())
    }
}

/// 测试链接处理器（启动应用不传URL）
#[tauri::command]
pub fn test_link_handler(app_path: String) -> Result<(), String> {
    launch_app_with_url(&app_path, "")
}
