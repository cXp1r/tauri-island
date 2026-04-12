use async_trait::async_trait;
use crate::providers::soda_music::SodaMusicApi;
use crate::models::ITrackMetadata;
use super::{ISearcher, ISearchResult, SearcherType};

pub struct SodaMusicSearcher {
    api: SodaMusicApi,
}

impl SodaMusicSearcher {
    pub fn new() -> Self {
        Self { api: SodaMusicApi::new() }
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
                                    let duration = track.duration.map(|d| d as i32);
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

    async fn make_search_string(&self, track: &dyn ITrackMetadata) -> Option<String> {
        let combined = format!(
            "{}",
            track.title().unwrap_or_default()
        ).replace(" - ", " ").trim().to_string();

        if combined.is_empty() {
            None
        } else {
            Some(combined)
        }
    }

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
        let artists: Vec<String> = track
            .artist()
            .unwrap_or_default()   // 👈 关键
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        for a in &artists {
            if result.artists().iter().any(|b| {
                let b = b.to_lowercase();
                a == &b || a.contains(&b) || b.contains(a)
            }) {
                score += 1;
            }
        }

        //duration_ms
        if let Some(duration_ms) = track.duration_ms() {
            if let Some(result_duration_ms) = result.duration_ms() {
                let diff = (duration_ms - result_duration_ms).abs();
                if diff == 0 { // 完全匹配
                    
                    score += 2;
                }else if diff <= 1000 { // 1秒内认为时长匹配
                    score += 1;
                }
                
            }
        }
        score
    }
}

pub struct SodaMusicSearchResult {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: Option<i32>,
    pub match_score: i8,
}

impl ISearchResult for SodaMusicSearchResult {
    fn title(&self) -> &str { &self.title }
    fn artists(&self) -> &[String] { &self.artists }
    fn album(&self) -> &str { &self.album }
    fn duration_ms(&self) -> Option<i32> { self.duration_ms }
    fn match_score(&self) -> i8 { self.match_score }
    fn set_match_score(&mut self, score: i8) { self.match_score = score; }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

//bro懂我的测试
#[cfg(test)]
mod tests {
    use super::*;
    use crate::searchers::soda_music::SodaMusicSearcher;
    use crate::models::TrackMetadata;
    #[tokio::test]
async fn test_soda_music_search_for_duration_debug() {
    let searcher = SodaMusicSearcher::new();

    let metadata = TrackMetadata {
        title: Some("只对你有感觉(DJAh Remix）".to_string()),
        artist: Some("DJAh".to_string()),
        album: Some("".to_string()),
        album_artist: Some("".to_string()),
        duration_ms: Some(168750),
        ..Default::default()
    };

    let Some(search_string) = searcher.make_search_string(&metadata).await else {
        return;
    };
    println!("search string = {}", search_string);
    let result = searcher
        .search_for_results_by_string(&search_string)
        .await;

    match result {
        Ok(mut list) => {

            for item in list.iter_mut() {
                let mt = searcher.compare_track(&metadata, item.as_ref());
                item.set_match_score(mt);
            }


            list.sort_by(|a, b| {
                let a_score = a.match_score() as i8;
                let b_score = b.match_score() as i8;
                b_score.cmp(&a_score)
            });

            println!("result count = {}", list.len());

            for (i, item) in list.iter().enumerate() {
                println!("--- item {} ---", i);
                println!("title = {}", item.title());
                println!("artists = {:?}", item.artists());
                println!("album = {}", item.album());
                println!("duration_ms = {:?}", item.duration_ms());
                println!("match_score = {}", item.match_score());
            }
        }
        Err(e) => {
            panic!("search failed: {:?}", e);
        }
    }
}
}