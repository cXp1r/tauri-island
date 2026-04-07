use crate::models::*;
use crate::parsers::models::yrc_models::CreditsInfo;
use regex::Regex;
use once_cell::sync::Lazy;

static LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[(\d+),(\d+)\]").unwrap());
static SYLLABLE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\((\d+),(\d+),\d+\)([^(]*)").unwrap());

/// 解析 YRC 歌词
pub fn parse(lyrics: &str) -> LyricsData {
    let mut data = LyricsData {
        track_metadata: Some(TrackMetadata::default()),
        file: Some(LyricsFileInfo {
            lyrics_type: LyricsTypes::Yrc,
            sync_type: SyncTypes::SyllableSynced,
            additional_info: None,
        }),
        ..Default::default()
    };

    let lines = parse_lyrics(lyrics);
    data.lines = lines;
    data
}

/// 解析 YRC 歌词行
pub fn parse_lyrics(lyrics: &str) -> Vec<LineInfo> {
    let mut result: Vec<LineInfo> = Vec::new();

    for raw_line in lyrics.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('{') {
            if let Ok(_credits) = serde_json::from_str::<CreditsInfo>(line) {
                continue;
            }
        }

        let line_cap = match LINE_RE.captures(line) {
            Some(c) => c,
            None => continue,
        };

        let _line_start: i32 = match line_cap.get(1).and_then(|m| m.as_str().parse().ok()) {
            Some(v) => v,
            None => continue,
        };

        let syllables: Vec<Syllable> = SYLLABLE_RE
            .captures_iter(line)
            .filter_map(|cap| {
                let start: i32 = cap.get(1)?.as_str().parse().ok()?;
                let duration: i32 = cap.get(2)?.as_str().parse().ok()?;
                let text = cap.get(3)?.as_str().to_string();
                Some(Syllable::Basic(SyllableInfo {
                    text,
                    start_time: Some(start),
                    end_time: Some(start + duration),
                }))
            })
            .collect();

        if !syllables.is_empty() {
            result.push(LineInfo::Syllable(SyllableLineInfo {
                syllables,
                lyrics_alignment: LyricsAlignment::Unspecified,
                sub_line: None,
            }));
        }
    }

    result
}
