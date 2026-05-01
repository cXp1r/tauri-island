use std::path::PathBuf;
use tokio::task;
use crate::logger;

const TAG: &str = "Email";

/// 邮件缓存目录：config_dir/dynamic-island/email
pub fn email_cache_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("dynamic-island")
        .join("email");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[derive(Clone)]
pub struct Email {
    pub username: String,
    pub auth: String,
    pub address: String,
    pub port: u16,
}

impl Email {
    pub fn is_configured(&self) -> bool {
        let ok = !self.username.trim().is_empty()
            && !self.auth.trim().is_empty()
            && !self.address.trim().is_empty()
            && self.port > 0;
        logger::debug(TAG, &format!("is_configured = {} (user={}, addr={}, port={})",
            ok, self.username, self.address, self.port));
        ok
    }

    pub async fn get_latest_uid(&self) -> Option<String> {
        let config = self.clone_email();
        logger::debug(TAG, "get_latest_uid: start");

        task::spawn_blocking(move || {
            if !config.is_configured() {
                logger::debug(TAG, "get_latest_uid: not configured, skip");
                return None;
            }

            logger::debug(TAG, &format!("get_latest_uid: building TLS connector"));
            let tls = match native_tls::TlsConnector::builder().build() {
                Ok(t) => t,
                Err(e) => {
                    logger::debug(TAG, &format!("get_latest_uid: TLS build failed: {e}"));
                    return None;
                }
            };

            logger::debug(TAG, &format!("get_latest_uid: connecting to {}:{}", config.address, config.port));
            let client = match imap::connect(
                (config.address.as_str(), config.port),
                config.address.as_str(),
                &tls,
            ) {
                Ok(c) => c,
                Err(e) => {
                    logger::debug(TAG, &format!("get_latest_uid: connect failed: {e}"));
                    return None;
                }
            };

            logger::debug(TAG, "get_latest_uid: logging in");
            let mut session = match client.login(&config.username, &config.auth) {
                Ok(s) => s,
                Err((e, _)) => {
                    logger::debug(TAG, &format!("get_latest_uid: login failed: {e}"));
                    return None;
                }
            };

            logger::debug(TAG, "get_latest_uid: selecting INBOX");
            if let Err(e) = session.select("INBOX") {
                logger::debug(TAG, &format!("get_latest_uid: select INBOX failed: {e}"));
                session.logout().ok();
                return None;
            }

            logger::debug(TAG, "get_latest_uid: uid_search ALL");
            let messages = match session.uid_search("ALL") {
                Ok(m) => m,
                Err(e) => {
                    logger::debug(TAG, &format!("get_latest_uid: uid_search failed: {e}"));
                    session.logout().ok();
                    return None;
                }
            };

            let max_uid = messages.into_iter().max();

            session.logout().ok();

            max_uid.map(|u| u.to_string())
        })
        .await
        .ok()
        .flatten()
    }

