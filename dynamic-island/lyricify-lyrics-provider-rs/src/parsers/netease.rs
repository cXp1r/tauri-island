use crate::parsers::{IParsers, lrc::*};
use regex::Regex;
use std::sync::LazyLock;
pub static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(\d+),(\d+)\]([^\[\n]+)").unwrap()
});

// Matches syllables AFTER KEY.replace_all has converted (s,d) → \u{E000}s,d\u{E001}
// Netease YRC prefix format: \u{E000}s,d\u{E001}text
pub static NETEASE_SYLLABLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("\u{E000}(?P<s>\\d+),(?P<d>\\d+)\u{E001}(?P<t>[^\n\u{E000}]+)").unwrap()
});

pub struct NeteaseLrcParser{
    pub version: u8,
}
impl LrcParser for NeteaseLrcParser {
    fn calc_timestamp(&self, t1: u32, t2: u32, t3: u32) -> u32 {
        match self.version {
            3 => t1 * 60000 + t2 * 1000 + t3,
            _ => t1 * 60000 + t2 * 1000 + t3*10
        }
    }
}


pub struct NeteaseParser;

impl IParsers for NeteaseParser {
    fn get_syllables_re(&self) -> &Regex {
        &NETEASE_SYLLABLE_RE
    }
}
