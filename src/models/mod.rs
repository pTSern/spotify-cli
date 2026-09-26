use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: Option<String>,
    pub is_active: bool,
    pub is_private_session: Option<bool>,
    pub is_restricted: Option<bool>,
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub volume_percent: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicesResponse {
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Followers {
    pub total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: Option<String>,
    pub name: String,
    pub uri: Option<String>,
    pub external_urls: Option<HashMap<String, String>>,
    pub genres: Option<Vec<String>>,
    pub followers: Option<Followers>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: Option<String>,
    pub name: String,
    pub uri: Option<String>,
    pub total_tracks: Option<u32>,
    pub release_date: Option<String>,
    pub artists: Option<Vec<Artist>>,
    pub external_urls: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: Option<String>,
    pub name: String,
    pub uri: Option<String>,
    pub duration_ms: u64,
    pub track_number: Option<u32>,
    #[serde(default)]
    pub artists: Vec<Artist>,
    pub album: Option<Album>,
    pub external_urls: Option<HashMap<String, String>>,
}

impl Track {
    pub fn artists_str(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackState {
    pub device: Option<Device>,
    pub repeat_state: String,
    pub shuffle_state: bool,
    pub is_playing: bool,
    pub item: Option<Track>,
    pub progress_ms: Option<u64>,
    pub currently_playing_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueResponse {
    pub currently_playing: Option<Track>,
    #[serde(default)]
    pub queue: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub product: Option<String>,
    pub external_urls: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistOwner {
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTracksRef {
    pub total: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub uri: String,
    pub description: Option<String>,
    pub owner: Option<PlaylistOwner>,
    pub tracks: Option<PlaylistTracksRef>,
    #[serde(default)]
    pub public: Option<bool>,
    #[serde(default)]
    pub collaborative: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTrackItem {
    pub track: Option<Track>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTracksResponse {
    pub items: Vec<PlaylistTrackItem>,
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
    pub next: Option<String>,
    pub previous: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub tracks: Option<Paginated<Track>>,
    pub albums: Option<Paginated<Album>>,
    pub artists: Option<Paginated<Artist>>,
    pub playlists: Option<Paginated<Playlist>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentlyPlayedItem {
    pub track: Track,
    pub played_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentlyPlayedResponse {
    pub items: Vec<RecentlyPlayedItem>,
}