    pub async fn get_latest_email(&self) -> Option<String> {
        let config = self.clone_email();
        logger::debug(TAG, "get_latest_email: start");

        task::spawn_blocking(move || {
            if !config.is_configured() {
                logger::debug(TAG, "get_latest_email: not configured, skip");
                return None;
            }

            logger::debug(TAG, "get_latest_email: building TLS connector");
            let tls = match native_tls::TlsConnector::builder().build() {
                Ok(t) => t,
                Err(e) => {
                    logger::debug(TAG, &format!("get_latest_email: TLS build failed: {e}"));
                    return None;
                }
            };

            logger::debug(TAG, &format!("get_latest_email: connecting to {}:{}", config.address, config.port));
            let client = match imap::connect(
                (config.address.as_str(), config.port),
                config.address.as_str(),
                &tls,
            ) {
                Ok(c) => c,
                Err(e) => {
                    logger::debug(TAG, &format!("get_latest_email: connect failed: {e}"));
                    return None;
                }
            };

            logger::debug(TAG, "get_latest_email: logging in");
            let mut session = match client.login(&config.username, &config.auth) {
                Ok(s) => s,
                Err((e, _)) => {
                    logger::debug(TAG, &format!("get_latest_email: login failed: {e}"));
                    return None;
                }
            };

            logger::debug(TAG, "get_latest_email: selecting INBOX");
            if let Err(e) = session.select("INBOX") {
                logger::debug(TAG, &format!("get_latest_email: select INBOX failed: {e}"));
                session.logout().ok();
                return None;
            }

            logger::debug(TAG, "get_latest_email: search ALL");
            let messages = match session.search("ALL") {
                Ok(m) => m,
                Err(e) => {
                    logger::debug(TAG, &format!("get_latest_email: search failed: {e}"));
                    session.logout().ok();
                    return None;
                }
            };

            let max_uid = match messages.into_iter().max() {
                Some(u) => u,
                None => {
                    logger::debug(TAG, "get_latest_email: no messages found");
                    session.logout().ok();
                    return None;
                }
            };

            logger::debug(TAG, &format!("get_latest_email: fetching RFC822 for {max_uid}"));
            let fetches = match session.fetch(max_uid.to_string(), "RFC822") {
                Ok(f) => f,
                Err(e) => {
                    logger::debug(TAG, &format!("get_latest_email: fetch failed: {e}"));
                    session.logout().ok();
                    return None;
                }
            };

            let message = fetches.iter().next();
            let body = message.and_then(|m| m.body());
            let text = body.map(|b| String::from_utf8_lossy(b).to_string());

            session.logout().ok();

            logger::info(TAG, &format!("get_latest_email: body len = {:?}", text.as_ref().map(|t| t.len())));
            text
        })
        .await
        .ok()
        .flatten()
    }

