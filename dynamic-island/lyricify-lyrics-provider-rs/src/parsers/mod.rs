//pub mod attributes_helper;//整改了还没修
pub mod qqmusic;
pub mod netease;
pub mod soda_music;
pub mod kugou;
pub mod lrc;
pub mod decrypt;

use regex::{Captures, Regex};
use crate::models::*;
use std::sync::LazyLock;
pub static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(\d+),(\d+)\]([^\[\n]+)").unwrap()
});

pub static KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\(<](\d+),(\d+)(?:,\d+)?[\)>]").unwrap()
});

pub static LINE_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(\d+),(\d+)\](.+)").unwrap()
});

pub static WORD_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\(<](\d+),(\d+)(?:,[^[\)>]]+)?[\)>]").unwrap()
    //来者不拒~~
    //(st,et,x,x,x)
    //<st,et,x,x,x>
});

pub static SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?P<t>[^\n\u{E000}\u{E001}\]]+)\u{E000}(?P<s>\d+),(?P<d>\d+)\u{E001}").unwrap()//来者不拒~~
    //歌词 一号时间戳 二号时间戳
});

pub static PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\u{E000}(?P<s>\d+),(?P<d>\d+)(?:,[^[\)>]]+)?\u{E001}(?P<t>[^\u{E001}\u{E000}]+)").unwrap()//来者不拒~~
    //s:starttime d:duration t:text
});





//谨记一个循环里面不判断,代码多点性能好
pub trait IParsers {
    fn parse_line(&self, caps: Captures<'_>) -> Result<(u32, u32, String), String> {
        let t1 = caps
                        .get(1)
                        .ok_or("Sync Parser: Missing line start_time".to_string())?
                        .as_str()
                        .parse::<u32>()
                        .map_err(|_| "Sync Parser: Can't parse line start_time".to_string())?;

        let t2 = caps.
                        get(2)
                        .ok_or("Sync Parser: Missing line duration".to_string())?
                        .as_str()
                        .parse::<u32>()
                        .map_err(|_| "Sync Parser: Can't parse line duration".to_string())?;
        let text = caps
                            .get(3)
                            .ok_or("Sync Parser: Missing line lyrics")?
                            .as_str()
                            .to_string();
        Ok((t1, t2, text))
    }
    //依旧神秘变量名
    fn parse_syllables(&self, caps: Captures<'_>) -> Result<(u32, u32, String), String> {
        let t1 = caps
                        .name("s")
                        .ok_or("Sync Parser: Missing start_time".to_string())?
                        .as_str()
                        .parse::<u32>()
                        .map_err(|_| "Sync Parser: Can't parse start_time".to_string())?;

        let t2 = caps
                        .name("d")
                        .ok_or("Sync Parser: Missing duration".to_string())?
                        .as_str()
                        .parse::<u32>()
                        .map_err(|_| "Sync Parser: Can't parse duration".to_string())?;
        let text = caps
                            .name("t")
                            .ok_or("Sync Parser: Missing lyrics")?
                            .as_str()
                            .to_string();
        Ok((t1, t2, text))
    }
    
    fn get_line_re(&self) -> &Regex {
        &RE
    }

    fn get_syllables_re(&self) -> &Regex {
        &SUFFIX_RE
    }
    
    fn get_offset_time(&self, t1: u32, t2: u32) -> Result<u16, String> {
        let diff = t2
            .checked_sub(t1)
            .ok_or(format!("Parsers: overflow ({} {})", t1, t2))?;
        //u16够你offset用了
        u16::try_from(diff)
            .map_err(|_| format!("Parsers: offset overflow({})",diff))
    }

    fn parse(&self, lyrics: String) -> Result<Vec<LineInfo>, String> {
        use std::time::Instant;
        let start = Instant::now();

        let lyrics= KEY.replace_all(&lyrics, "\u{E000}$1,$2\u{E001}");
        
        let mut lineinfo: Vec<LineInfo> = Vec::new();
        for caps in self.get_line_re().captures_iter(&lyrics) {
            let (s, d, text) = self.parse_line(caps)?;
            let mut textinfo: Vec<TextInfo> = Vec::new();
            for caps2 in self.get_syllables_re().captures_iter(&text) {
                let (s1, d1, text1) = self.parse_syllables(caps2)?;
                textinfo.push(TextInfo {
                    start_time: self.get_offset_time(s,s1).map_err(|e| e)?,
                    duration: d1 as u16,
                    text: text1,
                });
            }
            lineinfo.push(LineInfo {
                start_time: s,
                duration: d as u16,
                text: String::new(),
                syllables: textinfo,
            });
        }

        let elapsed = start.elapsed();
        println!("解析歌词耗时耗时: {:?}", elapsed);


        Ok(lineinfo)
    }
}
