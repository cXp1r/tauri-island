use crate::parsers::lrc::LrcParser;
use crate::parsers::{IParsers, decrypt::qrc::*};
use crate::models::*;
use memchr::memchr;
pub struct QQMusicLrcParser;
impl LrcParser for QQMusicLrcParser {}
pub struct QQMusicParser;
impl QQMusicParser {
    fn decrypt(&self, lyrics: &str) -> Result<String, String> {
        qrc_decrypt(lyrics)
    }
    pub fn decrypt_and_parse(&self, lyrics: String) -> Result<Vec<LineInfo>, String>  {
        let lyrics = self.decrypt(&lyrics)?;
        //println!("{}",lyrics);
        self.parse(lyrics)
    }
}
impl IParsers for QQMusicParser {
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
                continue;
            }

            let text_raw = content[cpos..cpos + lp].to_string();
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

            let Some(rp) = memchr(b')', &cbytes[cpos..]) else { break };
            cpos += rp + 1;

            result.push(TextInfo {
                start_time: self.get_offset_time(s, s1)?,
                duration: d1,
                text: text_raw,
            });
        }

    Ok(result)
}
}