    /// 拉取最新 10 封邮件，按 UID 缓存为 HTML 文件，返回元数据列表（新→旧）
    /// 日志策略：过程全 debug，结束时一条 info 汇总（成功 or 失败在哪一步）
    pub async fn fetch_latest_emails(&self) -> Vec<EmailMeta> {
        let config = self.clone_email();
        let cache_dir = email_cache_dir();

        // 步骤名称
        const STEPS: [&str; 9] = [
            "not_configured", "tls", "connect", "login",
            "select_inbox", "uid_search", "fetch_bodies", "envelopes", "done",
        ];
        const DONE: u8 = 8;

        task::spawn_blocking(move || {
            let mut step: u8 = 0;
            let mut err_msg = String::new();

            if !config.is_configured() {
                logger::info(TAG, "fetch_latest: not configured");
                return vec![];
            }

            let result = (|| -> Vec<EmailMeta> {
                // 1 TLS
                let tls = match native_tls::TlsConnector::builder().build() {
                    Ok(t) => t,
                    Err(e) => { err_msg = format!("{e}"); return vec![]; }
                };
                step = 1;

                // 2 connect
                logger::debug(TAG, &format!("fetch_latest: connecting {}:{}", config.address, config.port));
                let client = match imap::connect(
                    (config.address.as_str(), config.port),
                    config.address.as_str(), &tls,
                ) {
                    Ok(c) => c,
                    Err(e) => { err_msg = format!("{e}"); return vec![]; }
                };
                step = 2;

                // 3 login
                logger::debug(TAG, "fetch_latest: logging in");
                let mut session = match client.login(&config.username, &config.auth) {
                    Ok(s) => s,
                    Err((e, _)) => { err_msg = format!("{e}"); return vec![]; }
                };
                step = 3;

                // 4 select INBOX
                logger::debug(TAG, "fetch_latest: selecting INBOX");
                if let Err(e) = session.select("INBOX") {
                    err_msg = format!("{e}");
                    session.logout().ok();
                    return vec![];
                }
                step = 4;

                // 5 uid_search
                logger::debug(TAG, "fetch_latest: uid_search ALL");
                let uids = match session.uid_search("ALL") {
                    Ok(m) => m,
                    Err(e) => { err_msg = format!("{e}"); session.logout().ok(); return vec![]; }
                };
                let mut uid_list: Vec<u32> = uids.into_iter().collect();
                uid_list.sort_unstable();
                uid_list.reverse();
                uid_list.truncate(10);
                step = 5;
                logger::debug(TAG, &format!("fetch_latest: top {} UIDs: {:?}", uid_list.len(), uid_list));

                // 6 fetch bodies (逐封)
                let need_fetch: Vec<u32> = uid_list.iter()
                    .filter(|u| !cache_dir.join(format!("{}.html", u)).exists())
                    .copied()
                    .collect();
                if !need_fetch.is_empty() {
                    logger::debug(TAG, &format!("fetch_latest: fetching {} bodies", need_fetch.len()));
                    for &uid in &need_fetch {
                        let uid_str = uid.to_string();
                        match session.uid_fetch(&uid_str, "(UID RFC822)") {
                            Ok(fetches) => {
                                if let Some(f) = fetches.iter().next() {
                                    if let Some(body) = f.body() {
                                        logger::debug(TAG, &format!("fetch_latest: UID {} body {} bytes", uid, body.len()));
                                        let html = extract_html_from_rfc822(body);
                                        let path = cache_dir.join(format!("{}.html", uid));
                                        if let Err(e) = std::fs::write(&path, &html) {
                                            logger::debug(TAG, &format!("fetch_latest: write {uid}.html fail: {e}"));
                                        }
                                    }
                                }
                            }
                            Err(e) => logger::debug(TAG, &format!("fetch_latest: fetch UID {uid} fail: {e}")),
                        }
                    }
                }
                step = 6;

                // 7 envelopes (逐封)
                logger::debug(TAG, "fetch_latest: fetching envelopes");
                let mut metas = Vec::new();
                for &uid in &uid_list {
                    let uid_str = uid.to_string();
                    match session.uid_fetch(&uid_str, "(UID ENVELOPE)") {
                        Ok(fetches) => {
                            if let Some(f) = fetches.iter().next() {
                                let env = f.envelope();
                                let subject_str = env
                                    .and_then(|e| e.subject.as_ref())
                                    .map(|s| decode_mime_str(s))
                                    .unwrap_or_default();
                                let from_str = env
                                    .and_then(|e| e.from.as_ref())
                                    .and_then(|addrs| addrs.first())
                                    .map(|a| {
                                        let name = a.name.as_ref().map(|n| decode_mime_str(n)).unwrap_or_default();
                                        let mailbox = a.mailbox.as_ref().map(|m| String::from_utf8_lossy(m).to_string()).unwrap_or_default();
                                        let host = a.host.as_ref().map(|h| String::from_utf8_lossy(h).to_string()).unwrap_or_default();
                                        if name.is_empty() { format!("{}@{}", mailbox, host) } else { name }
                                    })
                                    .unwrap_or_default();
                                let date_str = env
                                    .and_then(|e| e.date.as_ref())
                                    .map(|d| String::from_utf8_lossy(d).to_string())
                                    .unwrap_or_default();
                                metas.push(EmailMeta {
                                    uid: uid.to_string(),
                                    from: from_str,
                                    subject: subject_str,
                                    date: date_str,
                                    cached: cache_dir.join(format!("{}.html", uid)).exists(),
                                });
                            }
                        }
                        Err(e) => logger::debug(TAG, &format!("fetch_latest: envelope {uid} fail: {e}")),
                    }
                }
                step = 7;

                session.logout().ok();
                step = DONE;
                metas
            })();

            // 唯一一条 INFO 汇总
            if step >= DONE {
                logger::info(TAG, &format!("fetch_latest: ok, {} emails", result.len()));
            } else {
                logger::info(TAG, &format!(
                    "fetch_latest: failed at step {}({}) - {}",
                    step, STEPS[step as usize], err_msg
                ));
            }

            let mut sorted = result;
            sorted.sort_by(|a, b| b.uid.cmp(&a.uid));
            sorted
        })
        .await
        .unwrap_or_default()
    }

    fn clone_email(&self) -> Self {
        Self {
            username: self.username.clone(),
            auth: self.auth.clone(),
            address: self.address.clone(),
            port: self.port,
        }
    }
}

/// 元数据 JSON 文件路径
fn email_meta_path() -> PathBuf {
    email_cache_dir().join("email_meta.json")
}

/// 从磁盘加载已缓存的元数据（冷启动用）
pub fn load_email_metas() -> Vec<EmailMeta> {
    let path = email_meta_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => vec![],
    }
}

