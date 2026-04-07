use regex::Regex;
use once_cell::sync::Lazy;

/// 获取夹在两个字符串中间的字符串
pub fn between(s: &str, start: &str, end: &str) -> Option<String> {
    if let Some(start_idx) = s.find(start) {
        let after_start = &s[start_idx + start.len()..];
        if let Some(end_idx) = after_start.find(end) {
            Some(after_start[..end_idx].to_string())
        } else {
            Some(after_start.to_string())
        }
    } else {
        None
    }
}

/// 是否是数字字符串
pub fn is_number(s: &str) -> bool {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\d+$").unwrap());
    RE.is_match(s)
}
