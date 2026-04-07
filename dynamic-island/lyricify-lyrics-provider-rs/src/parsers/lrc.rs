use crate::models::*;
use crate::parsers::attributes_helper;
use regex::Regex;
use once_cell::sync::Lazy;

static TIMESTAMP_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[(\d+:\d+[.:]\d+)\]").unwrap());

/// 解析 LRC 歌词
pub fn parse(lyrics: &str) -> LyricsData {
    let mut data = LyricsData {
        track_metadata: Some(TrackMetadata::default()),
        file: Some(LyricsFileInfo {
            lyrics_type: LyricsTypes::Lrc,
            sync_type: SyncTypes::LineSynced,
            additional_info: Some(AdditionalFileInfo::General(GeneralAdditionalInfo {
                attributes: Vec::new(),
            })),
        }),
        ..Default::default()
    };

    let (offset, start_index) =
        attributes_helper::parse_general_attributes_from_string(&mut data, lyrics);

    let content = if start_index < lyrics.len() {
        &lyrics[start_index..]
    } else {
        ""
    };

    let lines = parse_lyrics(content);
    data.lines = lines;

    if let Some(off) = offset {
        if off != 0 {
            crate::helpers::offset_helper::add_offset_to_lines(&mut data.lines, off);
        }
    }

    data
}

/// 解析 LRC 歌词行
pub fn parse_lyrics(lyrics: &str) -> Vec<LineInfo> {
    let mut result: Vec<LineInfo> = Vec::new();

    for raw_line in lyrics.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let timestamps: Vec<i32> = TIMESTAMP_RE
            .captures_iter(line)
            .filter_map(|cap| {
                let ts_str = cap.get(1)?.as_str();
                parse_lrc_timestamp(ts_str)
            })
            .collect();

        if timestamps.is_empty() {
            continue;
        }

        let text = match line.rfind(']') {
            Some(pos) => line[pos + 1..].to_string(),
            None => String::new(),
        };

        for ts in &timestamps {
            result.push(LineInfo::Basic(BasicLineInfo {
                text: text.clone(),
                start_time: Some(*ts),
                end_time: None,
                lyrics_alignment: LyricsAlignment::Unspecified,
                sub_line: None,
            }));
        }
    }

    result.sort_by(|a, b| {
        let ta = a.start_time().unwrap_or(0);
        let tb = b.start_time().unwrap_or(0);
        ta.cmp(&tb)
    });

    for i in 0..result.len() {
        if i + 1 < result.len() {
            let next_start = result[i + 1].start_time();
            if let LineInfo::Basic(ref mut l) = result[i] {
                l.end_time = next_start;
            }
        }
    }

    result
}

/// 解析 LRC 时间戳字符串 "[mm:ss.xxx]" -> 毫秒
fn parse_lrc_timestamp(s: &str) -> Option<i32> {
    let s = s.replace(':', ":");
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }
    let minutes: i32 = parts[0].parse().ok()?;
    let sec_parts: Vec<&str> = if parts[1].contains('.') {
        parts[1].splitn(2, '.').collect()
    } else if parts[1].contains(':') {
        parts[1].splitn(2, ':').collect()
    } else {
        return Some(minutes * 60 * 1000 + parts[1].parse::<i32>().ok()? * 1000);
    };

    if sec_parts.len() != 2 {
        return None;
    }
    let seconds: i32 = sec_parts[0].parse().ok()?;
    let ms_str = sec_parts[1];
    let milliseconds: i32 = match ms_str.len() {
        1 => ms_str.parse::<i32>().ok()? * 100,
        2 => ms_str.parse::<i32>().ok()? * 10,
        3 => ms_str.parse::<i32>().ok()?,
        _ => ms_str[..3].parse::<i32>().ok()?,
    };

    Some(minutes * 60 * 1000 + seconds * 1000 + milliseconds)
}
