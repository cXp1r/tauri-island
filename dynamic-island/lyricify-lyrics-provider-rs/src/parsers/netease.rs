use crate::parsers::{IParsers, PREFIX_RE, lrc::*};
use regex::Regex;
use std::sync::LazyLock;
pub static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(\d+),(\d+)\]([^\[\n]+)").unwrap()
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
        &PREFIX_RE
    }
}
