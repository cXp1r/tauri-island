use super::base_api::BaseApi;
use serde::Deserialize;
use std::collections::HashMap;

/// 某些嵌套对象的 id 字段在不同接口返回时为整数而非字符串，统一转为 String
mod string_or_int {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, D>(d: D) -> Result<Option<String>, D::Error>
    where D: Deserializer<'de> {
        let v: Option<serde_json::Value> = Option::deserialize(d)?;
        Ok(v.and_then(|v| match v {
            serde_json::Value::String(s) => Some(s),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        }))
    }
}

pub struct SodaMusicApi {
    api: BaseApi,
}

impl SodaMusicApi {
    pub fn new() -> Self {
        Self {
            api: BaseApi::new(Some("https://api.qishui.com/"), None),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            api: BaseApi::with_client(client, Some("https://api.qishui.com/"), None),
        }
    }

    /// 搜索歌曲
    pub async fn search(&self, keyword: &str) -> Result<Option<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://api.qishui.com/luna/pc/search/track?aid=386088&app_name=&region=&geo_region=&os_region=&sim_region=&device_id=&cdid=&iid=&version_name=&version_code=&channel=&build_mode=&network_carrier=&ac=&tz_name=&resolution=&device_platform=&device_type=&os_version=&fp=&q={}&cursor=&search_id=&search_method=input&debug_params=&from_search_id=&search_scene=",
            urlencoding::encode(keyword)
        );
        let resp = self.api.get_async(&url).await?;
        Ok(serde_json::from_str(&resp).ok())
    }

    /// 获取曲目详情 (含歌词)
    pub async fn get_detail(&self, id: &str) -> Result<Option<TrackDetailResult>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://api.qishui.com/luna/pc/track_v2?track_id={}&media_type=track&queue_type=",
            urlencoding::encode(id)
        );
        match self.api.get_async(&url).await {
            Ok(resp) => Ok(serde_json::from_str(&resp).ok()),
            Err(_) => Ok(None),
        }
    }
}

impl Default for SodaMusicApi {
    fn default() -> Self {
         Self::new()
    }
}
// ===== Response Models =====

#[derive(Debug, Deserialize, Default)]
pub struct SearchResult {
    #[serde(rename = "status_info")]
    pub status_info: Option<StatusInfo>,
    #[serde(rename = "result_groups")]
    pub result_groups: Option<Vec<ResultGroup>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct StatusInfo {
    #[serde(rename = "log_id")]
    pub log_id: Option<String>,
    pub now: Option<i64>,
    #[serde(rename = "now_ts_ms")]
    pub now_ts_ms: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ResultGroup {
    pub id: Option<String>,
    #[serde(rename = "has_more")]
    pub has_more: Option<bool>,
    pub data: Option<Vec<ResultGroupItem>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ResultGroupItem {
    pub meta: Option<Meta>,
    pub entity: Option<Entity>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Meta {
    #[serde(rename = "item_type")]
    pub item_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Entity {
    pub track: Option<TrackContainer>,
}

#[derive(Debug, Deserialize, Default)]
pub struct TrackContainer {
    #[serde(default, deserialize_with = "string_or_int::deserialize")]
    pub id: Option<String>,
    pub album: Option<SodaAlbum>,
    pub artists: Option<Vec<SodaArtist>>,
    pub duration: Option<i64>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SodaAlbum {
    #[serde(default, deserialize_with = "string_or_int::deserialize")]
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SodaArtist {
    #[serde(default, deserialize_with = "string_or_int::deserialize")]
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct TrackDetailResult {
    #[serde(rename = "status_info")]
    pub status_info: Option<StatusInfo>,
    pub lyric: Option<LyricInfo>,
    pub track: Option<TrackInfo>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LyricInfo {
    pub content: Option<String>,
    pub lang: Option<String>,
    #[serde(rename = "type")]
    pub lyric_type: Option<String>,
    #[serde(default, deserialize_with = "string_or_int::deserialize")]
    pub id: Option<String>,
    #[serde(rename = "lang_translations")]
    pub lang_translations: Option<HashMap<String, LyricTranslation>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LyricTranslation {
    pub content: Option<String>,
    pub lang: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct TrackInfo {
    #[serde(default, deserialize_with = "string_or_int::deserialize")]
    pub id: Option<String>,
    pub name: Option<String>,
    pub duration: Option<i64>,
    pub artists: Option<Vec<SodaArtist>>,
    pub album: Option<SodaAlbum>,
}