/// 将元数据持久化到磁盘
pub fn save_email_metas(metas: &[EmailMeta]) {
    let path = email_meta_path();
    if let Ok(json) = serde_json::to_string_pretty(metas) {
        if let Err(e) = std::fs::write(&path, json) {
            logger::debug(TAG, &format!("save_email_metas: write failed: {e}"));
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct EmailMeta {
    pub uid: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub cached: bool,
}

fn extract_html_from_rfc822(raw: &[u8]) -> String {
    match mailparse::parse_mail(raw) {
        Ok(parsed) => find_html_part(&parsed).unwrap_or_else(|| {
            let text = decode_body_smart(&parsed);
            format!("<html><head><meta charset=\"utf-8\"></head><body><pre>{}</pre></body></html>", html_escape(&text))
        }),
        Err(_) => {
            let text = String::from_utf8_lossy(raw).to_string();
            format!("<html><head><meta charset=\"utf-8\"></head><body><pre>{}</pre></body></html>", html_escape(&text))
        }
    }
}

/// 从 MIME part 的 Content-Type charset 参数或 HTML meta 标签中探测编码，
/// 用 encoding_rs 将原始字节正确转为 UTF-8。
fn decode_body_smart(mail: &mailparse::ParsedMail) -> String {
    let raw_bytes = match mail.get_body_raw() {
        Ok(b) => b,
        Err(_) => return mail.get_body().unwrap_or_default(),
    };

    // 1. 从 Content-Type charset 参数获取编码
    let mime_charset = mail.ctype.params.get("charset").map(|s| s.as_str()).unwrap_or("");

    // 2. 如果 MIME 没声明，从 HTML meta 标签中探测
    let charset = if mime_charset.is_empty() {
        detect_charset_from_html(&raw_bytes)
    } else {
        mime_charset.to_string()
    };

    // 3. 用 encoding_rs 解码
    decode_bytes_with_charset(&raw_bytes, &charset)
}

fn detect_charset_from_html(bytes: &[u8]) -> String {
    // 取前 2048 字节搜索 meta charset
    let head = if bytes.len() > 2048 { &bytes[..2048] } else { bytes };
    let lossy = String::from_utf8_lossy(head);
    let lower = lossy.to_lowercase();

    // <meta charset="xxx">
    if let Some(pos) = lower.find("charset") {
        let after = &lossy[pos..];
        // 跳过 charset 后的 = 和可能的空格/引号
        if let Some(eq) = after.find('=') {
            let val_start = &after[eq + 1..];
            let val = val_start
                .trim_start_matches(|c: char| c == ' ' || c == '"' || c == '\'')
                .split(|c: char| c == '"' || c == '\'' || c == ';' || c == ' ' || c == '>')
                .next()
                .unwrap_or("");
            if !val.is_empty() {
                return val.to_lowercase();
            }
        }
    }
    String::new()
}

fn decode_bytes_with_charset(bytes: &[u8], charset: &str) -> String {
    let label = charset.trim().to_lowercase();
    // 如果已经是 UTF-8 或为空，直接用 from_utf8_lossy
    if label.is_empty() || label == "utf-8" || label == "utf8" {
        return String::from_utf8_lossy(bytes).to_string();
    }
    // 用 encoding_rs 查找编码
    match encoding_rs::Encoding::for_label(label.as_bytes()) {
        Some(encoding) => {
            let (decoded, _, _) = encoding.decode(bytes);
            decoded.into_owned()
        }
        None => String::from_utf8_lossy(bytes).to_string(),
    }
}

/// 将 HTML 中的 charset 声明统一替换为 utf-8
fn normalize_html_charset(html: &str) -> String {
    use regex::Regex;
    let re1 = Regex::new(r#"(?i)(<meta\s+charset\s*=\s*")[^"]*(")"#).unwrap();
    let out = re1.replace_all(html, "${1}utf-8${2}").to_string();
    let re2 = Regex::new(r"(?i)(charset\s*=\s*)([\w\-]+)").unwrap();
    re2.replace_all(&out, "${1}utf-8").to_string()
}

fn find_html_part(mail: &mailparse::ParsedMail) -> Option<String> {
    let ct = mail.ctype.mimetype.to_lowercase();
    if ct == "text/html" {
        let decoded = decode_body_smart(mail);
        let normalized = normalize_html_charset(&decoded);
        return Some(ensure_html_head_charset(&normalized));
    }
    for sub in &mail.subparts {
        if let Some(html) = find_html_part(sub) {
            return Some(html);
        }
    }
    None
}

/// 确保 HTML 有 <head> 且包含 charset 声明，缺失时补 UTF-8
fn ensure_html_head_charset(html: &str) -> String {
    let lower = html.to_lowercase();
    // 已有 charset 声明 → 不动（normalize_html_charset 已处理过）
    if lower.contains("charset") {
        return html.to_string();
    }
    let meta = r#"<meta charset="utf-8">"#;
    // 有 <head> 但没 charset → 在 <head> 后插入
    if let Some(pos) = lower.find("<head>") {
        let insert = pos + "<head>".len();
        let mut out = String::with_capacity(html.len() + meta.len() + 1);
        out.push_str(&html[..insert]);
        out.push_str(meta);
        out.push_str(&html[insert..]);
        return out;
    }
    // 有 <head ...> 带属性的情况
    if let Some(pos) = lower.find("<head") {
        if let Some(close) = html[pos..].find('>') {
            let insert = pos + close + 1;
            let mut out = String::with_capacity(html.len() + meta.len() + 1);
            out.push_str(&html[..insert]);
            out.push_str(meta);
            out.push_str(&html[insert..]);
            return out;
        }
    }
    // 没有 <head> → 在 <html> 后插入 <head>...</head>，或直接加到最前面
    let head_block = format!("<head>{}</head>", meta);
    if let Some(pos) = lower.find("<html") {
        if let Some(close) = html[pos..].find('>') {
            let insert = pos + close + 1;
            let mut out = String::with_capacity(html.len() + head_block.len() + 1);
            out.push_str(&html[..insert]);
            out.push_str(&head_block);
            out.push_str(&html[insert..]);
            return out;
        }
    }
    // 完全没有 <html>/<head> → 包一层
    format!("<html>{}{}</html>", head_block, html)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn decode_mime_str(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw).to_string();
    match mailparse::parse_header(format!("X: {}", s).as_bytes()) {
        Ok((hdr, _)) => {
            let val = hdr.get_value();
            if val.is_empty() { s } else { val }
        }
        Err(_) => s,
    }
}

#[tauri::command]
pub fn fetch_emails(state: tauri::State<'_, crate::IslandState>) -> Result<Vec<EmailMeta>, String> {
    Ok(state.cached_email_metas.lock().unwrap().clone())
}

#[tauri::command]
pub async fn refresh_emails(state: tauri::State<'_, crate::IslandState>) -> Result<Vec<EmailMeta>, String> {
    let config = state.email_config.lock().unwrap().clone();
    let metas = config.fetch_latest_emails().await;
    *state.cached_email_metas.lock().unwrap() = metas.clone();
    save_email_metas(&metas);
    Ok(metas)
}

#[tauri::command]
pub fn get_email_cache_dir() -> String {
    email_cache_dir().to_string_lossy().to_string()
}

/// 获取所有邮件 UID 列表（新→旧），供前端无限滚动用
#[tauri::command]
pub async fn fetch_email_uid_list(state: tauri::State<'_, crate::IslandState>) -> Result<Vec<u32>, String> {
    let config = state.email_config.lock().unwrap().clone();
    task::spawn_blocking(move || {
        if !config.is_configured() {
            return Err("邮箱未配置".to_string());
        }
        let tls = native_tls::TlsConnector::builder().build().map_err(|e| e.to_string())?;
        let client = imap::connect(
            (config.address.as_str(), config.port),
            config.address.as_str(), &tls,
        ).map_err(|e| e.to_string())?;
        let mut session = client.login(&config.username, &config.auth)
            .map_err(|(e, _)| e.to_string())?;
        session.select("INBOX").map_err(|e| e.to_string())?;
        let uids = session.uid_search("ALL").map_err(|e| e.to_string())?;
        session.logout().ok();
        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.sort_unstable();
        uid_list.reverse();
        Ok(uid_list)
    }).await.map_err(|e| e.to_string())?
}

/// 按 UID 列表批量获取邮件元数据（主题、发件人、日期）
#[tauri::command]
pub async fn fetch_email_metas_by_uids(state: tauri::State<'_, crate::IslandState>, uids: Vec<u32>) -> Result<Vec<EmailMeta>, String> {
    if uids.is_empty() {
        return Ok(vec![]);
    }
    let config = state.email_config.lock().unwrap().clone();
    let cache_dir = email_cache_dir();
    task::spawn_blocking(move || {
        if !config.is_configured() {
            return Err("邮箱未配置".to_string());
        }
        let tls = native_tls::TlsConnector::builder().build().map_err(|e| e.to_string())?;
        let client = imap::connect(
            (config.address.as_str(), config.port),
            config.address.as_str(), &tls,
        ).map_err(|e| e.to_string())?;
        let mut session = client.login(&config.username, &config.auth)
            .map_err(|(e, _)| e.to_string())?;
        session.select("INBOX").map_err(|e| e.to_string())?;

        let mut metas = Vec::new();
        for &uid in &uids {
            let uid_str = uid.to_string();
            match session.uid_fetch(&uid_str, "(UID ENVELOPE)") {
                Ok(fetches) => {
                    if let Some(f) = fetches.iter().next() {
                        let env = f.envelope();
                        let subject_str = env
                            .and_then(|e| e.subject.as_ref())
                            .map(|s| decode_mime_str(s))
                            .unwrap_or_default();
                        let from_str = env
                            .and_then(|e| e.from.as_ref())
                            .and_then(|addrs| addrs.first())
                            .map(|a| {
                                let name = a.name.as_ref().map(|n| decode_mime_str(n)).unwrap_or_default();
                                let mailbox = a.mailbox.as_ref().map(|m| String::from_utf8_lossy(m).to_string()).unwrap_or_default();
                                let host = a.host.as_ref().map(|h| String::from_utf8_lossy(h).to_string()).unwrap_or_default();
                                if name.is_empty() { format!("{}@{}", mailbox, host) } else { name }
                            })
                            .unwrap_or_default();
                        let date_str = env
                            .and_then(|e| e.date.as_ref())
                            .map(|d| String::from_utf8_lossy(d).to_string())
                            .unwrap_or_default();
                        metas.push(EmailMeta {
                            uid: uid.to_string(),
                            from: from_str,
                            subject: subject_str,
                            date: date_str,
                            cached: cache_dir.join(format!("{}.html", uid)).exists(),
                        });
                    }
                }
                Err(e) => logger::debug(TAG, &format!("fetch_metas_by_uids: envelope {uid} fail: {e}")),
            }
        }
        session.logout().ok();
        logger::info(TAG, &format!("fetch_metas_by_uids: returned {} metas", metas.len()));
        Ok(metas)
    }).await.map_err(|e| e.to_string())?
}

/// 按 UID 获取单封邮件正文并缓存为 HTML，返回是否成功
#[tauri::command]
pub async fn fetch_email_body_by_uid(state: tauri::State<'_, crate::IslandState>, uid: u32) -> Result<bool, String> {
    let cache_dir = email_cache_dir();
    let path = cache_dir.join(format!("{}.html", uid));
    // 已缓存则直接返回
    if path.exists() {
        return Ok(true);
    }
    let config = state.email_config.lock().unwrap().clone();
    task::spawn_blocking(move || {
        if !config.is_configured() {
            return Err("邮箱未配置".to_string());
        }
        let tls = native_tls::TlsConnector::builder().build().map_err(|e| e.to_string())?;
        let client = imap::connect(
            (config.address.as_str(), config.port),
            config.address.as_str(), &tls,
        ).map_err(|e| e.to_string())?;
        let mut session = client.login(&config.username, &config.auth)
            .map_err(|(e, _)| e.to_string())?;
        session.select("INBOX").map_err(|e| e.to_string())?;

        let uid_str = uid.to_string();
        match session.uid_fetch(&uid_str, "(UID RFC822)") {
            Ok(fetches) => {
                if let Some(f) = fetches.iter().next() {
                    if let Some(body) = f.body() {
                        let html = extract_html_from_rfc822(body);
                        std::fs::write(&path, &html).map_err(|e| e.to_string())?;
                        logger::info(TAG, &format!("fetch_body_by_uid: UID {} cached ({} bytes)", uid, html.len()));
                    }
                }
            }
            Err(e) => {
                session.logout().ok();
                return Err(format!("fetch UID {} failed: {}", uid, e));
            }
        }
        session.logout().ok();
        Ok(true)
    }).await.map_err(|e| e.to_string())?
}