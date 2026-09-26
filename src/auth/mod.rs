pub mod pkce;
pub mod server;

use crate::config::Config;
use anyhow::{bail, Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const REDIRECT_URI: &str = "http://127.0.0.1:8888/callback";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
const CALLBACK_PORT: u16 = 8888;

pub const SCOPES: &[&str] = &[
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "user-read-recently-played",
    "user-top-read",
    "user-library-read",
    "user-library-modify",
    "user-follow-read",
    "user-follow-modify",
    "playlist-read-private",
    "playlist-read-collaborative",
    "playlist-modify-public",
    "playlist-modify-private",
];

#[derive(Debug, Deserialize, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
}

pub async fn login(config: &mut Config) -> Result<()> {
    let client_id = match &config.client_id {
        Some(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => {
            println!("{}", "No Spotify Client ID configured.".yellow());
            println!("You need a free Spotify Developer App Client ID to access the API.");
            println!("Create one in 1 minute at: https://developer.spotify.com/dashboard");
            println!("Make sure to add {} to your app's Redirect URIs in the dashboard.\n", REDIRECT_URI.cyan());

            let input: String = inquire::Text::new("Enter your Spotify Client ID: ")
                .with_placeholder("e.g. 3e96f0ef8d6d4e0994e15bf2b168235f")
                .prompt()
                .context("Prompt cancelled")?;

            let trimmed = input.trim().to_string();
            if trimmed.is_empty() {
                bail!("Client ID cannot be empty");
            }
            config.client_id = Some(trimmed.clone());
            config.save()?;
            trimmed
        }
    };

    let verifier = pkce::generate_code_verifier();
    let challenge = pkce::generate_code_challenge(&verifier);
    let state = pkce::generate_state();

    let scopes_str = SCOPES.join(" ");
    let mut auth_url = url::Url::parse(AUTHORIZE_URL)?;
    auth_url.query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &challenge)
        .append_pair("state", &state)
        .append_pair("scope", &scopes_str);

    let auth_url_str = auth_url.to_string();
    println!("\nOpening browser for Spotify authorization...");
    println!("If the browser does not open automatically, visit this URL:\n{}\n", auth_url_str.underline().blue());

    if let Err(e) = open::that(&auth_url_str) {
        eprintln!("Warning: Failed to launch browser automatically: {}", e);
    }

    // Await callback on local TCP listener
    let code = server::wait_for_callback(&state, CALLBACK_PORT).await?;
    println!("{}", "Authorization code received! Exchanging for tokens...".green());

    let client = reqwest::Client::new();
    let params = [
        ("client_id", client_id.as_str()),
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", REDIRECT_URI),
        ("code_verifier", verifier.as_str()),
    ];

    let res = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .context("Failed to exchange authorization code for tokens")?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        bail!("Failed to obtain access token (HTTP {}): {}", status, body);
    }

    let token_resp: TokenResponse = res.json().await.context("Failed to parse token response")?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    config.access_token = Some(token_resp.access_token);
    if let Some(ref_tok) = token_resp.refresh_token {
        config.refresh_token = Some(ref_tok);
    }
    config.expires_at = Some(now + token_resp.expires_in);
    config.save()?;

    println!("{}", "Authentication successful! Tokens stored securely.".green().bold());
    Ok(())
}

pub async fn refresh_access_token(config: &mut Config) -> Result<String> {
    let client_id = config
        .client_id
        .as_ref()
        .context("Spotify Client ID is not configured")?;
    let refresh_token = config
        .refresh_token
        .as_ref()
        .context("No refresh token available. Please run `spotify auth login`.")?;

    let client = reqwest::Client::new();
    let params = [
        ("client_id", client_id.as_str()),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token.as_str()),
    ];

    let res = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .context("Failed to send token refresh request")?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        bail!("Failed to refresh access token (HTTP {}): {}", status, body);
    }

    let token_resp: TokenResponse = res.json().await.context("Failed to parse refreshed token")?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    config.access_token = Some(token_resp.access_token.clone());
    if let Some(ref_tok) = token_resp.refresh_token {
        config.refresh_token = Some(ref_tok);
    }
    config.expires_at = Some(now + token_resp.expires_in);
    config.save()?;

    Ok(token_resp.access_token)
}
