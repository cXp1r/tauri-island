use async_trait::async_trait;
use crate::providers::soda_music::SodaMusicApi;
use super::{ISearcher, ISearchResult, SearcherType};
use crate::searchers::ITrackMetadata;
pub struct SodaMusicSearcher {
    api: SodaMusicApi,
}

impl SodaMusicSearcher {
    pub fn new() -> Self {
        Self { api: SodaMusicApi::new() }
    }
}

impl Default for SodaMusicSearcher {
    fn default() -> Self {
        Self::new()
    }
}

//挨千刀的汽水只给两个arg
//还好给了时长...不像隔壁酷狗
//为了提高爆率,含泪加上时长匹配
#[async_trait]
impl ISearcher for SodaMusicSearcher {
    fn name(&self) -> &str { "SodaMusic" }
    fn display_name(&self) -> &str { "Soda Music" }
    fn searcher_type(&self) -> SearcherType { SearcherType::SodaMusic }

    async fn search_for_results_by_string(&self, search_string: &str) -> Result<Vec<Box<dyn ISearchResult>>, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.api.search(search_string).await?;
        let mut results: Vec<Box<dyn ISearchResult>> = Vec::new();

        if let Some(resp) = result {
            if let Some(groups) = resp.result_groups {
                for group in groups {
                    if let Some(items) = group.data {
                        for item in items {
                            if let Some(entity) = item.entity {
                                if let Some(track) = entity.track {
                                    let title = track.name.unwrap_or_default();
                                    let artists: Vec<String> = track.artists
                                        .unwrap_or_default()
                                        .iter()
                                        .filter_map(|a| a.name.clone())
                                        .collect();
                                    let album = track.album.as_ref().and_then(|a| a.name.clone()).unwrap_or_default();
                                    let duration = track.duration.map(|d| d as u32);
                                    let id = track.id.unwrap_or_default();
                                    results.push(Box::new(SodaMusicSearchResult {
                                        id,
                                        title,
                                        artists,
                                        album,
                                        duration_ms: duration,
                                        match_score: 0,
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    fn min_score(&self) -> i8 { 5 }
    fn get_split_char(&self) -> char {
        ','
    }
    async fn make_search_string(&self, track: &dyn ITrackMetadata) -> Option<String> {
        let combined = track.title().unwrap_or_default().to_string();

        if combined.is_empty() {
            None
        } else {
            Some(combined)
        }
    }
}

pub struct SodaMusicSearchResult {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: Option<u32>,
    pub match_score: i8,
}

impl ISearchResult for SodaMusicSearchResult {
    fn title(&self) -> &str { &self.title }
    fn artists(&self) -> &[String] { &self.artists }
    fn album(&self) -> &str { &self.album }
    fn duration_ms(&self) -> Option<u32> { self.duration_ms }
    fn match_score(&self) -> i8 { self.match_score }
    fn set_match_score(&mut self, score: i8) { self.match_score = score; }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

