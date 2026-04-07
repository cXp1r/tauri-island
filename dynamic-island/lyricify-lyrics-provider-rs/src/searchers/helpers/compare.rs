use crate::models::ITrackMetadata;
use super::super::ISearchResult;

/// 匹配程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchType {
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Perfect = 4,
}

/// 比较曲目与搜索结果的匹配程度
pub fn compare_track(track: &dyn ITrackMetadata, result: &dyn ISearchResult) -> MatchType {
    let mut score = 0i32;

    // Name match
    let track_title = track.title().unwrap_or_default().to_lowercase();
    let result_title = result.title().to_lowercase();
    if !track_title.is_empty() && !result_title.is_empty() {
        if track_title == result_title {
            score += 4;
        } else if result_title.contains(&track_title) || track_title.contains(&result_title) {
            score += 2;
        } else {
            let clean_track = clean_title(&track_title);
            let clean_result = clean_title(&result_title);
            if clean_track == clean_result {
                score += 3;
            } else if clean_result.contains(&clean_track) || clean_track.contains(&clean_result) {
                score += 1;
            }
        }
    }

    // Artist match
    let track_artist = track.artist().unwrap_or_default().to_lowercase();
    let result_artist = result.artist().to_lowercase();
    if !track_artist.is_empty() && !result_artist.is_empty() {
        if track_artist == result_artist {
            score += 3;
        } else if result_artist.contains(&track_artist) || track_artist.contains(&result_artist) {
            score += 2;
        } else {
            // Check individual artists
            let track_artists: Vec<&str> = track_artist.split(|c: char| c == ',' || c == '/' || c == '&' || c == ';')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            let result_artists: Vec<String> = result.artists().iter().map(|s| s.to_lowercase()).collect();
            let mut any_match = false;
            for ta in &track_artists {
                for ra in &result_artists {
                    if ta == ra || ra.contains(ta) || ta.contains(ra.as_str()) {
                        any_match = true;
                        break;
                    }
                }
                if any_match { break; }
            }
            if any_match {
                score += 1;
            }
        }
    }

    // Duration match
    if let (Some(track_dur), Some(result_dur)) = (track.duration_ms(), result.duration_ms()) {
        let diff = (track_dur - result_dur).abs();
        if diff < 1000 {
            score += 2;
        } else if diff < 3000 {
            score += 1;
        }
    }

    // Album match
    let track_album = track.album().unwrap_or_default().to_lowercase();
    let result_album = result.album().to_lowercase();
    if !track_album.is_empty() && !result_album.is_empty() {
        if track_album == result_album {
            score += 1;
        }
    }

    match score {
        0 => MatchType::None,
        1..=3 => MatchType::Low,
        4..=6 => MatchType::Medium,
        7..=8 => MatchType::High,
        _ => MatchType::Perfect,
    }
}

fn clean_title(title: &str) -> String {
    let mut result = title.to_string();
    // Remove common suffixes in parentheses
    for pattern in &["(", "[", " - "] {
        if let Some(idx) = result.find(pattern) {
            result = result[..idx].trim().to_string();
        }
    }
    result
}
