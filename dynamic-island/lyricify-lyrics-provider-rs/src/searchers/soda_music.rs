use async_trait::async_trait;
use crate::providers::soda_music::SodaMusicApi;
use super::{ISearcher, ISearchResult, SearcherType};
use super::helpers::compare::MatchType;

pub struct SodaMusicSearcher {
    api: SodaMusicApi,
}

impl SodaMusicSearcher {
    pub fn new() -> Self {
        Self { api: SodaMusicApi::new() }
    }
}

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
                                    let duration = track.duration.map(|d| d as i32);
                                    let id = track.id.unwrap_or_default();

                                    results.push(Box::new(SodaMusicSearchResult {
                                        id,
                                        title,
                                        artists,
                                        album,
                                        duration_ms: duration,
                                        match_type: None,
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
}

pub struct SodaMusicSearchResult {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: Option<i32>,
    pub match_type: Option<MatchType>,
}

impl ISearchResult for SodaMusicSearchResult {
    fn title(&self) -> &str { &self.title }
    fn artists(&self) -> &[String] { &self.artists }
    fn album(&self) -> &str { &self.album }
    fn duration_ms(&self) -> Option<i32> { self.duration_ms }
    fn match_type(&self) -> Option<MatchType> { self.match_type }
    fn set_match_type(&mut self, mt: Option<MatchType>) { self.match_type = mt; }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
