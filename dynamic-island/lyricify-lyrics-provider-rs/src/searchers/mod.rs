pub mod helpers;
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
    fn artist(&self) -> String {
        self.artists().join(", ")
    }
    fn album(&self) -> &str;
    fn album_artists(&self) -> Option<&[String]> { None }
    fn duration_ms(&self) -> Option<i32>;
    fn match_type(&self) -> Option<helpers::compare::MatchType>;
    fn set_match_type(&mut self, mt: Option<helpers::compare::MatchType>);
    fn as_any(&self) -> &dyn std::any::Any;
}

/// 搜索提供者 trait
#[async_trait]
pub trait ISearcher: Send + Sync {
    fn name(&self) -> &str;
    fn display_name(&self) -> &str;
    fn searcher_type(&self) -> SearcherType;

    async fn search_for_results_by_string(&self, search_string: &str) -> Result<Vec<Box<dyn ISearchResult>>, Box<dyn std::error::Error + Send + Sync>>;

    async fn search_for_results(&self, track: &dyn ITrackMetadata, full_search: bool) -> Result<Vec<Box<dyn ISearchResult>>, Box<dyn std::error::Error + Send + Sync>> {
        let search_string = format!(
            "{} {} {}",
            track.title().unwrap_or_default(),
            track.artist().unwrap_or_default().replace(", ", " "),
            track.album().unwrap_or_default()
        ).replace(" - ", " ").trim().to_string();

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
            let mt = helpers::compare::compare_track(track, result.as_ref());
            result.set_match_type(Some(mt));
        }

        // Sort by match type (descending)
        search_results.sort_by(|a, b| {
            let a_val = a.match_type().map(|m| m as i32).unwrap_or(0);
            let b_val = b.match_type().map(|m| m as i32).unwrap_or(0);
            b_val.cmp(&a_val)
        });

        Ok(search_results)
    }

    async fn search_for_result(&self, track: &dyn ITrackMetadata) -> Result<Option<Box<dyn ISearchResult>>, Box<dyn std::error::Error + Send + Sync>> {
        let search = self.search_for_results(track, false).await?;
        if !search.is_empty() {
            return Ok(Some(search.into_iter().next().unwrap()));
        }
        let search = self.search_for_results(track, true).await?;
        Ok(search.into_iter().next())
    }
}
