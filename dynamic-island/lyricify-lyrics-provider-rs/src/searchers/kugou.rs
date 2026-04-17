use async_trait::async_trait;
use crate::providers::kugou::KugouApi;
use super::{ISearcher, ISearchResult, SearcherType};
use crate::models::ITrackMetadata;
pub struct KugouSearcher {
    api: KugouApi,
}

impl KugouSearcher {
    pub fn new() -> Self {
        Self { api: KugouApi::new() }
    }
}
//酷狗音乐SMTC只提供title artist albumArtist? 
//duration只能api拿了
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
                        let artists: Vec<String> = singer.split('、')//酷狗用中文顿号分词
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let album = info.album_name.clone().unwrap_or_default();
                        let duration = info.duration.map(|d| (d * 1000) as u32);
                        let hash = info.hash.clone().unwrap_or_default();

                        results.push(Box::new(KugouSearchResult {
                            hash,
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

        Ok(results)
    }
    async fn make_search_string(&self, track: &dyn ITrackMetadata) -> Option<String> {
        let combined = format!(
            "{} {} {}",
            track.title().unwrap_or_default(),
            track.artist().unwrap_or_default(),
            track.album_artist().unwrap_or_default(),
        ).replace(" - ", " ").trim().to_string();

        if combined.is_empty() {
            None
        } else {
            Some(combined)
        }
    }
    fn min_score(&self) -> i8 { 5 }
    fn get_split_char(&self) -> char {
        '、'
    }
}

pub struct KugouSearchResult {
    pub hash: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: Option<u32>,
    pub match_score: i8,
}

impl ISearchResult for KugouSearchResult {
    fn title(&self) -> &str { &self.title }
    fn artists(&self) -> &[String] { &self.artists }
    fn album(&self) -> &str { &self.album }
    fn duration_ms(&self) -> Option<u32> { self.duration_ms }
    fn match_score(&self) -> i8 { self.match_score }
    fn set_match_score(&mut self, score: i8) { self.match_score = score; }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
