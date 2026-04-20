//pub mod attributes_helper;//整改了还没修
pub mod qqmusic;
pub mod netease;
pub mod soda_music;
pub mod kugou;
pub mod lrc;
pub mod decrypt;
use memchr::memchr;
use crate::models::*;
pub trait IParsers {

    fn get_offset_time(&self, t1: u32, t2: u32) -> Result<u16, String> {
        let diff = t2
            .checked_sub(t1)
            .ok_or(format!("Parsers: overflow ({} {})", t1, t2))?;
        //u16够你offset用了
        u16::try_from(diff)
            .map_err(|_| format!("Parsers: offset overflow({})",diff))
    }
    fn parse(&self, lyrics: String) -> Result<Vec<LineInfo>, String> {
        let start = std::time::Instant::now();
        let result = self.parse_without_re(lyrics);
        println!("parse took: {:?}", start.elapsed());
        result
    }
    fn parse_syllables(&self, s: u32, content: &str) -> Result<Vec<TextInfo>, String> {
        let cbytes = content.as_bytes();
        let clen = cbytes.len();
        let mut cpos = 0;
        let mut result: Vec<TextInfo> = Vec::new();

        while cpos < clen {
            // 找 '<'
            let Some(la) = memchr(b'<', &cbytes[cpos..]) else { break };

            let after_la = cpos + la + 1;
            if after_la >= clen || !cbytes[after_la].is_ascii_digit() {
                cpos += la + 1;
                continue;
            }
            cpos += la + 1;

            // s1
            let Some(c1) = memchr(b',', &cbytes[cpos..]) else { break };
            let s1 = content[cpos..cpos + c1]
                .parse::<u32>()
                .map_err(|e| format!("s1: {:?} raw={:?}", e, &content[cpos..cpos + c1]))?;
            cpos += c1 + 1;

            // d1，兼容 <s,d> 和 <s,d,x>
            let next_comma = memchr(b',', &cbytes[cpos..]).map(|x| cpos + x);
            let next_angle = memchr(b'>', &cbytes[cpos..]).map(|x| cpos + x);
            let d1_end = match (next_comma, next_angle) {
                (Some(nc), Some(na)) => nc.min(na),
                (Some(nc), None)     => nc,
                (None, Some(na))     => na,
                (None, None)         => break,
            };
            let d1 = content[cpos..d1_end]
                .parse::<u16>()
                .map_err(|e| format!("d1: {:?} raw={:?}", e, &content[cpos..d1_end]))?;

            // 跳到 '>' 后面
            let Some(ra) = memchr(b'>', &cbytes[cpos..]) else { break };
            cpos += ra + 1;

            // 文字在 '>' 到下一个 '<' 之间
            let text_end = memchr(b'<', &cbytes[cpos..])
                .map(|x| cpos + x)
                .unwrap_or(clen);
            let text_raw = content[cpos..text_end].to_string();
            cpos = text_end;

            result.push(TextInfo {
                start_time: self.get_offset_time(s, s1)?,
                duration: d1,
                text: text_raw,
            });
        }

        Ok(result)
    }
    

    
    fn parse_without_re(&self, lyrics: String) -> Result<Vec<LineInfo>, String> {
        let mut lineinfo: Vec<LineInfo> = Vec::new();
        let src = lyrics.as_bytes();
        let len = src.len();
        let mut pos = 0;

        while pos < len {
            // 1. 找 '['
            let Some(lb) = memchr(b'[', &src[pos..]) else { break };
            pos += lb + 1;

            // 2. tag1 必须是纯数字，否则跳过整个 [...]
            let Some(cm) = memchr(b',', &src[pos..]) else { break };
            let tag1_str = &lyrics[pos..pos + cm];
            if !tag1_str.bytes().all(|b| b.is_ascii_digit()) {
                if let Some(rb) = memchr(b']', &src[pos..]) {
                    pos += rb + 1;
                } else {
                    break;
                }
                continue;
            }
            let s = tag1_str.parse::<u32>().map_err(|e| e.to_string())?;
            pos += cm + 1;

            // 3. tag2 → d
            let Some(rb) = memchr(b']', &src[pos..]) else { break };
            let d = lyrics[pos..pos + rb]
                .parse::<u32>()
                .map_err(|e| format!("d parse error: {:?} raw={:?}", e, &lyrics[pos..pos + rb]))?;
            pos += rb + 1;

            // 4. content 到下一个 '[' 或末尾
            let content_end = memchr(b'[', &src[pos..])
                .map(|x| pos + x)
                .unwrap_or(len);
            let content = lyrics[pos..content_end].trim();
            pos = content_end;


            
            
            lineinfo.push(LineInfo {
                start_time: s,
                duration: d as u16,
                text: String::new(),
                syllables: self.parse_syllables(s, content)?,
            });
        }

        Ok(lineinfo)
    }

}

