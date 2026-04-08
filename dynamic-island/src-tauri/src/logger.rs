use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::OnceLock;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

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
                    }
                }
            })
            .ok();

        tx
    })
}

fn write_log(tag: &str, level: &str, message: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let line = format!("[{}][{}][{}] {}", tag, level, now, message);
    let _ = get_sender().send(LogMsg::Line(line));
}

pub fn info(tag: &str, message: &str) {
    write_log(tag, "INFO", message);
}

pub fn warn(tag: &str, message: &str) {
    write_log(tag, "WARN", message);
}
