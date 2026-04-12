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
                        let duration = info.duration.map(|d| d * 1000);
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
        if !track_artist.is_empty() {
            for result_artist in result.artists() {
                let result_artist = result_artist.to_lowercase();

                if result_artist == track_artist {
                    score += 2;
                    break;
                } else if result_artist.contains(&track_artist) || track_artist.contains(&result_artist) {
                    score += 1;
                    break;
                }
            }
        }


        // Kugou albumArtist match
        let track_album_artist = self.clean_title(&track.album_artist()
            .unwrap_or_default()
            .to_lowercase());

        let result_album_artist = result.album();

        if result_album_artist.contains(&track_album_artist) || track_album_artist.contains(&result_album_artist) {
            score += 1;
        }

        score
    }
}

pub struct KugouSearchResult {
    pub hash: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub duration_ms: Option<i32>,
    pub match_score: i8,
}

impl ISearchResult for KugouSearchResult {
    fn title(&self) -> &str { &self.title }
    fn artists(&self) -> &[String] { &self.artists }
    fn album(&self) -> &str { &self.album }
    fn duration_ms(&self) -> Option<i32> { self.duration_ms }
    fn match_score(&self) -> i8 { self.match_score }
    fn set_match_score(&mut self, score: i8) { self.match_score = score; }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
//可以看到duration
//bro懂我的测试
#[cfg(test)]
mod tests {
    use super::*;
    use crate::searchers::kugou::KugouSearcher;
    use crate::models::TrackMetadata;
    #[tokio::test]
async fn test_kugou_search_for_duration_debug() {
    let searcher = KugouSearcher::new();

    let metadata = TrackMetadata {
        title: Some("Remember".to_string()),
        artist: Some("日本群星".to_string()),
        album: Some("".to_string()),
        album_artist: Some("《超かぐや姫！》".to_string()),
        ..Default::default()
    };

    let Some(search_string) = searcher.make_search_string(&metadata).await else {
        return;
    };

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