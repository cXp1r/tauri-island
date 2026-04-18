use super::base_api::BaseApi;
use serde::Deserialize;
use std::collections::HashMap;

pub struct NeteaseApi {
    api: BaseApi,
}

impl NeteaseApi {
    fn netease_headers() -> HashMap<String, String> {
        let mut h = HashMap::new();
        h.insert("cookie".to_string(), super::base_api::COOKIE.to_string());
        h
    }

    pub fn new() -> Self {
        Self {
            api: BaseApi::new(Some("https://music.163.com/"), Some(Self::netease_headers())),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            api: BaseApi::with_client(client, Some("https://music.163.com/"), Some(Self::netease_headers())),
        }
    }

    /// 搜索歌曲
    pub async fn search(&self, keyword: &str, search_type: i32) -> Result<SearchResult, reqwest::Error> {
        let mut params = HashMap::new();
        params.insert("s".to_string(), keyword.to_string());
        params.insert("type".to_string(), search_type.to_string());
        params.insert("limit".to_string(), "20".to_string());
        params.insert("offset".to_string(), "0".to_string());

        let resp = self.api.post_form_async(
            "https://music.163.com/api/search/get/web",
            &params,
        ).await?;


        let parsed: SearchResult = serde_json::from_str(&resp).unwrap_or(SearchResult { code: -1, result: None });


        Ok(parsed)
    }

    /// 获取歌词
    pub async fn get_lyric(&self, id: &str) -> Result<LyricResult, reqwest::Error> {
        let mut params = HashMap::new();
        params.insert("id".to_string(), id.to_string());
        params.insert("lv".to_string(), "-1".to_string());
        params.insert("kv".to_string(), "-1".to_string());
        params.insert("tv".to_string(), "-1".to_string());
        params.insert("rv".to_string(), "-1".to_string());
        params.insert("yv".to_string(), "-1".to_string());
        params.insert("ytv".to_string(), "-1".to_string());
        params.insert("yrv".to_string(), "-1".to_string());


        let resp = self.api.post_form_async(
            "https://interface3.music.163.com/api/song/lyric/v1",
            &params,
        ).await?;

        let parsed: LyricResult = serde_json::from_str(&resp).unwrap_or(LyricResult::default());


        Ok(parsed)
    }

    /// 获取歌曲详情
    pub async fn get_detail(&self, ids: &[&str]) -> Result<DetailResult, reqwest::Error> {
        let c: Vec<serde_json::Value> = ids.iter().map(|id| {
            serde_json::json!({ "id": id })
        }).collect();

        let mut params = HashMap::new();
        params.insert("c".to_string(), serde_json::to_string(&c).unwrap_or_default());

        let resp = self.api.post_form_async(
            "https://music.163.com/api/v3/song/detail",
            &params,
        ).await?;

        Ok(serde_json::from_str(&resp).unwrap_or(DetailResult { songs: vec![], code: -1 }))
    }
}

impl Default for NeteaseApi {
    fn default() -> Self {
         Self::new()
    }
}
// ===== Response Models =====

#[derive(Debug, Deserialize, Default)]
pub struct SearchResult {
    pub code: i64,
    pub result: Option<SearchResultData>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultData {
    pub songs: Option<Vec<Song>>,
    pub song_count: Option<i64>,
    pub albums: Option<Vec<Album>>,
    pub album_count: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LyricResult {
    pub code: Option<i64>,
    pub nolyric: Option<bool>,
    pub uncollected: Option<bool>,
    pub lrc: Option<Lyrics>,
    pub klyric: Option<Lyrics>,
    pub tlyric: Option<Lyrics>,
    pub romalrc: Option<Lyrics>,
    pub yrc: Option<Lyrics>,
    pub ytlrc: Option<Lyrics>,
    pub yromalrc: Option<Lyrics>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Lyrics {
    pub version: Option<i64>,
    pub lyric: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DetailResult {
    pub songs: Vec<Song>,
    pub code: i64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub name: Option<String>,
    pub id: Option<serde_json::Value>,
    #[serde(alias = "ar")]
    pub artists: Option<Vec<Ar>>,
    #[serde(alias = "al")]
    pub album: Option<Al>,
    #[serde(alias = "dt")]
    pub duration: Option<i64>,
    pub publish_time: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Ar {
    pub id: Option<i64>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Al {
    pub id: Option<i64>,
    pub name: Option<String>,
    #[serde(rename = "picUrl")]
    pub pic_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Album {
    pub name: Option<String>,
    pub id: Option<i64>,
    pub size: Option<i64>,
    pub artist: Option<Artist>,
}

#[derive(Debug, Deserialize)]
pub struct Artist {
    pub name: Option<String>,
    pub id: Option<i64>,
}
