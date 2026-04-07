use serde::{Deserialize, Serialize};

/// 音节信息 trait
pub trait ISyllableInfo {
    fn text(&self) -> &str;
    fn start_time(&self) -> Option<i32>;
    fn end_time(&self) -> Option<i32>;

    fn duration(&self) -> Option<i32> {
        match (self.start_time(), self.end_time()) {
            (Some(s), Some(e)) => Some(e - s),
            _ => None,
        }
    }
}

/// 音节信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyllableInfo {
    pub text: String,
    pub start_time: Option<i32>,
    pub end_time: Option<i32>,
}

impl Default for SyllableInfo {
    fn default() -> Self {
        Self {
            text: String::new(),
            start_time: None,
            end_time: None,
        }
    }
}

impl ISyllableInfo for SyllableInfo {
    fn text(&self) -> &str { &self.text }
    fn start_time(&self) -> Option<i32> { self.start_time }
    fn end_time(&self) -> Option<i32> { self.end_time }
}

/// 完整音节信息 (包含子项)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSyllableInfo {
    pub text: String,
    pub start_time: Option<i32>,
    pub end_time: Option<i32>,
    pub sub_items: Vec<SyllableInfo>,
}

impl Default for FullSyllableInfo {
    fn default() -> Self {
        Self {
            text: String::new(),
            start_time: None,
            end_time: None,
            sub_items: Vec::new(),
        }
    }
}

impl ISyllableInfo for FullSyllableInfo {
    fn text(&self) -> &str { &self.text }
    fn start_time(&self) -> Option<i32> { self.start_time }
    fn end_time(&self) -> Option<i32> { self.end_time }
}

/// 音节枚举，用于统一存储不同类型的音节
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Syllable {
    Basic(SyllableInfo),
    Full(FullSyllableInfo),
}

impl Syllable {
    pub fn text(&self) -> &str {
        match self {
            Syllable::Basic(s) => &s.text,
            Syllable::Full(s) => &s.text,
        }
    }

    pub fn start_time(&self) -> Option<i32> {
        match self {
            Syllable::Basic(s) => s.start_time,
            Syllable::Full(s) => s.start_time,
        }
    }

    pub fn end_time(&self) -> Option<i32> {
        match self {
            Syllable::Basic(s) => s.end_time,
            Syllable::Full(s) => s.end_time,
        }
    }

    pub fn duration(&self) -> Option<i32> {
        match (self.start_time(), self.end_time()) {
            (Some(s), Some(e)) => Some(e - s),
            _ => None,
        }
    }
}

/// 辅助函数：拼接音节文本
pub fn concatenate_syllables(syllables: &[Syllable]) -> String {
    syllables.iter().map(|s| s.text()).collect::<String>()
}
