pub(crate) fn extract_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for word in text.split(|c: char| {
        c.is_whitespace()
            || c == '"'
            || c == '\''
            || c == '<'
            || c == '>'
            || c == '('
            || c == ')'
            || c == '['
            || c == ']'
            || c == '{'
            || c == '}'
            || c == '|'
    }) {
        let w = word.trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == '!' || c == '?');
        if (w.starts_with("http://") || w.starts_with("https://")) && w.len() > 10 {
            if let Ok(parsed) = url::Url::parse(w) {
                if parsed.host().is_some() {
                    urls.push(w.to_string());
                }
            }
        }
    }
    urls.dedup();
    urls
}

pub(crate) fn read_clipboard_text() -> Option<String> {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, GetClipboardData, IsClipboardFormatAvailable};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    unsafe {
        if IsClipboardFormatAvailable(13).is_err() { return None; } // CF_UNICODETEXT = 13
        if OpenClipboard(None).is_err() { return None; }
        let h = GetClipboardData(13); // CF_UNICODETEXT
        let result = if let Ok(h) = h {
            let ptr = GlobalLock(HGLOBAL(h.0));
            if ptr.is_null() {
                None
            } else {
                let wide_ptr = ptr as *const u16;
                let mut len = 0;
                while *wide_ptr.add(len) != 0 { len += 1; }
                let slice = std::slice::from_raw_parts(wide_ptr, len);
                let text = String::from_utf16_lossy(slice);
                GlobalUnlock(HGLOBAL(h.0)).ok();
                Some(text.trim().to_string())
            }
        } else {
            None
        };
        CloseClipboard().ok();
        result
    }
}
