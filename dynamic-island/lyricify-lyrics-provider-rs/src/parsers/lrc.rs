use crate::models::LineInfo;
use memchr::memchr;

pub trait LrcParser {
    fn parse_lrc_time(&self, tag: &str) -> Result<u32, String> {
        let tbytes = tag.as_bytes();
        // 找 ':'
        let Some(col) = memchr(b':', tbytes) else {
            return Err(format!("no ':' in time tag: {:?}", tag));
        };
        // 找 '.'
        let Some(dot) = memchr(b'.', tbytes) else {
            return Err(format!("no '.' in time tag: {:?}", tag));
        };

        let minutes = tag[..col].parse::<u32>()
            .map_err(|e| format!("minutes: {:?} raw={:?}", e, &tag[..col]))?;
        let seconds = tag[col + 1..dot].parse::<u32>()
            .map_err(|e| format!("seconds: {:?} raw={:?}", e, &tag[col+1..dot]))?;
        let centis = tag[dot + 1..].parse::<u32>()
            .map_err(|e| format!("centis: {:?} raw={:?}", e, &tag[dot+1..]))?;

        Ok(minutes * 60_000 + seconds * 1_000 + centis * 10)
    }

    fn parse(&self, lyrics: String) -> Result<Vec<LineInfo>, String> {
        let mut lineinfo: Vec<LineInfo> = Vec::new();
        let len = lyrics.len();
        let cbytes = lyrics.as_bytes();
        let mut c = 0;

        while c < len {
            let Some(lb) = memchr(b'[', &cbytes[c..]) else { break };
            c += lb + 1;

            if c >= len || !cbytes[c].is_ascii_digit() {
                // 跳过整个 [...] 块
                if let Some(rb) = memchr(b']', &cbytes[c..]) {
                    c += rb + 1;
                } else {
                    break;
                }
                continue;
            }

            // 解析 mm:ss.xx 格式时间戳 → 毫秒
            let Some(rb) = memchr(b']', &cbytes[c..]) else { break };
            let tag = &lyrics[c..c + rb];
            let s = self.parse_lrc_time(tag)?;
            c += rb + 1;

            // content 到下一个 '[' 或末尾，trim 换行
            let content_end = memchr(b'[', &cbytes[c..])
                .map(|x| c + x)
                .unwrap_or(len);
            let text = lyrics[c..content_end]
                .trim_matches(|ch| ch == '\r' || ch == '\n')
                .to_string();
            c = content_end;

            lineinfo.push(LineInfo {
                start_time: s,
                duration: 0, // LRC 格式没有 duration，后面可以用下一行的 start_time 补
                text,
                syllables: vec![],
            });
        }

        Ok(lineinfo)
    }

    
}