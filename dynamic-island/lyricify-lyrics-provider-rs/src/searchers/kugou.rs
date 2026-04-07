use async_trait::async_trait;
use crate::providers::kugou::KugouApi;
use super::{ISearcher, ISearchResult, SearcherType};
use super::helpers::compare::MatchType;

pub struct KugouSearcher {
    api: KugouApi,
}

impl KugouSearcher {
    pub fn new() -> Self {
        Self { api: KugouApi::new() }
    }
}

#[async_trait]
impl ISearcher for KugouSearcher {
    fn name(&self) -> &str { "Kugou" }
    fn display_name(&self) -> &str { "Kugou Music" }
    fn searcher_type(&self) -> SearcherType { SearcherType::Kugou }

    async fn search_for_results_by_string(&self, search_string: &str) -> Result<Vec<Box<dyn ISearchResult>>, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.api.get_search_song(search_string).await?;
        let mut results: Vec<Box<dyn ISearchResult>> = Vec::new();

        if let Some(resp) = result {
            if let Some(data) = resp.data {
                if let Some(info_list) = data.info {
                    for info in info_list {
                        let title = info.song_name.clone().unwrap_or_default();
                        let singer = info.singer_name.clone().unwrap_or_default();
                        let artists: Vec<String> = singer.split('、')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let album = info.album_name.clone().unwrap_or_default();
                        let duration = info.duration.map(|d| d * 1000);
                        let hash = info.hash.clone().unwrap_or_default();

                        results.push(Box::new(KugouSearchResult {
                            hash,
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

        Ok(results)
    }
}

pub struct KugouSearchResult {
    pub hash: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: Option<i32>,
    pub match_type: Option<MatchType>,
}

impl ISearchResult for KugouSearchResult {
    fn title(&self) -> &str { &self.title }
    fn artists(&self) -> &[String] { &self.artists }
    fn album(&self) -> &str { &self.album }
    fn duration_ms(&self) -> Option<i32> { self.duration_ms }
    fn match_type(&self) -> Option<MatchType> { self.match_type }
    fn set_match_type(&mut self, mt: Option<MatchType>) { self.match_type = mt; }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
