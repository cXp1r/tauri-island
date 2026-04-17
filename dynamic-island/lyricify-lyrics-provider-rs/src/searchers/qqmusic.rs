use async_trait::async_trait;
use crate::providers::qqmusic::QQMusicApi;
use super::{ISearcher, ISearchResult, SearcherType};

pub struct QQMusicSearcher {
    api: QQMusicApi,
}

impl QQMusicSearcher {
    pub fn new() -> Self {
        Self { api: QQMusicApi::new() }
    }
}

#[async_trait]
impl ISearcher for QQMusicSearcher {
    fn name(&self) -> &str { "QQMusic" }
    fn display_name(&self) -> &str { "QQ Music" }
    fn searcher_type(&self) -> SearcherType { SearcherType::QQMusic }

    async fn search_for_results_by_string(&self, search_string: &str) -> Result<Vec<Box<dyn ISearchResult>>, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.api.search(search_string).await?;
        
        let mut results: Vec<Box<dyn ISearchResult>> = Vec::new();
        if let Some(resp) = result{
            if let Some(req1) = resp.req_1 {
                if let Some(data) = req1.data {
                    if let Some(body) = data.body {
                        if let Some(song_list) = body.song {
                            if let Some(songs) = song_list.list {
                                for song in songs {
                                    let title = song.name.or(song.title).unwrap_or_default();
                                    let artists: Vec<String> = song.singer
                                        .unwrap_or_default()
                                        .iter()
                                        .filter_map(|s| s.name.clone())
                                        .collect();
                                    let album = song.album.as_ref().and_then(|a| a.name.clone()).unwrap_or_default();
                                    let duration = song.interval.map(|i| (i * 1000) as u32);
                                    let mid = song.mid.unwrap_or_default();
                                    let id  = song.id.unwrap_or_default();

                                    results.push(Box::new(QQMusicSearchResult {
                                        id,
                                        mid,
                                        title,
                                        artists,
                                        album,
                                        duration_ms: duration,
                                        match_score: 0,
                                    }));
                                    return Ok(results);
                                }
                            }
                        }
                        return Err("QQMusicApi: No song".into());
                    }
                    return Err("QQMusicApi: No body".into());
                }
                return Err("QQMusicApi: No data".into());
            }
            return Err("QQMusicApi: No req_1".into());      
        }
        return Err("QQMusicApi: No resp".into());    
    }
    fn get_split_char(&self) -> char {
        '/'
    }
}

pub struct QQMusicSearchResult {
    pub mid: String,
    pub id: u32,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: Option<u32>,
    pub match_score: i8,
}

impl ISearchResult for QQMusicSearchResult {
    fn title(&self) -> &str { &self.title }
    fn artists(&self) -> &[String] { &self.artists }
    fn album(&self) -> &str { &self.album }
    fn duration_ms(&self) -> Option<u32> { self.duration_ms }
    fn match_score(&self) -> i8 { self.match_score }
    fn set_match_score(&mut self, score: i8) { self.match_score = score; }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

