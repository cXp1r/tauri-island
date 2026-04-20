use crate::parsers::{IParsers, lrc::*};
use memchr::{memchr, memchr2};
use crate::models::*;

pub struct NeteaseLrcParser{
    pub version: u8,
}
impl LrcParser for NeteaseLrcParser {
    fn parse_lrc_time(&self, tag: &str) -> Result<u32, String> {
        let tbytes = tag.as_bytes();

        // 找第一个 ':'
        let Some(col) = memchr(b':', tbytes) else {
            return Err(format!("no ':' in time tag: {:?}", tag));
        };

        let minutes = tag[..col]
            .parse::<u32>()
            .map_err(|e| format!("minutes: {:?} raw={:?}", e, &tag[..col]))?;

        // col 之后找 ':' 或 '.'，看哪个先出现来盲判格式
        let Some(sep) = memchr2(b':', b'.', &tbytes[col + 1..]) else {
            return Err(format!("no second separator in time tag: {:?}", tag));
        };
        let sep = col + 1 + sep; // 转绝对偏移

        let seconds = tag[col + 1..sep]
            .parse::<u32>()
            .map_err(|e| format!("seconds: {:?} raw={:?}", e, &tag[col+1..sep]))?;
        let centis = tag[sep + 1..]
            .parse::<u32>()
            .map_err(|e| format!("centis: {:?} raw={:?}", e, &tag[sep+1..]))?;

        // ':' → v3 毫秒直接用，'.' → v4 百分秒 *10
        match tbytes[sep] {
            b'.' => Ok(minutes * 60_000 + seconds * 1_000 + centis),
            _    => Ok(minutes * 60_000 + seconds * 1_000 + centis * 10),
        }
    }
}


pub struct NeteaseParser;

impl IParsers for NeteaseParser {
    fn parse_syllables(&self, s: u32, content: &str) -> Result<Vec<TextInfo>, String> {
        let cbytes = content.as_bytes();
        let clen = cbytes.len();
        let mut cpos = 0;
        let mut result: Vec<TextInfo> = Vec::new();

        while cpos < clen {
            let Some(lp) = memchr(b'(', &cbytes[cpos..]) else { break };

            let after_lp = cpos + lp + 1;
            if after_lp >= clen || !cbytes[after_lp].is_ascii_digit() {
                cpos += lp + 1;
                continue;//不是数字,你已飞升
            }
            cpos += lp + 1;

            // s1
            let Some(c1) = memchr(b',', &cbytes[cpos..]) else { break };
            let s1 = content[cpos..cpos + c1]
                .parse::<u32>()
                .map_err(|e| format!("s1: {:?} raw={:?}", e, &content[cpos..cpos + c1]))?;
            cpos += c1 + 1;

            // d1，兼容 (s,d,x)
            let next_comma = memchr(b',', &cbytes[cpos..]).map(|x| cpos + x);
            let next_paren = memchr(b')', &cbytes[cpos..]).map(|x| cpos + x);
            let d1_end = match (next_comma, next_paren) {
                (Some(nc), Some(np)) => nc.min(np),
                (Some(nc), None)     => nc,
                (None, Some(np))     => np,
                (None, None)         => break,
            };
            let d1 = content[cpos..d1_end]
                .parse::<u16>()
                .map_err(|e| format!("d1: {:?} raw={:?}", e, &content[cpos..d1_end]))?;

            // 跳到 ')' 后面
            let Some(rp) = memchr(b')', &cbytes[cpos..]) else { break };
            cpos += rp + 1;

            // 文字在 ')' 到下一个 '(' 之间
            let text_end = memchr(b'(', &cbytes[cpos..])
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
}
