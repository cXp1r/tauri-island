use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{OnceLock, RwLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_FILE_PATH: OnceLock<PathBuf> = OnceLock::new();

// 0=DEBUG, 1=INFO, 2=WARN
static LOG_LEVEL: AtomicU8 = AtomicU8::new(1);
static LOG_FILTER_TAGS: OnceLock<RwLock<Vec<String>>> = OnceLock::new();
static LOG_FILTER_INVERT: AtomicU8 = AtomicU8::new(0);

fn level_to_u8(level: &str) -> u8 {
    match level {
        "DEBUG" => 0,
        "INFO"  => 1,
        "WARN"  => 2,
        "ERROR" => 3,
        _ => 1,
    }
}

pub fn set_level(level: &str) {
    let normalized = level.to_ascii_uppercase();
    LOG_LEVEL.store(level_to_u8(&normalized), Ordering::Relaxed);
}

pub fn get_level() -> String {
    match LOG_LEVEL.load(Ordering::Relaxed) {
        0 => "debug".to_string(),
        2 => "warn".to_string(),
        _ => "info".to_string(),
    }
}

pub fn set_filter(tags: Vec<String>, invert: bool) {
    let normalized: Vec<String> = tags
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect();
    *LOG_FILTER_TAGS.get_or_init(|| RwLock::new(Vec::new())).write().unwrap() = normalized;
    LOG_FILTER_INVERT.store(if invert { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn get_filter_tags() -> Vec<String> {
    LOG_FILTER_TAGS
        .get_or_init(|| RwLock::new(Vec::new()))
        .read()
        .unwrap()
        .clone()
}

pub fn get_filter_invert() -> bool {
    LOG_FILTER_INVERT.load(Ordering::Relaxed) != 0
}

pub fn log_file_path() -> Option<&'static PathBuf> {
    LOG_FILE_PATH.get()
}

enum LogMsg {
    Line(String),
}

fn get_sender() -> &'static Sender<LogMsg> {
    static SENDER: OnceLock<Sender<LogMsg>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<LogMsg>();

        let log_dir: PathBuf = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dynamic-island")
            .join("log");
        let _ = std::fs::create_dir_all(&log_dir);

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let file_path = log_dir.join(format!("island_{}.log", ts));
        let _ = LOG_FILE_PATH.set(file_path.clone());

        thread::Builder::new()
            .name("logger".into())
            .spawn(move || {
                let mut file: Option<File> = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&file_path)
                    .ok();

                for msg in rx {
                    let LogMsg::Line(line) = msg;
                    println!("{}", line);
                    if let Some(ref mut f) = file {
                        let _ = writeln!(f, "{}", line);
                        let _ = f.flush();
                    }
                }
            })
            .ok();

        tx
    })
}

fn write_log(tag: &str, level: &str, message: &str) {
    if level_to_u8(level) < LOG_LEVEL.load(Ordering::Relaxed) {
        return;
    }
    let filter_tags = LOG_FILTER_TAGS.get_or_init(|| RwLock::new(Vec::new())).read().unwrap();
    if !filter_tags.is_empty() {
        let matched = filter_tags.contains(&tag.to_string());
        let invert = LOG_FILTER_INVERT.load(Ordering::Relaxed) != 0;
        if (!invert && matched) || (invert && !matched) {
            return;
        }
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let line = format!("[{}][{}][{}] {}", tag, level, now, message);
    let _ = get_sender().send(LogMsg::Line(line));
}

pub fn debug(tag: &str, message: &str) {
    write_log(tag, "DEBUG", message);
}

pub fn info(tag: &str, message: &str) {
    write_log(tag, "INFO", message);
}

pub fn warn(tag: &str, message: &str) {
    write_log(tag, "WARN", message);
}

pub fn error(tag: &str, message: &str) {
    write_log(tag, "ERROR", message);
}

#[tauri::command]
pub fn get_log_level() -> String {
    get_level()
}

#[tauri::command]
pub fn get_log_level_num() -> u8 {
    LOG_LEVEL.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn set_log_level(level: String) {
    set_level(&level);
    info("Logger", &format!("日志等级已设为: {}", get_level()));
}

#[tauri::command]
pub fn get_log_path() -> String {
    // 触发初始化，确保路径已生成
    info("Logger", "get_log_path called");
    log_file_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "未知".to_string())
}

#[tauri::command]
pub fn open_log_dir() {
    // 触发初始化
    info("Logger", "open_log_dir called");
    if let Some(path) = log_file_path() {
        if let Some(dir) = path.parent() {
            let _ = std::process::Command::new("explorer")
                .arg(dir)
                .spawn();
        }
    }
}
