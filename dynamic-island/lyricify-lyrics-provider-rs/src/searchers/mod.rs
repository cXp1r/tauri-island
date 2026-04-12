pub mod netease;
pub mod qqmusic;
pub mod kugou;
pub mod soda_music;

use async_trait::async_trait;
use crate::models::ITrackMetadata;



/// 搜索源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearcherType {
    Netease,
    QQMusic,
    Kugou,
    SodaMusic,
}

/// 搜索结果 trait
pub trait ISearchResult: Send + Sync {
    fn title(&self) -> &str;
    fn artists(&self) -> &[String];
    fn album(&self) -> &str;
    fn album_artists(&self) -> Option<&[String]> { None }
    fn duration_ms(&self) -> Option<i32>;
    fn match_score(&self) -> i8;
    fn set_match_score(&mut self, mt: i8);
    fn as_any(&self) -> &dyn std::any::Any;
}

/// 搜索提供者 trait
#[async_trait]
pub trait ISearcher: Send + Sync {
    fn name(&self) -> &str;
    fn display_name(&self) -> &str;
    fn searcher_type(&self) -> SearcherType;

    async fn search_for_results_by_string(&self, search_string: &str) -> Result<Vec<Box<dyn ISearchResult>>, Box<dyn std::error::Error + Send + Sync>>;

    
    async fn make_search_string(&self, track: &dyn ITrackMetadata) -> Option<String> {
        let combined = format!(
            "{} {} {}",
            track.title().unwrap_or_default(),
            track.artist().unwrap_or_default(),
            track.album().unwrap_or_default(),
        ).replace(" - ", " ").trim().to_string();

        if combined.is_empty() {
            None
        } else {
            Some(combined)
        }
    }
    //下面那个函数调用了这个
    async fn search_for_results(&self, track: &dyn ITrackMetadata, full_search: bool) -> Result<Vec<Box<dyn ISearchResult>>, Box<dyn std::error::Error + Send + Sync>> {
        let search_string: String = match self.make_search_string(track).await {
            Some(s) => s,
            _ => return Ok(vec![]),
        };

        let mut search_results: Vec<Box<dyn ISearchResult>> = Vec::new();

        let mut level = 1;
        let mut current_search = search_string.clone();

        loop {
            if let Ok(results) = self.search_for_results_by_string(&current_search).await {
                search_results.extend(results);
            }

            let mut new_title = track.title().unwrap_or_default().to_string();
            if let Some(idx) = new_title.find("(feat.") {
                new_title = new_title[..idx].trim().to_string();
            }
            if let Some(idx) = new_title.find(" - feat.") {
                new_title = new_title[..idx].trim().to_string();
            }

            if full_search || search_results.is_empty() {
                let new_search = match level {
                    1 => format!("{} {}", new_title, track.artist().unwrap_or_default().replace(", ", " ")).replace(" - ", " ").trim().to_string(),
                    2 => new_title.replace(" - ", " ").trim().to_string(),
                    _ => String::new(),
                };
                if new_search != current_search && !new_search.is_empty() {
                    current_search = new_search;
                } else {
                    break;
                }
            } else {
                break;
            }

            level += 1;
            if level >= 3 {
                break;
            }
        }

        // Set match types
        for result in search_results.iter_mut() {
            let mt = self.compare_track(track, result.as_ref());
            result.set_match_score(mt);
        }

        // Sort by match type (descending)
        search_results.sort_by(|a, b| {
            let a_val = a.match_score();
            let b_val = b.match_score();
            b_val.cmp(&a_val)
        });

        Ok(search_results)
    }

    //smtc统一接口调用了这个
    async fn search_for_result(&self, track: &dyn ITrackMetadata) -> Result<Option<Box<dyn ISearchResult>>, Box<dyn std::error::Error + Send + Sync>> {
        let search = self.search_for_results(track, false).await?;
        if !search.is_empty() {
            return Ok(Some(search.into_iter().next().unwrap()));
        }
        let search = self.search_for_results(track, true).await?;
        Ok(search.into_iter().next())
    }

    /// 比较曲目与搜索结果的匹配程度（默认通用实现，各 searcher 可 override）
    fn compare_track(&self, track: &dyn ITrackMetadata, result: &dyn ISearchResult) -> i8 {
        let mut score = 0i8;

        // Name match
        let track_title = track.title().unwrap_or_default().to_lowercase();
        let result_title = result.title().to_lowercase();
        if !track_title.is_empty() && !result_title.is_empty() {
            if track_title == result_title {
                score += 4;
            } else if result_title.contains(&track_title) || track_title.contains(&result_title) {
                score += 2;
            } else {
                let clean_track = self.clean_title(&track_title);
                let clean_result = self.clean_title(&result_title);
                if clean_track == clean_result {
                    score += 3;
                } else if clean_result.contains(&clean_track) || clean_track.contains(&clean_result) {
                    score += 1;
                }
            }
        }

        // Artist match
        let track_artist = track.artist().unwrap_or_default().to_lowercase();
        let result_artists = result.artists().to_vec();
        if !track_artist.is_empty() && !result_artists.is_empty() {
            for result_artist in result_artists{
                if result_artist == track_artist {
                    score += 2;
                    
                } else if result_artist.contains(&track_artist) {
                    score += 1;
                }
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

        // Album artist match
        let track_album_artist = self.clean_title(&track.album_artist().unwrap_or_default().to_lowercase());
        let result_album_artist = result.album_artists().unwrap_or_default().to_vec();

        if result_album_artist.iter().any(|s:&String| s.contains(&track_album_artist)) {
            score += 1;
        }

        score
    }

    /// 清理标题中的常见符号（供 compare_track 使用，可 override）
    fn clean_title(&self, title: &str) -> String {
        let mut result = title.to_string();
        for pattern in &["(", "[", " - "] {
            if let Some(idx) = result.find(pattern) {
                result = result[..idx].trim().to_string();
            }
        }
        result = result
            .chars()
            .filter(|c| {
                !matches!(
                    c,
                    '《' | '》' | '「' | '」' | '『' | '』' |
                    '！' | '!' | '？' | '?' | '。' | '、' |
                    '·' | '•' | '…'
                )
            })
            .collect();
        result.trim().to_string()
    }
}
