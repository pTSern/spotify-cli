use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpotifyError {
    #[error("Not authenticated. Please run `spotify auth login` first.")]
    NotAuthenticated,

    #[error("No active Spotify device found. Please start playback on your Spotify app or use `spotify devices --switch <name>`.")]
    NoActiveDevice,

    #[error("Spotify Premium is required for playback control via the API.")]
    PremiumRequired,

    #[error("Spotify API rate limit reached. Please wait {0} seconds.")]
    RateLimited(u64),

    #[error("Spotify API error (HTTP {status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Request error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Unexpected error: {0}")]
    Other(#[from] anyhow::Error),
}
