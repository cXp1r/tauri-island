use super::base_api::BaseApi;
use serde::Deserialize;

pub struct KugouApi {
    api: BaseApi,
}

impl KugouApi {
    pub fn new() -> Self {
        Self {
            api: BaseApi::new(None, None),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            api: BaseApi::with_client(client, None, None),
        }
    }

    /// 搜索歌曲
    pub async fn get_search_song(&self, keywords: &str) -> Result<Option<SearchSongResponse>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "http://mobilecdn.kugou.com/api/v3/search/song?format=json&keyword={}&page=1&pagesize=20&showtype=1",
            urlencoding::encode(keywords)
        );
        let resp = self.api.get_async(&url).await?;
        Ok(serde_json::from_str(&resp).ok())
    }

    /// 下载 KRC 歌词（返回加密 base64 内容）
    pub async fn get_download_krc(
        &self,
        id: &str,
        access_key: &str,
    ) -> Result<Option<DownloadKrcResponse>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://lyrics.kugou.com/download?ver=1&client=pc&id={}&accesskey={}&fmt=krc&charset=utf8",
            id, access_key
        );
        let resp = self.api.get_async(&url).await?;
        Ok(serde_json::from_str(&resp).ok())
    }

    /// 搜索歌词
    pub async fn get_search_lyrics(
        &self,
        keywords: Option<&str>,
        duration: Option<i32>,
        hash: Option<&str>,
    ) -> Result<Option<SearchLyricsResponse>, Box<dyn std::error::Error + Send + Sync>> {
        let duration_param = duration.map(|d| format!("&duration={}", d)).unwrap_or_default();
        let hash_val = hash.unwrap_or("");
        let keyword_val = keywords.unwrap_or("");
        let url = format!(
            "https://lyrics.kugou.com/search?ver=1&man=yes&client=pc&keyword={}{}&hash={}",
            urlencoding::encode(keyword_val),
            duration_param,
            hash_val
        );
        let resp = self.api.get_async(&url).await?;
        Ok(serde_json::from_str(&resp).ok())
    }
}

// ===== Response Models =====

#[derive(Debug, Deserialize, Default)]
pub struct SearchSongResponse {
    pub status: Option<i32>,
    pub error: Option<String>,
    pub data: Option<SearchSongData>,
    #[serde(rename = "errcode")]
    pub error_code: Option<i32>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchSongData {
    pub timestamp: Option<i64>,
    pub total: Option<i32>,
    pub info: Option<Vec<SearchSongInfo>>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct SearchSongInfo {
    #[serde(rename = "hash")]
    pub hash: Option<String>,
    #[serde(rename = "songname")]
    pub song_name: Option<String>,
    #[serde(rename = "album_name")]
    pub album_name: Option<String>,
    #[serde(rename = "songname_original")]
    pub song_name_original: Option<String>,
    #[serde(rename = "singername")]
    pub singer_name: Option<String>,
    pub duration: Option<i32>,
    #[serde(rename = "filename")]
    pub filename: Option<String>,
    pub group: Option<Vec<SearchSongInfo>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchLyricsResponse {
    pub status: Option<i32>,
    pub info: Option<String>,
    #[serde(rename = "errcode")]
    pub error_code: Option<i32>,
    #[serde(rename = "errmsg")]
    pub error_message: Option<String>,
    pub proposal: Option<String>,
    pub candidates: Option<Vec<LyricsCandidate>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LyricsCandidate {
    pub id: Option<String>,
    #[serde(rename = "product_from")]
    pub product_from: Option<String>,
    #[serde(rename = "accesskey")]
    pub access_key: Option<String>,
    pub singer: Option<String>,
    pub song: Option<String>,
    pub duration: Option<i32>,
    pub uid: Option<String>,
    pub nickname: Option<String>,
    pub language: Option<String>,
    #[serde(rename = "krctype")]
    pub krc_type: Option<i32>,
    pub score: Option<i32>,
    #[serde(rename = "contenttype")]
    pub content_type: Option<i32>,
    #[serde(rename = "content_format")]
    pub content_format: Option<i32>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DownloadKrcResponse {
    pub content: Option<String>,
    pub info: Option<String>,
    pub status: Option<i32>,
    #[serde(rename = "contenttype")]
    pub content_type: Option<i32>,
    #[serde(rename = "error_code")]
    pub error_code: Option<i32>,
    pub fmt: Option<String>,
}
