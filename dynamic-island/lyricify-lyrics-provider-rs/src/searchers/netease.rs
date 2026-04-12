use async_trait::async_trait;
use crate::providers::netease::NeteaseApi;
use super::{ISearcher, ISearchResult, SearcherType};

pub struct NeteaseSearcher {
    api: NeteaseApi,
}

impl NeteaseSearcher {
    pub fn new() -> Self {
        Self { api: NeteaseApi::new() }
    }
}

#[async_trait]
impl ISearcher for NeteaseSearcher {
    fn name(&self) -> &str { "Netease" }
    fn display_name(&self) -> &str { "NetEase Cloud Music" }
    fn searcher_type(&self) -> SearcherType { SearcherType::Netease }

    async fn search_for_results_by_string(&self, search_string: &str) -> Result<Vec<Box<dyn ISearchResult>>, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.api.search(search_string, 1).await?;
        let mut results: Vec<Box<dyn ISearchResult>> = Vec::new();

        if let Some(data) = result.result {
            if let Some(songs) = data.songs {
                for song in songs {
                    let title = song.name.unwrap_or_default();
                    let artists: Vec<String> = song.artists
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|a| a.name.clone())
                        .collect();
                    let album = song.album.as_ref().and_then(|a| a.name.clone()).unwrap_or_default();
                    let duration = song.duration.map(|d| d as i32);
                    let id = match &song.id {
                        Some(serde_json::Value::Number(n)) => n.to_string(),
                        Some(serde_json::Value::String(s)) => s.clone(),
                        _ => String::new(),
                    };

                    results.push(Box::new(NeteaseSearchResult {
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

        Ok(results)
    }
}

pub struct NeteaseSearchResult {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: Option<i32>,
    pub match_score: i8,
}

impl ISearchResult for NeteaseSearchResult {
    fn title(&self) -> &str { &self.title }
    fn artists(&self) -> &[String] { &self.artists }
    fn album(&self) -> &str { &self.album }
    fn duration_ms(&self) -> Option<i32> { self.duration_ms }
    fn match_score(&self) -> i8 { self.match_score }
    fn set_match_score(&mut self, score: i8) { self.match_score = score; }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
