use super::base_api::BaseApi;
use serde::Deserialize;
use std::{collections::HashMap, sync::LazyLock};
use regex::Regex;
pub struct QQMusicApi {
    api: BaseApi,
}
static CDATA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"CDATA\[([0-9A-F]+)\]").unwrap()
});



impl QQMusicApi {
    pub fn new() -> Self {
        Self {
            api: BaseApi::new(Some("https://c.y.qq.com/"), None),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            api: BaseApi::with_client(client, Some("https://c.y.qq.com/"), None),
        }
    }

    /// 搜索歌曲
    pub async fn search(&self, keyword: &str) -> Result<Option<MusicFcgApiResult>, Box<dyn std::error::Error + Send + Sync>> {
        let data = serde_json::json!({
            "req_1": {
                "method": "DoSearchForQQMusicDesktop",
                "module": "music.search.SearchCgiService",
                "param": {
                    "num_per_page": "20",
                    "page_num": "1",
                    "query": keyword,
                    "search_type": 0
                }
            }
        });

        let resp = self.api.post_json_async("https://u.y.qq.com/cgi-bin/musicu.fcg", &data).await?;
        Ok(serde_json::from_str(&resp)?)
    }

    /// 获取歌词
    pub async fn get_lyric(&self, song_mid: &str) -> Result<Option<LyricResult>, Box<dyn std::error::Error + Send + Sync>> {
        let current_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        let callback = "MusicJsonCallback_lrc";
        let mut params = HashMap::new();
        params.insert("callback".to_string(), callback.to_string());
        params.insert("pcachetime".to_string(), current_millis.to_string());
        params.insert("songmid".to_string(), song_mid.to_string());
        params.insert("g_tk".to_string(), "5381".to_string());
        params.insert("jsonpCallback".to_string(), callback.to_string());
        params.insert("loginUin".to_string(), "0".to_string());
        params.insert("hostUin".to_string(), "0".to_string());
        params.insert("format".to_string(), "jsonp".to_string());
        params.insert("inCharset".to_string(), "utf8".to_string());
        params.insert("outCharset".to_string(), "utf8".to_string());
        params.insert("notice".to_string(), "0".to_string());
        params.insert("platform".to_string(), "yqq".to_string());
        params.insert("needNewCode".to_string(), "0".to_string());

        let resp = self.api.post_form_async(
            "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg",
            &params,
        ).await?;

        let json_str = resolve_resp_json(callback, &resp);
        if json_str.is_empty() {
            return Ok(None);
        }

        let mut result: LyricResult = serde_json::from_str(&json_str)?;
        result.decode();
        Ok(Some(result))
    }

    /// 通过 ID 获取解密后的歌词 (QRC)
    pub async fn get_lyrics_qrc(&self, id: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut params = HashMap::new();
        params.insert("version".to_string(), "15".to_string());
        params.insert("miniversion".to_string(), "82".to_string());
        params.insert("lrctype".to_string(), "4".to_string());
        params.insert("musicid".to_string(), id.to_string());

        let resp = self.api.post_form_async(
            "https://c.y.qq.com/qqmusic/fcgi-bin/lyric_download.fcg",
            &params,
        ).await?;
        
        
        // XML parsing and QRC decryption would go here
        // This is a simplified version - full implementation needs XML parsing + QRC decrypter
        Ok(CDATA.captures(&resp).ok_or("QQMusicApi: No match qrc")?.get(1).ok_or("QQMusicApi: Nothing here")?.as_str().to_string())
    }
}

impl Default for QQMusicApi {
    fn default() -> Self {
         Self::new()
    }
}

fn resolve_resp_json(callback_sign: &str, val: &str) -> String {
    if !val.starts_with(callback_sign) {
        return String::new();
    }
    let json_str = val.replacen(&format!("{}(", callback_sign), "", 1);
    if json_str.ends_with(')') {
        json_str[..json_str.len() - 1].to_string()
    } else {
        json_str
    }
}

// ===== Response Models =====

#[derive(Debug, Deserialize, Default)]
pub struct MusicFcgApiResult {
    pub code: Option<i64>,
    pub req_1: Option<MusicFcgReq1>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MusicFcgReq1 {
    pub code: Option<i64>,
    pub data: Option<MusicFcgReq1Data>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MusicFcgReq1Data {
    pub body: Option<MusicFcgReq1DataBody>,

}

#[derive(Debug, Deserialize, Default)]
pub struct MusicFcgReq1DataBody {
    pub song: Option<SongList>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SongList {
    pub list: Option<Vec<Song>>,
}


#[derive(Debug, Deserialize, Default)]
pub struct Song {
    pub album: Option<Album>,
    pub id: Option<u32>,
    pub interval: Option<i32>,
    pub mid: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub singer: Option<Vec<Singer>>,
    pub time_public: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Singer {
    pub id: Option<i64>,
    pub mid: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Album {
    pub id: Option<i32>,
    pub mid: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LyricResult {
    pub code: Option<i64>,
    #[serde(rename = "lyric")]
    pub lyric: Option<String>,
    pub trans: Option<String>,
}

impl LyricResult {
    pub fn decode(&mut self) {
        use base64::Engine;
        use base64::engine::general_purpose::STANDARD;
        if let Some(ref lyric) = self.lyric {
            if let Ok(decoded) = STANDARD.decode(lyric) {
                self.lyric = String::from_utf8(decoded).ok();
            }
        }
        if let Some(ref trans) = self.trans {
            if let Ok(decoded) = STANDARD.decode(trans) {
                self.trans = String::from_utf8(decoded).ok();
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct QqLyricsResponse {
    pub lyrics: String,
    pub trans: String,
}
