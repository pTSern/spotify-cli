pub mod errors;

use crate::auth::refresh_access_token;
use crate::config::Config;
use crate::models::*;
use errors::SpotifyError;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{Method, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;

const BASE_URL: &str = "https://api.spotify.com/v1";

pub struct SpotifyClient {
    http: reqwest::Client,
    pub config: Config,
}

impl SpotifyClient {
    pub fn new(config: Config) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
        }
    }

    async fn ensure_valid_token(&mut self) -> Result<String, SpotifyError> {
        if self.config.access_token.is_none() {
            return Err(SpotifyError::NotAuthenticated);
        }

        if self.config.is_token_expired() {
            let new_token = refresh_access_token(&mut self.config)
                .await
                .map_err(|e| SpotifyError::Other(e))?;
            return Ok(new_token);
        }

        Ok(self.config.access_token.clone().unwrap())
    }

    async fn execute_request(
        &mut self,
        method: Method,
        endpoint: &str,
        body: Option<Value>,
    ) -> Result<Response, SpotifyError> {
        let token = self.ensure_valid_token().await?;
        let url = if endpoint.starts_with("http") {
            endpoint.to_string()
        } else {
            format!("{}/{}", BASE_URL, endpoint.trim_start_matches('/'))
        };

        let send_req = |token: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
            );
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

            let mut req = self.http.request(method.clone(), &url);
            if let Some(ref b) = body {
                req = req.headers(headers).json(b);
            } else {
                if method == Method::POST || method == Method::PUT || method == Method::DELETE {
                    headers.insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
                    req = req.headers(headers).body("");
                } else {
                    req = req.headers(headers);
                }
            }
            req
        };

        let mut res = send_req(&token).send().await?;

        // Handle token expiration retry
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            let new_token = refresh_access_token(&mut self.config)
                .await
                .map_err(|e| SpotifyError::Other(e))?;
            res = send_req(&new_token).send().await?;
        }

        let status = res.status();
        if status.is_success() {
            return Ok(res);
        }

        let err_body = res.text().await.unwrap_or_default();

        let req_body_str = body.as_ref().map(|b| b.to_string());
        crate::logger::log_api_call(
            method.as_str(),
            &url,
            req_body_str.as_deref(),
            Some(status.as_u16()),
            &err_body,
        );

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SpotifyError::RateLimited(5));
        }

        if status == reqwest::StatusCode::FORBIDDEN {
            if err_body.contains("PREMIUM_REQUIRED") {
                return Err(SpotifyError::PremiumRequired);
            }
            return Err(SpotifyError::ApiError {
                status: status.as_u16(),
                message: "Forbidden: check account permissions or Spotify Premium status. (If using playlist features, run `spotify auth login` once to grant updated permissions)".to_string(),
            });
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            if err_body.contains("NO_ACTIVE_DEVICE") || err_body.contains("Device not found") {
                return Err(SpotifyError::NoActiveDevice);
            }
        }

        // Try extracting error message from Spotify JSON format
        let message = if let Ok(val) = serde_json::from_str::<Value>(&err_body) {
            val["error"]["message"]
                .as_str()
                .unwrap_or(&err_body)
                .to_string()
        } else {
            err_body
        };

        Err(SpotifyError::ApiError {
            status: status.as_u16(),
            message,
        })
    }

    async fn get_json<T: DeserializeOwned>(&mut self, endpoint: &str) -> Result<T, SpotifyError> {
        let res = self.execute_request(Method::GET, endpoint, None).await?;
        let data = res.json::<T>().await?;
        Ok(data)
    }

    async fn send_empty(
        &mut self,
        method: Method,
        endpoint: &str,
        body: Option<Value>,
    ) -> Result<(), SpotifyError> {
        let _ = self.execute_request(method, endpoint, body).await?;
        Ok(())
    }

    // --- High Level API Methods ---

    pub async fn get_me(&mut self) -> Result<UserProfile, SpotifyError> {
        self.get_json("me").await
    }

    pub async fn get_playback_state(&mut self) -> Result<Option<PlaybackState>, SpotifyError> {
        let res = self.execute_request(Method::GET, "me/player", None).await;
        match res {
            Ok(r) => {
                if r.status() == reqwest::StatusCode::NO_CONTENT {
                    return Ok(None);
                }
                let text = r.text().await?;
                if text.trim().is_empty() {
                    return Ok(None);
                }
                let state: PlaybackState = serde_json::from_str(&text)
                    .map_err(|e| SpotifyError::Other(e.into()))?;
                Ok(Some(state))
            }
            Err(SpotifyError::NoActiveDevice) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn play(
        &mut self,
        context_uri: Option<String>,
        uris: Option<Vec<String>>,
        offset_uri: Option<String>,
        device_id: Option<&str>,
    ) -> Result<(), SpotifyError> {
        let mut endpoint = "me/player/play".to_string();
        if let Some(id) = device_id {
            endpoint.push_str(&format!("?device_id={}", id));
        }

        let mut body = serde_json::Map::new();
        if let Some(ctx) = context_uri {
            body.insert("context_uri".to_string(), Value::String(ctx));
        }
        if let Some(u) = uris {
            let uris_val: Vec<Value> = u.into_iter().map(Value::String).collect();
            body.insert("uris".to_string(), Value::Array(uris_val));
        }
        if let Some(off) = offset_uri {
            let mut off_map = serde_json::Map::new();
            off_map.insert("uri".to_string(), Value::String(off));
            body.insert("offset".to_string(), Value::Object(off_map));
        }

        let body_val = if body.is_empty() {
            None
        } else {
            Some(Value::Object(body))
        };

        self.send_empty(Method::PUT, &endpoint, body_val).await
    }

    pub async fn pause(&mut self, device_id: Option<&str>) -> Result<(), SpotifyError> {
        let mut endpoint = "me/player/pause".to_string();
        if let Some(id) = device_id {
            endpoint.push_str(&format!("?device_id={}", id));
        }
        self.send_empty(Method::PUT, &endpoint, None).await
    }

    pub async fn next(&mut self, device_id: Option<&str>) -> Result<(), SpotifyError> {
        let mut endpoint = "me/player/next".to_string();
        if let Some(id) = device_id {
            endpoint.push_str(&format!("?device_id={}", id));
        }
        self.send_empty(Method::POST, &endpoint, None).await
    }

    pub async fn previous(&mut self, device_id: Option<&str>) -> Result<(), SpotifyError> {
        let mut endpoint = "me/player/previous".to_string();
        if let Some(id) = device_id {
            endpoint.push_str(&format!("?device_id={}", id));
        }
        self.send_empty(Method::POST, &endpoint, None).await
    }

    pub async fn seek(&mut self, position_ms: u64, device_id: Option<&str>) -> Result<(), SpotifyError> {
        let mut endpoint = format!("me/player/seek?position_ms={}", position_ms);
        if let Some(id) = device_id {
            endpoint.push_str(&format!("&device_id={}", id));
        }
        self.send_empty(Method::PUT, &endpoint, None).await
    }

    pub async fn set_volume(&mut self, volume_percent: u32, device_id: Option<&str>) -> Result<(), SpotifyError> {
        let percent = volume_percent.min(100);
        let mut endpoint = format!("me/player/volume?volume_percent={}", percent);
        if let Some(id) = device_id {
            endpoint.push_str(&format!("&device_id={}", id));
        }
        self.send_empty(Method::PUT, &endpoint, None).await
    }

    pub async fn set_shuffle(&mut self, state: bool, device_id: Option<&str>) -> Result<(), SpotifyError> {
        let mut endpoint = format!("me/player/shuffle?state={}", state);
        if let Some(id) = device_id {
            endpoint.push_str(&format!("&device_id={}", id));
        }
        self.send_empty(Method::PUT, &endpoint, None).await
    }

    pub async fn set_repeat(&mut self, state: &str, device_id: Option<&str>) -> Result<(), SpotifyError> {
        let mut endpoint = format!("me/player/repeat?state={}", state);
        if let Some(id) = device_id {
            endpoint.push_str(&format!("&device_id={}", id));
        }
        self.send_empty(Method::PUT, &endpoint, None).await
    }

    pub async fn get_devices(&mut self) -> Result<Vec<Device>, SpotifyError> {
        let resp: DevicesResponse = self.get_json("me/player/devices").await?;
        Ok(resp.devices)
    }

    pub async fn transfer_playback(&mut self, device_id: &str, play: bool) -> Result<(), SpotifyError> {
        let endpoint = "me/player";
        let body = serde_json::json!({
            "device_ids": [device_id],
            "play": play
        });
        self.send_empty(Method::PUT, endpoint, Some(body)).await
    }

    pub async fn get_queue(&mut self) -> Result<QueueResponse, SpotifyError> {
        self.get_json("me/player/queue").await
    }

    pub async fn add_to_queue(&mut self, uri: &str, device_id: Option<&str>) -> Result<(), SpotifyError> {
        let mut endpoint = format!("me/player/queue?uri={}", urlencoding(uri));
        if let Some(id) = device_id {
            endpoint.push_str(&format!("&device_id={}", id));
        }
        self.send_empty(Method::POST, &endpoint, None).await
    }

    pub async fn search(
        &mut self,
        query: &str,
        search_types: &str,
        limit: u32,
        offset: u32,
    ) -> Result<SearchResponse, SpotifyError> {
        let endpoint = format!(
            "search?q={}&type={}&limit={}&offset={}",
            urlencoding(query),
            search_types,
            limit.min(50),
            offset
        );
        self.get_json(&endpoint).await
    }

    pub async fn get_recently_played(&mut self, limit: u32) -> Result<RecentlyPlayedResponse, SpotifyError> {
        let endpoint = format!("me/player/recently-played?limit={}", limit.min(50));
        self.get_json(&endpoint).await
    }

    pub async fn get_user_top_tracks(&mut self, limit: u32) -> Result<Paginated<Track>, SpotifyError> {
        let endpoint = format!("me/top/tracks?limit={}", limit.min(50));
        self.get_json(&endpoint).await
    }

    pub async fn get_user_top_artists(&mut self, limit: u32) -> Result<Paginated<Artist>, SpotifyError> {
        let endpoint = format!("me/top/artists?limit={}", limit.min(50));
        self.get_json(&endpoint).await
    }

    pub async fn save_tracks(&mut self, ids: &[&str]) -> Result<(), SpotifyError> {
        let endpoint = format!("me/tracks?ids={}", ids.join(","));
        self.send_empty(Method::PUT, &endpoint, None).await
    }

    pub async fn save_albums(&mut self, ids: &[&str]) -> Result<(), SpotifyError> {
        let endpoint = format!("me/albums?ids={}", ids.join(","));
        self.send_empty(Method::PUT, &endpoint, None).await
    }

    #[allow(dead_code)]
    pub async fn follow_artists(&mut self, ids: &[&str]) -> Result<(), SpotifyError> {
        let endpoint = format!("me/following?type=artist&ids={}", ids.join(","));
        self.send_empty(Method::PUT, &endpoint, None).await
    }

    pub async fn get_current_user(&mut self) -> Result<UserProfile, SpotifyError> {
        self.get_json("me").await
    }

    pub async fn get_user_playlists(&mut self, limit: u32, offset: u32) -> Result<Paginated<Playlist>, SpotifyError> {
        let endpoint = format!("me/playlists?limit={}&offset={}", limit.min(50), offset);
        self.get_json(&endpoint).await
    }

    #[allow(dead_code)]
    pub async fn get_playlist(&mut self, playlist_id: &str) -> Result<Playlist, SpotifyError> {
        let endpoint = format!("playlists/{}", playlist_id);
        self.get_json(&endpoint).await
    }

    pub async fn get_playlist_tracks(&mut self, playlist_id: &str, limit: u32, offset: u32) -> Result<PlaylistTracksResponse, SpotifyError> {
        let endpoint = format!("playlists/{}/tracks?limit={}&offset={}", playlist_id, limit.min(50), offset);
        self.get_json(&endpoint).await
    }

    pub async fn create_playlist(
        &mut self,
        user_id: &str,
        name: &str,
        description: Option<&str>,
        is_public: bool,
        is_collaborative: bool,
    ) -> Result<Playlist, SpotifyError> {
        let endpoint = format!("users/{}/playlists", user_id);
        let mut body = serde_json::json!({
            "name": name,
            "public": is_public,
            "collaborative": is_collaborative
        });
        if let Some(desc) = description {
            body["description"] = serde_json::Value::String(desc.to_string());
        }
        let res = self.execute_request(Method::POST, &endpoint, Some(body)).await?;
        let playlist = res.json::<Playlist>().await?;
        Ok(playlist)
    }

    pub async fn add_tracks_to_playlist(
        &mut self,
        playlist_id: &str,
        track_uris: &[&str],
    ) -> Result<(), SpotifyError> {
        let endpoint = format!("playlists/{}/tracks", playlist_id);
        let body = serde_json::json!({
            "uris": track_uris
        });
        self.send_empty(Method::POST, &endpoint, Some(body)).await
    }

    pub async fn change_playlist_details(
        &mut self,
        playlist_id: &str,
        name: Option<&str>,
        description: Option<&str>,
        is_public: Option<bool>,
        is_collaborative: Option<bool>,
    ) -> Result<(), SpotifyError> {
        let endpoint = format!("playlists/{}", playlist_id);
        let mut map = serde_json::Map::new();
        if let Some(n) = name {
            map.insert("name".to_string(), serde_json::Value::String(n.to_string()));
        }
        if let Some(d) = description {
            map.insert("description".to_string(), serde_json::Value::String(d.to_string()));
        }
        if let Some(p) = is_public {
            map.insert("public".to_string(), serde_json::Value::Bool(p));
        }
        if let Some(c) = is_collaborative {
            map.insert("collaborative".to_string(), serde_json::Value::Bool(c));
        }
        self.send_empty(Method::PUT, &endpoint, Some(serde_json::Value::Object(map))).await
    }
}

fn urlencoding(input: &str) -> String {
    url::form_urlencoded::byte_serialize(input.as_bytes()).collect()
}
