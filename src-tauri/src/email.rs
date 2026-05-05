use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use crate::logger;
use tokio::task;

const TAG: &str = "Email";

static EMAIL_META_SAVE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
        //logger::debug(TAG, &format!("is_configured = {} (user={}, addr={}, port={})",
        //    ok, self.username, self.address, self.port));
        ok
    }

    pub async fn get_latest_uid(&self) -> Option<String> {
        let config = self.clone_email();
        //logger::debug(TAG, "get_latest_uid: start");

        task::spawn_blocking(move || {
            if !config.is_configured() {
                //logger::debug(TAG, "get_latest_uid: not configured, skip");
                return None;
            }

            //logger::debug(TAG, &format!("get_latest_uid: building TLS connector"));
            let tls = match native_tls::TlsConnector::builder().build() {
                Ok(t) => t,
                Err(_) => {
                    //logger::debug(TAG, &format!("get_latest_uid: TLS build failed: {e}"));
                    return None;
                }
            };

            //logger::debug(TAG, &format!("get_latest_uid: connecting to {}:{}", config.address, config.port));
            let client = match imap::connect(
                (config.address.as_str(), config.port),
                config.address.as_str(),
                &tls,
            ) {
                Ok(c) => c,
                Err(_) => {
                    //logger::debug(TAG, &format!("get_latest_uid: connect failed: {e}"));
                    return None;
                }
            };

            //logger::debug(TAG, "get_latest_uid: logging in");
            let mut session = match client.login(&config.username, &config.auth) {
                Ok(s) => s,
                Err((_, _)) => {
                    //logger::debug(TAG, &format!("get_latest_uid: login failed: {e}"));
                    return None;
                }
            };

            //logger::debug(TAG, "get_latest_uid: selecting INBOX");
            if let Err(_) = session.select("INBOX") {
                //logger::debug(TAG, &format!("get_latest_uid: select INBOX failed: {e}"));
                session.logout().ok();
                return None;
            }

            //logger::debug(TAG, "get_latest_uid: uid_search ALL");
            let messages = match session.uid_search("ALL") {
                Ok(m) => m,
                Err(_) => {
                    //logger::debug(TAG, &format!("get_latest_uid: uid_search failed: {e}"));
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
        //logger::debug(TAG, "get_latest_email: start");

        task::spawn_blocking(move || {
            if !config.is_configured() {
                //logger::debug(TAG, "get_latest_email: not configured, skip");
                return None;
            }

            //logger::debug(TAG, "get_latest_email: building TLS connector");
            let tls = match native_tls::TlsConnector::builder().build() {
                Ok(t) => t,
                Err(_) => {
                    //logger::debug(TAG, &format!("get_latest_email: TLS build failed: {e}"));
                    return None;
                }
            };

            //logger::debug(TAG, &format!("get_latest_email: connecting to {}:{}", config.address, config.port));
            let client = match imap::connect(
                (config.address.as_str(), config.port),
                config.address.as_str(),
                &tls,
            ) {
                Ok(c) => c,
                Err(_) => {
                    //logger::debug(TAG, &format!("get_latest_email: connect failed: {e}"));
                    return None;
                }
            };

            //logger::debug(TAG, "get_latest_email: logging in");
            let mut session = match client.login(&config.username, &config.auth) {
                Ok(s) => s,
                Err((_, _)) => {
                    //logger::debug(TAG, &format!("get_latest_email: login failed: {e}"));
                    return None;
                }
            };

            //logger::debug(TAG, "get_latest_email: selecting INBOX");
            if let Err(_) = session.select("INBOX") {
                //logger::debug(TAG, &format!("get_latest_email: select INBOX failed: {e}"));
                session.logout().ok();
                return None;
            }

            //logger::debug(TAG, "get_latest_email: search ALL");
            let messages = match session.search("ALL") {
                Ok(m) => m,
                Err(_) => {
                    //logger::debug(TAG, &format!("get_latest_email: search failed: {e}"));
                    session.logout().ok();
                    return None;
                }
            };

            let max_uid = match messages.into_iter().max() {
                Some(u) => u,
                None => {
                    //logger::debug(TAG, "get_latest_email: no messages found");
                    session.logout().ok();
                    return None;
                }
            };

            //logger::debug(TAG, &format!("get_latest_email: fetching RFC822 for {max_uid}"));
            let fetches = match session.fetch(max_uid.to_string(), "RFC822") {
                Ok(f) => f,
                Err(_) => {
                    //logger::debug(TAG, &format!("get_latest_email: fetch failed: {e}"));
                    session.logout().ok();
                    return None;
                }
            };

            let message = fetches.iter().next();
            let body = message.and_then(|m| m.body());
            let text = body.map(|b| String::from_utf8_lossy(b).to_string());

            session.logout().ok();

            //logger::info(TAG, &format!("get_latest_email: body len = {:?}", text.as_ref().map(|t| t.len())));
            text
        })
        .await
        .ok()
        .flatten()
    }

    pub async fn fetch_metas_by_uids(&self, uids: Vec<u32>) -> Result<Vec<EmailMeta>, String> {
        if uids.is_empty() {
            logger::debug(TAG, "fetch_metas_by_uids: empty uids");
            return Ok(vec![]);
        }
        logger::debug(TAG, &format!("fetch_metas_by_uids: start, {} uids", uids.len()));
        let config = self.clone_email();
        let cache_dir = email_cache_dir();
        let requested_len = uids.len();
        let metas = task::spawn_blocking(move || {
            if !config.is_configured() {
                logger::debug(TAG, "fetch_metas_by_uids: not configured");
                return Err("邮箱未配置".to_string());
            }

            let max_parallel = 4usize;
            let chunk_size = ((uids.len() + max_parallel - 1) / max_parallel).max(1);
            let mut handles = Vec::new();

            for chunk in uids.chunks(chunk_size) {
                let chunk_uids = chunk.to_vec();
                let config_t = config.clone();
                let cache_dir_t = cache_dir.clone();
                handles.push(std::thread::spawn(move || {
                    logger::debug(TAG, &format!("fetch_metas_by_uids: worker start, {} uids", chunk_uids.len()));
                    let tls = match native_tls::TlsConnector::builder().build() {
                        Ok(t) => t,
                        Err(_) => {
                            logger::debug(TAG, "fetch_metas_by_uids: TLS build failed");
                            return Vec::new();
                        }
                    };
                    let client = match imap::connect(
                        (config_t.address.as_str(), config_t.port),
                        config_t.address.as_str(), &tls,
                    ) {
                        Ok(c) => c,
                        Err(_) => {
                            logger::debug(TAG, "fetch_metas_by_uids: connect failed");
                            return Vec::new();
                        }
                    };
                    let mut session = match client.login(&config_t.username, &config_t.auth) {
                        Ok(s) => s,
                        Err((_, _)) => {
                            logger::debug(TAG, "fetch_metas_by_uids: login failed");
                            return Vec::new();
                        }
                    };
                    if let Err(_) = session.select("INBOX") {
                        logger::debug(TAG, "fetch_metas_by_uids: select INBOX failed");
                        session.logout().ok();
                        return Vec::new();
                    }

                    let uid_query = chunk_uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
                    let mut metas = Vec::new();
                    match session.uid_fetch(&uid_query, "(UID ENVELOPE)") {
                        Ok(fetches) => {
                            for f in fetches.iter() {
                                let Some(uid) = f.uid else { continue; };
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
                                    cached: cache_dir_t.join(format!("{}.html", uid)).exists(),
                                });
                            }
                        }
                        Err(_) => {
                            logger::debug(TAG, &format!("fetch_metas_by_uids: uid_fetch failed for {}", uid_query));
                        }
                    }
                    session.logout().ok();
                    logger::debug(TAG, &format!("fetch_metas_by_uids: worker done, {} metas", metas.len()));
                    metas
                }));
            }

            let mut metas = Vec::new();
            for handle in handles {
                match handle.join() {
                    Ok(mut batch) => metas.append(&mut batch),
                    Err(_) => {}
                }
            }

            let uid_order: std::collections::HashMap<u32, usize> = uids
                .iter()
                .enumerate()
                .map(|(idx, uid)| (*uid, idx))
                .collect();
            metas.sort_by_key(|m| m.uid.parse::<u32>().ok().and_then(|uid| uid_order.get(&uid).copied()).unwrap_or(usize::MAX));
            Ok(metas)
        }).await.map_err(|e| e.to_string())??;
        logger::debug(TAG, &format!("fetch_metas_by_uids: done, requested {} returned {}", requested_len, metas.len()));
        Ok(metas)
    }

    pub async fn fetch_bodies_by_uids(&self, uids: Vec<u32>) -> Result<Vec<u32>, String> {
        if uids.is_empty() {
            logger::debug(TAG, "fetch_bodies_by_uids: empty uids");
            return Ok(vec![]);
        }
        logger::debug(TAG, &format!("fetch_bodies_by_uids: start, {} uids", uids.len()));
        let config = self.clone_email();
        let cache_dir = email_cache_dir();
        let requested_uids = uids.clone();
        let cached_uids: Vec<u32> = uids
            .iter()
            .filter(|uid| cache_dir.join(format!("{}.html", uid)).exists())
            .copied()
            .collect();
        let missing_uids: Vec<u32> = uids
            .iter()
            .filter(|uid| !cache_dir.join(format!("{}.html", uid)).exists())
            .copied()
            .collect();
        logger::debug(TAG, &format!("fetch_bodies_by_uids: cache hit {} missing {}", cached_uids.len(), missing_uids.len()));
        if missing_uids.is_empty() {
            return Ok(cached_uids);
        }

        let mut done_uids = task::spawn_blocking(move || {
            if !config.is_configured() {
                logger::debug(TAG, "fetch_bodies_by_uids: not configured");
                return Err("邮箱未配置".to_string());
            }
            logger::debug(TAG, &format!("fetch_bodies_by_uids: connecting {}:{}", config.address, config.port));
            let tls = native_tls::TlsConnector::builder().build().map_err(|e| e.to_string())?;
            let client = imap::connect(
                (config.address.as_str(), config.port),
                config.address.as_str(), &tls,
            ).map_err(|e| e.to_string())?;
            logger::debug(TAG, "fetch_bodies_by_uids: logging in");
            let mut session = client.login(&config.username, &config.auth)
                .map_err(|(e, _)| e.to_string())?;
            logger::debug(TAG, "fetch_bodies_by_uids: selecting INBOX");
            session.select("INBOX").map_err(|e| e.to_string())?;

            let mut done_uids = cached_uids;
            for uid in missing_uids {
                let uid_str = uid.to_string();
                logger::debug(TAG, &format!("fetch_bodies_by_uids: uid_fetch RFC822 uid={}", uid));
                match session.uid_fetch(&uid_str, "(UID RFC822)") {
                    Ok(fetches) => {
                        if let Some(f) = fetches.iter().next() {
                            if let Some(body) = f.body() {
                                let html = extract_html_from_rfc822(body);
                                let path = cache_dir.join(format!("{}.html", uid));
                                match std::fs::write(&path, &html) {
                                    Ok(_) => {
                                        logger::debug(TAG, &format!("fetch_bodies_by_uids: cached uid={} bytes={}", uid, html.len()));
                                        done_uids.push(uid);
                                    }
                                    Err(e) => {
                                        logger::debug(TAG, &format!("fetch_bodies_by_uids: write failed uid={} error={}", uid, e));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        logger::debug(TAG, &format!("fetch_bodies_by_uids: uid_fetch failed uid={} error={}", uid, e));
                    }
                }
            }
            session.logout().ok();
            Ok(done_uids)
        }).await.map_err(|e| e.to_string())??;

        let uid_order: std::collections::HashMap<u32, usize> = requested_uids
            .iter()
            .enumerate()
            .map(|(idx, uid)| (*uid, idx))
            .collect();
        done_uids.sort_by_key(|uid| uid_order.get(uid).copied().unwrap_or(usize::MAX));
        logger::debug(TAG, &format!("fetch_bodies_by_uids: done, {} bodies available", done_uids.len()));
        Ok(done_uids)
    }

    pub async fn fetch_metas_and_bodies_by_uids(&self, uids: Vec<u32>) -> Result<Vec<EmailMeta>, String> {
        if uids.is_empty() {
            logger::debug(TAG, "fetch_metas_and_bodies_by_uids: empty uids");
            return Ok(vec![]);
        }
        logger::debug(TAG, &format!("fetch_metas_and_bodies_by_uids: start, {} uids", uids.len()));
        let _ = self.fetch_bodies_by_uids(uids.clone()).await?;
        let mut metas = self.fetch_metas_by_uids(uids).await?;
        let cache_dir = email_cache_dir();
        for meta in &mut metas {
            meta.cached = cache_dir.join(format!("{}.html", meta.uid)).exists();
        }
        logger::debug(TAG, &format!("fetch_metas_and_bodies_by_uids: done, {} metas", metas.len()));
        Ok(metas)
    }

    /// 拉取最新 10 封邮件，按 UID 缓存为 HTML 文件，返回元数据列表（新→旧）
    /// 日志策略：过程全 debug，结束时一条 info 汇总（成功 or 失败在哪一步）
    pub async fn fetch_latest_emails(&self) -> Vec<EmailMeta> {
        let config = self.clone_email();
        let cache_dir = email_cache_dir();
        logger::debug(TAG, &format!("fetch_latest_emails: start cache_dir={}", cache_dir.to_string_lossy()));

        // 步骤名称
        const DONE: u8 = 8;

        task::spawn_blocking(move || {
            let mut step: u8 = 0;
            let mut err_msg = String::new();

            if !config.is_configured() {
                logger::debug(TAG, "fetch_latest_emails: not configured");
                //logger::info(TAG, "fetch_latest: not configured");
                return vec![];
            }

            let result = (|| -> Vec<EmailMeta> {
                // 1 TLS
                let tls = match native_tls::TlsConnector::builder().build() {
                    Ok(t) => t,
                    Err(e) => { err_msg = format!("{e}"); logger::debug(TAG, &format!("fetch_latest_emails: TLS build failed: {}", err_msg)); return vec![]; }
                };
                step = 1;

                // 2 connect
                logger::debug(TAG, &format!("fetch_latest_emails: connecting {}:{}", config.address, config.port));
                let client = match imap::connect(
                    (config.address.as_str(), config.port),
                    config.address.as_str(), &tls,
                ) {
                    Ok(c) => c,
                    Err(e) => { err_msg = format!("{e}"); logger::debug(TAG, &format!("fetch_latest_emails: connect failed: {}", err_msg)); return vec![]; }
                };
                step = 2;

                // 3 login
                logger::debug(TAG, "fetch_latest_emails: logging in");
                let mut session = match client.login(&config.username, &config.auth) {
                    Ok(s) => s,
                    Err((e, _)) => { err_msg = format!("{e}"); logger::debug(TAG, &format!("fetch_latest_emails: login failed: {}", err_msg)); return vec![]; }
                };
                step = 3;

                // 4 select INBOX
                logger::debug(TAG, "fetch_latest_emails: selecting INBOX");
                if let Err(e) = session.select("INBOX") {
                    err_msg = format!("{e}");
                    logger::debug(TAG, &format!("fetch_latest_emails: select INBOX failed: {}", err_msg));
                    session.logout().ok();
                    return vec![];
                }
                step = 4;

                // 5 uid_search
                logger::debug(TAG, "fetch_latest_emails: uid_search ALL");
                let uids = match session.uid_search("ALL") {
                    Ok(m) => m,
                    Err(e) => { err_msg = format!("{e}"); logger::debug(TAG, &format!("fetch_latest_emails: uid_search failed: {}", err_msg)); session.logout().ok(); return vec![]; }
                };
                let mut uid_list: Vec<u32> = uids.into_iter().collect();
                uid_list.sort_unstable();
                uid_list.reverse();
                uid_list.truncate(10);
                step = 5;
                logger::debug(TAG, &format!("fetch_latest_emails: top {} UIDs: {:?}", uid_list.len(), uid_list));

                // 6 fetch bodies (逐封)
                let need_fetch: Vec<u32> = uid_list.iter()
                    .filter(|u| !cache_dir.join(format!("{}.html", u)).exists())
                    .copied()
                    .collect();
                logger::debug(TAG, &format!("fetch_latest_emails: need fetch {} bodies", need_fetch.len()));
                if !need_fetch.is_empty() {
                    //logger::debug(TAG, &format!("fetch_latest: fetching {} bodies", need_fetch.len()));
                    for &uid in &need_fetch {
                        let uid_str = uid.to_string();
                        match session.uid_fetch(&uid_str, "(UID RFC822)") {
                            Ok(fetches) => {
                                if let Some(f) = fetches.iter().next() {
                                    if let Some(body) = f.body() {
                                        logger::debug(TAG, &format!("fetch_latest_emails: UID {} body {} bytes", uid, body.len()));
                                        let html = extract_html_from_rfc822(body);
                                        let path = cache_dir.join(format!("{}.html", uid));
                                        if let Err(e) = std::fs::write(&path, &html) {
                                            logger::debug(TAG, &format!("fetch_latest_emails: write {}.html failed: {}", uid, e));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                logger::debug(TAG, &format!("fetch_latest_emails: fetch body failed uid={} error={}", uid, e));
                            }
                        }
                    }
                }
                step = 6;

                // 7 envelopes (逐封)
                logger::debug(TAG, "fetch_latest_emails: fetching envelopes");
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
                        Err(e) => {
                            logger::debug(TAG, &format!("fetch_latest_emails: envelope fetch failed uid={} error={}", uid, e));
                        }
                    }
                }
                step = 7;

                session.logout().ok();
                step = DONE;
                metas
            })();

            // 唯一一条 INFO 汇总
            if step >= DONE {
                logger::debug(TAG, &format!("fetch_latest_emails: ok, {} emails", result.len()));
            } else {
                logger::debug(TAG, &format!("fetch_latest_emails: failed at step {} - {}", step, err_msg));
            }

            let mut sorted = result;
            sorted.sort_by(|a, b| b.uid.cmp(&a.uid));
            logger::debug(TAG, &format!("fetch_latest_emails: sorted {} metas", sorted.len()));
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

fn email_meta_save_lock() -> &'static Mutex<()> {
    EMAIL_META_SAVE_LOCK.get_or_init(|| Mutex::new(()))
}

fn scan_cached_email_body_metas() -> Vec<EmailMeta> {
    let dir = email_cache_dir();
    let mut metas = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("html") {
                continue;
            }
            let Some(uid) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if uid.parse::<u32>().is_err() {
                continue;
            }
            metas.push(EmailMeta {
                uid: uid.to_string(),
                from: String::new(),
                subject: String::new(),
                date: String::new(),
                cached: true,
            });
        }
    }
    metas.sort_by(|a, b| {
        let au = a.uid.parse::<u32>().unwrap_or_default();
        let bu = b.uid.parse::<u32>().unwrap_or_default();
        bu.cmp(&au)
    });
    logger::debug(TAG, &format!("scan_cached_email_body_metas: found {} html bodies", metas.len()));
    metas
}

fn is_blank_email_meta(meta: &EmailMeta) -> bool {
    meta.from.trim().is_empty() && meta.subject.trim().is_empty() && meta.date.trim().is_empty()
}

fn sort_email_metas_newest_first(metas: &mut Vec<EmailMeta>) {
    metas.sort_by(|a, b| {
        let au = a.uid.parse::<u32>().unwrap_or_default();
        let bu = b.uid.parse::<u32>().unwrap_or_default();
        bu.cmp(&au)
    });
}

fn upsert_email_metas(cached: &mut Vec<EmailMeta>, metas: &[EmailMeta]) {
    for meta in metas {
        if let Some(existing) = cached.iter_mut().find(|m| m.uid == meta.uid) {
            if is_blank_email_meta(meta) && !is_blank_email_meta(existing) {
                existing.cached = existing.cached || meta.cached;
            } else {
                *existing = meta.clone();
            }
        } else {
            cached.push(meta.clone());
        }
    }
    sort_email_metas_newest_first(cached);
}

pub fn merge_email_metas(cached: &mut Vec<EmailMeta>, metas: &[EmailMeta]) {
    upsert_email_metas(cached, metas);
}

fn read_email_meta_json_file(path: &PathBuf) -> Result<Vec<EmailMeta>, String> {
    let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

/// 从磁盘加载已缓存的元数据（冷启动用）
pub fn load_email_metas() -> Vec<EmailMeta> {
    let path = email_meta_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let metas: Vec<EmailMeta> = serde_json::from_str(&s).unwrap_or_default();
            if metas.is_empty() {
                logger::debug(TAG, "load_email_metas: meta json empty, scanning cached bodies");
                scan_cached_email_body_metas()
            } else {
                logger::debug(TAG, &format!("load_email_metas: loaded {} metas from json", metas.len()));
                metas
            }
        }
        Err(e) => {
            logger::debug(TAG, &format!("load_email_metas: meta json unavailable {}, scanning cached bodies", e));
            scan_cached_email_body_metas()
        }
    }
}

pub fn save_email_metas(metas: &[EmailMeta]) {
    let _guard = match email_meta_save_lock().lock() {
        Ok(guard) => guard,
        Err(e) => {
            logger::debug(TAG, &format!("save_email_metas: lock poisoned: {}", e));
            return;
        }
    };
    let path = email_meta_path();
    let existing = read_email_meta_json_file(&path).unwrap_or_default();
    if metas.is_empty() && !existing.is_empty() {
        logger::debug(TAG, &format!("save_email_metas: skip empty overwrite, existing {} metas at {}", existing.len(), path.to_string_lossy()));
        return;
    }
    let json = match serde_json::to_string_pretty(metas) {
        Ok(json) => json,
        Err(e) => {
            logger::debug(TAG, &format!("save_email_metas: serialize failed: {}", e));
            return;
        }
    };
    let backup_path = path.with_extension("json.bak");
    if path.exists() {
        if let Err(e) = std::fs::copy(&path, &backup_path) {
            logger::debug(TAG, &format!("save_email_metas: backup failed {} -> {} error={}", path.to_string_lossy(), backup_path.to_string_lossy(), e));
        }
    }
    let tmp_path = path.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&tmp_path, json) {
        logger::debug(TAG, &format!("save_email_metas: tmp write failed {} error={}", tmp_path.to_string_lossy(), e));
        return;
    }
    if path.exists() {
        if let Err(e) = std::fs::remove_file(&path) {
            logger::debug(TAG, &format!("save_email_metas: remove old file failed {} error={}", path.to_string_lossy(), e));
            let _ = std::fs::remove_file(&tmp_path);
            return;
        }
    }
    if let Err(e) = std::fs::rename(&tmp_path, &path) {
        logger::debug(TAG, &format!("save_email_metas: rename failed {} -> {} error={}", tmp_path.to_string_lossy(), path.to_string_lossy(), e));
        let _ = std::fs::remove_file(&tmp_path);
        if backup_path.exists() && !path.exists() {
            if let Err(restore_err) = std::fs::copy(&backup_path, &path) {
                logger::debug(TAG, &format!("save_email_metas: restore backup failed {} -> {} error={}", backup_path.to_string_lossy(), path.to_string_lossy(), restore_err));
            }
        }
        return;
    }
    logger::debug(TAG, &format!("save_email_metas: saved {} metas to {}", metas.len(), path.to_string_lossy()));
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct EmailMeta {
    pub uid: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub cached: bool,
}

#[derive(Clone, serde::Serialize)]
pub struct EmailCacheDiagnostics {
    pub cache_dir: String,
    pub meta_path: String,
    pub meta_backup_path: String,
    pub meta_tmp_path: String,
    pub html_count: usize,
    pub meta_count: usize,
    pub blank_meta_count: usize,
    pub html_without_meta: Vec<u32>,
    pub meta_without_html: Vec<u32>,
    pub meta_file_exists: bool,
    pub meta_backup_exists: bool,
    pub meta_tmp_exists: bool,
    pub meta_file_bytes: u64,
    pub meta_backup_bytes: u64,
    pub meta_tmp_bytes: u64,
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
pub async fn fetch_emails(state: tauri::State<'_, crate::IslandState>) -> Result<Vec<EmailMeta>, String> {
    logger::debug(TAG, "fetch_emails: start");
    let local_body_metas = scan_cached_email_body_metas();
    let mut metas = state.cached_email_metas.lock().unwrap().clone();
    if !local_body_metas.is_empty() {
        logger::debug(TAG, &format!("fetch_emails: merging {} local body metas", local_body_metas.len()));
        upsert_email_metas(&mut metas, &local_body_metas);
    }

    let missing_meta_uids: Vec<u32> = metas
        .iter()
        .filter(|meta| meta.cached && is_blank_email_meta(meta))
        .filter_map(|meta| meta.uid.parse::<u32>().ok())
        .collect();
    logger::debug(TAG, &format!("fetch_emails: {} cached bodies missing metas", missing_meta_uids.len()));

    if !missing_meta_uids.is_empty() {
        let config = state.email_config.lock().unwrap().clone();
        match config.fetch_metas_by_uids(missing_meta_uids).await {
            Ok(fetched_metas) => {
                logger::debug(TAG, &format!("fetch_emails: fetched {} missing metas", fetched_metas.len()));
                upsert_email_metas(&mut metas, &fetched_metas);
            }
            Err(e) => {
                logger::debug(TAG, &format!("fetch_emails: fetch missing metas failed: {}", e));
            }
        }
    }

    {
        let mut cached = state.cached_email_metas.lock().unwrap();
        *cached = metas.clone();
        save_email_metas(&cached);
    }
    logger::debug(TAG, &format!("fetch_emails: returning {} cached metas", metas.len()));
    Ok(metas)
}

#[tauri::command]
pub async fn refresh_emails(state: tauri::State<'_, crate::IslandState>) -> Result<Vec<EmailMeta>, String> {
    logger::debug(TAG, "refresh_emails: start");
    let config = state.email_config.lock().unwrap().clone();
    let metas = config.fetch_latest_emails().await;
    let mut cached = state.cached_email_metas.lock().unwrap();
    if metas.is_empty() && !cached.is_empty() {
        logger::debug(TAG, &format!("refresh_emails: fetched 0 metas, keeping existing {} cached metas", cached.len()));
        return Ok(cached.clone());
    }
    upsert_email_metas(&mut cached, &metas);
    save_email_metas(&cached);
    logger::debug(TAG, &format!("refresh_emails: done, fetched {} metas, cached {} metas", metas.len(), cached.len()));
    Ok(cached.clone())
}

#[tauri::command]
pub fn get_email_cache_dir() -> String {
    let dir = email_cache_dir().to_string_lossy().to_string();
    logger::debug(TAG, &format!("get_email_cache_dir: {}", dir));
    dir
}

#[tauri::command]
pub fn diagnose_email_cache(state: tauri::State<'_, crate::IslandState>) -> Result<EmailCacheDiagnostics, String> {
    let cache_dir = email_cache_dir();
    let meta_path = email_meta_path();
    let backup_path = meta_path.with_extension("json.bak");
    let tmp_path = meta_path.with_extension("json.tmp");
    let body_metas = scan_cached_email_body_metas();
    let cached = state.cached_email_metas.lock().unwrap().clone();
    let html_uids: std::collections::HashSet<u32> = body_metas
        .iter()
        .filter_map(|meta| meta.uid.parse::<u32>().ok())
        .collect();
    let meta_uids: std::collections::HashSet<u32> = cached
        .iter()
        .filter_map(|meta| meta.uid.parse::<u32>().ok())
        .collect();
    let mut html_without_meta: Vec<u32> = html_uids.difference(&meta_uids).copied().collect();
    let mut meta_without_html: Vec<u32> = meta_uids.difference(&html_uids).copied().collect();
    html_without_meta.sort_unstable();
    html_without_meta.reverse();
    meta_without_html.sort_unstable();
    meta_without_html.reverse();
    let diag = EmailCacheDiagnostics {
        cache_dir: cache_dir.to_string_lossy().to_string(),
        meta_path: meta_path.to_string_lossy().to_string(),
        meta_backup_path: backup_path.to_string_lossy().to_string(),
        meta_tmp_path: tmp_path.to_string_lossy().to_string(),
        html_count: html_uids.len(),
        meta_count: cached.len(),
        blank_meta_count: cached.iter().filter(|meta| is_blank_email_meta(meta)).count(),
        html_without_meta,
        meta_without_html,
        meta_file_exists: meta_path.exists(),
        meta_backup_exists: backup_path.exists(),
        meta_tmp_exists: tmp_path.exists(),
        meta_file_bytes: meta_path.metadata().map(|m| m.len()).unwrap_or_default(),
        meta_backup_bytes: backup_path.metadata().map(|m| m.len()).unwrap_or_default(),
        meta_tmp_bytes: tmp_path.metadata().map(|m| m.len()).unwrap_or_default(),
    };
    logger::debug(TAG, &format!(
        "diagnose_email_cache: html={} metas={} blank={} html_without_meta={} meta_without_html={} meta_exists={} bak_exists={} tmp_exists={}",
        diag.html_count,
        diag.meta_count,
        diag.blank_meta_count,
        diag.html_without_meta.len(),
        diag.meta_without_html.len(),
        diag.meta_file_exists,
        diag.meta_backup_exists,
        diag.meta_tmp_exists
    ));
    Ok(diag)
}

#[tauri::command]
pub fn clear_email_cache(state: tauri::State<'_, crate::IslandState>) -> Result<(), String> {
    let dir = email_cache_dir();
    logger::debug(TAG, &format!("clear_email_cache: start dir={}", dir.to_string_lossy()));
    let mut removed = 0usize;
    if dir.exists() {
        for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
                removed += 1;
            }
        }
    }
    state.cached_email_metas.lock().unwrap().clear();
    logger::debug(TAG, &format!("clear_email_cache: removed {} files", removed));
    Ok(())
}

/// 获取所有邮件 UID 列表（新→旧），供前端无限滚动用
#[tauri::command]
pub async fn fetch_email_uid_list(state: tauri::State<'_, crate::IslandState>) -> Result<Vec<u32>, String> {
    logger::debug(TAG, "fetch_email_uid_list: start");
    let config = state.email_config.lock().unwrap().clone();
    let uid_list = task::spawn_blocking(move || {
        if !config.is_configured() {
            logger::debug(TAG, "fetch_email_uid_list: not configured");
            return Err("邮箱未配置".to_string());
        }
        logger::debug(TAG, &format!("fetch_email_uid_list: connecting {}:{}", config.address, config.port));
        let tls = native_tls::TlsConnector::builder().build().map_err(|e| e.to_string())?;
        let client = imap::connect(
            (config.address.as_str(), config.port),
            config.address.as_str(), &tls,
        ).map_err(|e| e.to_string())?;
        logger::debug(TAG, "fetch_email_uid_list: logging in");
        let mut session = client.login(&config.username, &config.auth)
            .map_err(|(e, _)| e.to_string())?;
        logger::debug(TAG, "fetch_email_uid_list: selecting INBOX");
        session.select("INBOX").map_err(|e| e.to_string())?;
        logger::debug(TAG, "fetch_email_uid_list: uid_search ALL");
        let uids = session.uid_search("ALL").map_err(|e| e.to_string())?;
        session.logout().ok();
        let mut uid_list: Vec<u32> = uids.into_iter().collect();
        uid_list.sort_unstable();
        uid_list.reverse();
        Ok(uid_list)
    }).await.map_err(|e| e.to_string())??;
    logger::debug(TAG, &format!("fetch_email_uid_list: done, {} uids", uid_list.len()));
    Ok(uid_list)
}

#[tauri::command]
pub async fn fetch_email_metas_by_uids(state: tauri::State<'_, crate::IslandState>, uids: Vec<u32>) -> Result<Vec<EmailMeta>, String> {
    let config = state.email_config.lock().unwrap().clone();
    let metas = config.fetch_metas_by_uids(uids).await?;
    {
        let mut cached = state.cached_email_metas.lock().unwrap();
        upsert_email_metas(&mut cached, &metas);
        save_email_metas(&cached);
    }
    Ok(metas)
}

#[tauri::command]
pub async fn fetch_email_bodies_by_uids(state: tauri::State<'_, crate::IslandState>, uids: Vec<u32>) -> Result<Vec<u32>, String> {
    let config = state.email_config.lock().unwrap().clone();
    config.fetch_bodies_by_uids(uids).await
}

#[tauri::command]
pub async fn fetch_email_metas_and_bodies_by_uids(state: tauri::State<'_, crate::IslandState>, uids: Vec<u32>) -> Result<Vec<EmailMeta>, String> {
    let config = state.email_config.lock().unwrap().clone();
    let metas = config.fetch_metas_and_bodies_by_uids(uids).await?;
    {
        let mut cached = state.cached_email_metas.lock().unwrap();
        upsert_email_metas(&mut cached, &metas);
        save_email_metas(&cached);
    }
    Ok(metas)
}

#[tauri::command]
pub async fn fetch_email_body_by_uid(state: tauri::State<'_, crate::IslandState>, uid: u32) -> Result<bool, String> {
    let config = state.email_config.lock().unwrap().clone();
    let done = config.fetch_bodies_by_uids(vec![uid]).await?;
    Ok(done.contains(&uid))
}

#[tauri::command]
pub fn read_email_body_by_uid(uid: u32) -> Result<String, String> {
    let path = email_cache_dir().join(format!("{}.html", uid));
    logger::debug(TAG, &format!("read_email_body_by_uid: start uid={} path={}", uid, path.to_string_lossy()));
    let html = std::fs::read_to_string(&path).map_err(|e| {
        logger::debug(TAG, &format!("read_email_body_by_uid: failed uid={} error={}", uid, e));
        e.to_string()
    })?;
    logger::debug(TAG, &format!("read_email_body_by_uid: done uid={} bytes={}", uid, html.len()));
    Ok(html)
}
