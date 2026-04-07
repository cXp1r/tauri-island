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

        println!("[netease-api] search: keyword='{}' type={} limit=20 url='https://music.163.com/api/search/get/web'", keyword, search_type);

        let resp = self.api.post_form_async(
            "https://music.163.com/api/search/get/web",
            &params,
        ).await?;

        println!("[netease-api] search: response length={} bytes", resp.len());

        let parsed: SearchResult = serde_json::from_str(&resp).unwrap_or(SearchResult { code: -1, result: None });

        if let Some(ref data) = parsed.result {
            let song_count = data.song_count.unwrap_or(0);
            let songs_len = data.songs.as_ref().map(|s| s.len()).unwrap_or(0);
            println!("[netease-api] search: code={} song_count={} returned_songs={}", parsed.code, song_count, songs_len);
            if let Some(ref songs) = data.songs {
                for (i, song) in songs.iter().enumerate().take(5) {
                    let id_str = match &song.id {
                        Some(serde_json::Value::Number(n)) => n.to_string(),
                        Some(serde_json::Value::String(s)) => s.clone(),
                        _ => "?".to_string(),
                    };
                    let name = song.name.as_deref().unwrap_or("?");
                    let artists = song.artists.as_ref()
                        .map(|arr| arr.iter().filter_map(|a| a.name.as_deref()).collect::<Vec<_>>().join("/"))
                        .unwrap_or_default();
                    let album = song.album.as_ref().and_then(|a| a.name.as_deref()).unwrap_or("?");
                    let dur = song.duration.unwrap_or(0);
                    println!("[netease-api] search:   [{}] id={} name='{}' artists='{}' album='{}' duration={}ms", i, id_str, name, artists, album, dur);
                }
            }
        } else {
            println!("[netease-api] search: code={} result=None (no data)", parsed.code);
        }

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

        println!("[netease-api] get_lyric: id='{}' url='https://interface3.music.163.com/api/song/lyric/v1'", id);

        let resp = self.api.post_form_async(
            "https://interface3.music.163.com/api/song/lyric/v1",
            &params,
        ).await?;

        println!("[netease-api] get_lyric: response length={} bytes", resp.len());

        let parsed: LyricResult = serde_json::from_str(&resp).unwrap_or(LyricResult::default());

        println!("[netease-api] get_lyric: code={:?} nolyric={:?} uncollected={:?}", parsed.code, parsed.nolyric, parsed.uncollected);
        if let Some(ref lrc) = parsed.lrc {
            let len = lrc.lyric.as_ref().map(|s| s.len()).unwrap_or(0);
            let preview: String = lrc.lyric.as_deref().unwrap_or("").chars().take(120).collect();
            println!("[netease-api] get_lyric: lrc version={:?} length={} preview='{}'", lrc.version, len, preview);
        } else {
            println!("[netease-api] get_lyric: lrc=None");
        }
        if let Some(ref tlyric) = parsed.tlyric {
            let len = tlyric.lyric.as_ref().map(|s| s.len()).unwrap_or(0);
            println!("[netease-api] get_lyric: tlyric (translation) version={:?} length={}", tlyric.version, len);
        }
        if let Some(ref yrc) = parsed.yrc {
            let len = yrc.lyric.as_ref().map(|s| s.len()).unwrap_or(0);
            println!("[netease-api] get_lyric: yrc (word-level) version={:?} length={}", yrc.version, len);
        }

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
