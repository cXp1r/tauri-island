use serde::{Deserialize, Serialize};
use super::syllable_info::{Syllable, concatenate_syllables};

/// 歌词对齐方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LyricsAlignment {
    Unspecified,
    Left,
    Right,
    Center,
}

impl Default for LyricsAlignment {
    fn default() -> Self {
        LyricsAlignment::Unspecified
    }
}

/// 歌词行枚举，统一存储不同类型的行
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LineInfo {
    /// 基本行 (逐行同步)
    Basic(BasicLineInfo),
    /// 音节行 (逐字同步)
    Syllable(SyllableLineInfo),
    /// 完整行 (含翻译/发音)
    Full(FullLineInfo),
    /// 完整音节行 (含翻译/发音)
    FullSyllable(FullSyllableLineInfo),
}

impl LineInfo {
    pub fn text(&self) -> String {
        match self {
            LineInfo::Basic(l) => l.text.clone(),
            LineInfo::Syllable(l) => concatenate_syllables(&l.syllables),
            LineInfo::Full(l) => l.text.clone(),
            LineInfo::FullSyllable(l) => concatenate_syllables(&l.syllables),
        }
    }

    pub fn start_time(&self) -> Option<i32> {
        match self {
            LineInfo::Basic(l) => l.start_time,
            LineInfo::Syllable(l) => l.syllables.first().and_then(|s| s.start_time()),
            LineInfo::Full(l) => l.start_time,
            LineInfo::FullSyllable(l) => l.syllables.first().and_then(|s| s.start_time()),
        }
    }

    pub fn end_time(&self) -> Option<i32> {
        match self {
            LineInfo::Basic(l) => l.end_time,
            LineInfo::Syllable(l) => l.syllables.last().and_then(|s| s.end_time()),
            LineInfo::Full(l) => l.end_time,
            LineInfo::FullSyllable(l) => l.syllables.last().and_then(|s| s.end_time()),
        }
    }

    pub fn duration(&self) -> Option<i32> {
        match (self.start_time(), self.end_time()) {
            (Some(s), Some(e)) => Some(e - s),
            _ => None,
        }
    }

    pub fn alignment(&self) -> LyricsAlignment {
        match self {
            LineInfo::Basic(l) => l.lyrics_alignment,
            LineInfo::Syllable(l) => l.lyrics_alignment,
            LineInfo::Full(l) => l.lyrics_alignment,
            LineInfo::FullSyllable(l) => l.lyrics_alignment,
        }
    }

    pub fn sub_line(&self) -> Option<&Box<LineInfo>> {
        match self {
            LineInfo::Basic(l) => l.sub_line.as_ref(),
            LineInfo::Syllable(l) => l.sub_line.as_ref(),
            LineInfo::Full(l) => l.sub_line.as_ref(),
            LineInfo::FullSyllable(l) => l.sub_line.as_ref(),
        }
    }

    pub fn sub_line_mut(&mut self) -> &mut Option<Box<LineInfo>> {
        match self {
            LineInfo::Basic(l) => &mut l.sub_line,
            LineInfo::Syllable(l) => &mut l.sub_line,
            LineInfo::Full(l) => &mut l.sub_line,
            LineInfo::FullSyllable(l) => &mut l.sub_line,
        }
    }

    /// 完整文本 (含子行)
    pub fn full_text(&self) -> String {
        let text = self.text();
        if let Some(sub) = self.sub_line() {
            let sub_text = sub.text();
            if sub_text.is_empty() {
                text
            } else {
                format!("{}({})", text, sub_text)
            }
        } else {
            text
        }
    }

    /// 考虑子行的起始时间
    pub fn start_time_with_sub_line(&self) -> Option<i32> {
        let main = self.start_time();
        if let Some(sub) = self.sub_line() {
            let sub_start = sub.start_time();
            match (main, sub_start) {
                (Some(m), Some(s)) => Some(m.min(s)),
                (Some(m), None) => Some(m),
                (None, Some(s)) => Some(s),
                _ => None,
            }
        } else {
            main
        }
    }

    /// 考虑子行的结束时间
    pub fn end_time_with_sub_line(&self) -> Option<i32> {
        let main = self.end_time();
        if let Some(sub) = self.sub_line() {
            let sub_end = sub.end_time();
            match (main, sub_end) {
                (Some(m), Some(s)) => Some(m.max(s)),
                (Some(m), None) => Some(m),
                (None, Some(s)) => Some(s),
                _ => None,
            }
        } else {
            main
        }
    }

    /// 获取音节列表（如果是音节行的话）
    pub fn syllables(&self) -> Option<&Vec<Syllable>> {
        match self {
            LineInfo::Syllable(l) => Some(&l.syllables),
            LineInfo::FullSyllable(l) => Some(&l.syllables),
            _ => None,
        }
    }

    pub fn syllables_mut(&mut self) -> Option<&mut Vec<Syllable>> {
        match self {
            LineInfo::Syllable(l) => Some(&mut l.syllables),
            LineInfo::FullSyllable(l) => Some(&mut l.syllables),
            _ => None,
        }
    }

    /// 获取翻译 (FullLineInfo / FullSyllableLineInfo)
    pub fn translation(&self) -> Option<&str> {
        match self {
            LineInfo::Full(l) => l.translation.as_deref(),
            LineInfo::FullSyllable(l) => l.translation.as_deref(),
            _ => None,
        }
    }

    /// 获取发音 (FullLineInfo / FullSyllableLineInfo)
    pub fn pronunciation(&self) -> Option<&str> {
        match self {
            LineInfo::Full(l) => l.pronunciation.as_deref(),
            LineInfo::FullSyllable(l) => l.pronunciation.as_deref(),
            _ => None,
        }
    }
}

/// 基本行信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BasicLineInfo {
    pub text: String,
    pub start_time: Option<i32>,
    pub end_time: Option<i32>,
    pub lyrics_alignment: LyricsAlignment,
    pub sub_line: Option<Box<LineInfo>>,
}

/// 音节行信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyllableLineInfo {
    pub syllables: Vec<Syllable>,
    pub lyrics_alignment: LyricsAlignment,
    pub sub_line: Option<Box<LineInfo>>,
}

/// 完整行信息 (含翻译/发音)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FullLineInfo {
    pub text: String,
    pub start_time: Option<i32>,
    pub end_time: Option<i32>,
    pub lyrics_alignment: LyricsAlignment,
    pub sub_line: Option<Box<LineInfo>>,
    pub translation: Option<String>,
    pub pronunciation: Option<String>,
}

/// 完整音节行信息 (含翻译/发音)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FullSyllableLineInfo {
    pub syllables: Vec<Syllable>,
    pub lyrics_alignment: LyricsAlignment,
    pub sub_line: Option<Box<LineInfo>>,
    pub translation: Option<String>,
    pub pronunciation: Option<String>,
}

impl From<SyllableLineInfo> for LineInfo {
    fn from(val: SyllableLineInfo) -> Self {
        LineInfo::Syllable(val)
    }
}

impl From<BasicLineInfo> for LineInfo {
    fn from(val: BasicLineInfo) -> Self {
        LineInfo::Basic(val)
    }
}

impl From<FullLineInfo> for LineInfo {
    fn from(val: FullLineInfo) -> Self {
        LineInfo::Full(val)
    }
}

impl From<FullSyllableLineInfo> for LineInfo {
    fn from(val: FullSyllableLineInfo) -> Self {
        LineInfo::FullSyllable(val)
    }
}
