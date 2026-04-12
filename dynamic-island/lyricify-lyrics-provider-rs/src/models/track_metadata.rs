use serde::{Deserialize, Serialize};

/// 曲目元数据 trait (用于搜索匹配)
pub trait ITrackMetadata: Send + Sync {
    fn title(&self) -> Option<&str>;
    fn artist(&self) -> Option<&str>;
    fn album(&self) -> Option<&str>;
    fn album_artist(&self) -> Option<&str> { None }
    fn duration_ms(&self) -> Option<i32> { None }
}

impl ITrackMetadata for TrackMetadata {
    fn title(&self) -> Option<&str> { self.title.as_deref() }
    fn artist(&self) -> Option<&str> { self.artist.as_deref() }
    fn album(&self) -> Option<&str> { self.album.as_deref() }
    fn album_artist(&self) -> Option<&str> { self.album_artist.as_deref() }
    fn duration_ms(&self) -> Option<i32> { self.duration_ms }
}

/// 曲目元数据类型在此...............
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_ms: Option<i32>,
    pub isrc: Option<String>,
    pub language: Option<Vec<String>>,
}

/// 多艺术家曲目元数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackMultiArtistMetadata {
    pub title: Option<String>,
    pub artists: Option<Vec<String>>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_ms: Option<i32>,
    pub isrc: Option<String>,
    pub language: Option<Vec<String>>,
}

impl TrackMultiArtistMetadata {
    pub fn artist_string(&self) -> Option<String> {
        self.artists.as_ref().map(|a| a.join(", "))
    }
}

/// Spotify 曲目元数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpotifyTrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_ms: Option<i32>,
    pub isrc: Option<String>,
    pub language: Option<Vec<String>>,
    pub spotify_id: Option<String>,
    pub spotify_uri: Option<String>,
}
