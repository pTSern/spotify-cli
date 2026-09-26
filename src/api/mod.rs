pub mod errors;

use crate::auth::refresh_access_token;
use crate::config::Config;
use crate::models::*;
use errors::SpotifyError;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
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

            let mut req = self.http.request(method.clone(), &url).headers(headers);
            if let Some(ref b) = body {
                req = req.json(b);
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

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SpotifyError::RateLimited(5));
        }

        if status == reqwest::StatusCode::FORBIDDEN {
            if err_body.contains("PREMIUM_REQUIRED") {
                return Err(SpotifyError::PremiumRequired);
            }
            return Err(SpotifyError::ApiError {
                status: status.as_u16(),
                message: "Forbidden: check account permissions or Spotify Premium status".to_string(),
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
}

fn urlencoding(input: &str) -> String {
    url::form_urlencoded::byte_serialize(input.as_bytes()).collect()
}
