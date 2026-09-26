use crate::api::errors::SpotifyError;
use crate::api::SpotifyClient;
use crate::ui::{format_duration, print_playback_status};
use anyhow::Result;
use colored::Colorize;

pub async fn run_status(client: &mut SpotifyClient) -> Result<()> {
    match client.get_playback_state().await {
        Ok(Some(state)) => {
            print_playback_status(&state);
        }
        Ok(None) => {
            println!("{}", "No playback active. Start music on Spotify or run `spotify play`.".yellow());
        }
        Err(e) => {
            eprintln!("{}: {}", "Error retrieving playback status".red(), e);
        }
    }
    Ok(())
}

pub async fn run_play(
    client: &mut SpotifyClient,
    query: Option<String>,
    play_type: &str, // "track", "album", "playlist"
) -> Result<()> {
    // If a query is provided, search or parse URI
    if let Some(q) = query {
        let q_trimmed = q.trim();
        if q_trimmed.starts_with("spotify:") {
            if q_trimmed.starts_with("spotify:track:") {
                client.play(None, Some(vec![q_trimmed.to_string()]), None, None).await?;
            } else {
                client.play(Some(q_trimmed.to_string()), None, None, None).await?;
            }
            println!("Playing URI: {}", q_trimmed.cyan());
        } else {
            // Search Spotify
            println!("Searching for {} '{}'...", play_type, q_trimmed.cyan());
            let results = client.search(q_trimmed, play_type, 1, 0).await?;

            match play_type {
                "album" => {
                    if let Some(albums) = results.albums {
                        if let Some(first) = albums.items.first() {
                            println!("▶ Playing Album: {} by {}", first.name.bold(), first.artists.as_ref().map(|a| a.iter().map(|x| x.name.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default().cyan());
                            client.play(first.uri.clone(), None, None, None).await?;
                        } else {
                            println!("{}", "No albums found matching query.".yellow());
                        }
                    }
                }
                "playlist" => {
                    if let Some(playlists) = results.playlists {
                        if let Some(first) = playlists.items.first() {
                            println!("▶ Playing Playlist: {}", first.name.bold().cyan());
                            client.play(Some(first.uri.clone()), None, None, None).await?;
                        } else {
                            println!("{}", "No playlists found matching query.".yellow());
                        }
                    }
                }
                _ => {
                    // Default: track
                    if let Some(tracks) = results.tracks {
                        if let Some(first) = tracks.items.first() {
                            println!("▶ Playing: {} - {}", first.name.bold().white(), first.artists_str().cyan());
                            if let Some(ref album) = first.album {
                                client.play(
                                    album.uri.clone(),
                                    None,
                                    first.uri.clone(),
                                    None,
                                ).await?;
                            } else {
                                client.play(None, Some(vec![first.uri.clone().unwrap()]), None, None).await?;
                            }
                        } else {
                            println!("{}", "No tracks found matching query.".yellow());
                        }
                    }
                }
            }
        }
    } else {
        // Resume playback
        match client.play(None, None, None, None).await {
            Ok(_) => println!("{}", "▶ Resumed playback.".green()),
            Err(SpotifyError::NoActiveDevice) => {
                try_recover_device(client).await?;
            }
            Err(e) => eprintln!("{}: {}", "Failed to play".red(), e),
        }
    }

    Ok(())
}

pub async fn run_pause(client: &mut SpotifyClient) -> Result<()> {
    match client.pause(None).await {
        Ok(_) => println!("{}", "⏸ Paused playback.".yellow()),
        Err(SpotifyError::NoActiveDevice) => {
            println!("{}", "No active Spotify device to pause.".yellow());
        }
        Err(e) => eprintln!("{}: {}", "Failed to pause".red(), e),
    }
    Ok(())
}

pub async fn run_toggle(client: &mut SpotifyClient) -> Result<()> {
    match client.get_playback_state().await {
        Ok(Some(state)) => {
            if state.is_playing {
                run_pause(client).await
            } else {
                run_play(client, None, "track").await
            }
        }
        Ok(None) => {
            // No active state, attempt to resume or connect device
            run_play(client, None, "track").await
        }
        Err(e) => {
            eprintln!("{}: {}", "Error getting state for toggle".red(), e);
            Ok(())
        }
    }
}

pub async fn run_next(client: &mut SpotifyClient) -> Result<()> {
    client.next(None).await?;
    println!("{}", "⏭ Skipped to next track.".green());
    tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
    let _ = run_status(client).await;
    Ok(())
}

pub async fn run_previous(client: &mut SpotifyClient) -> Result<()> {
    client.previous(None).await?;
    println!("{}", "⏮ Skipped to previous track.".green());
    tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
    let _ = run_status(client).await;
    Ok(())
}

pub async fn run_seek(client: &mut SpotifyClient, time_str: &str) -> Result<()> {
    // Parse time string: e.g. "1:30" or "90" or "+10" or "-10"
    let target_ms = if time_str.starts_with('+') || time_str.starts_with('-') {
        let delta_secs: i64 = time_str.parse().unwrap_or(0);
        let current_state = client.get_playback_state().await?;
        let current_ms = current_state.and_then(|s| s.progress_ms).unwrap_or(0) as i64;
        (current_ms + (delta_secs * 1000)).max(0) as u64
    } else if time_str.contains(':') {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() == 2 {
            let m: u64 = parts[0].parse().unwrap_or(0);
            let s: u64 = parts[1].parse().unwrap_or(0);
            (m * 60 + s) * 1000
        } else {
            0
        }
    } else {
        let s: u64 = time_str.parse().unwrap_or(0);
        s * 1000
    };

    client.seek(target_ms, None).await?;
    println!("Seeked to {}", format_duration(target_ms).cyan());
    Ok(())
}

pub async fn run_shuffle(client: &mut SpotifyClient, mode: Option<String>) -> Result<()> {
    let new_state = match mode.as_deref() {
        Some("on") | Some("true") | Some("1") => true,
        Some("off") | Some("false") | Some("0") => false,
        _ => {
            let state = client.get_playback_state().await?;
            let current = state.map(|s| s.shuffle_state).unwrap_or(false);
            !current
        }
    };

    client.set_shuffle(new_state, None).await?;
    if new_state {
        println!("{}", "🔀 Shuffle turned ON.".green());
    } else {
        println!("{}", "🔀 Shuffle turned OFF.".yellow());
    }
    Ok(())
}

pub async fn run_repeat(client: &mut SpotifyClient, mode: Option<String>) -> Result<()> {
    let mode_str = match mode.as_deref() {
        Some("track") | Some("single") | Some("1") => "track",
        Some("context") | Some("all") | Some("on") => "context",
        Some("off") => "off",
        _ => {
            // Cycle: off -> context -> track -> off
            let current = client
                .get_playback_state()
                .await?
                .map(|s| s.repeat_state)
                .unwrap_or_else(|| "off".to_string());
            match current.as_str() {
                "off" => "context",
                "context" => "track",
                _ => "off",
            }
        }
    };

    client.set_repeat(mode_str, None).await?;
    match mode_str {
        "track" => println!("{}", "🔂 Repeat set to TRACK.".green()),
        "context" => println!("{}", "🔁 Repeat set to ALL.".green()),
        _ => println!("{}", "🔁 Repeat turned OFF.".yellow()),
    }
    Ok(())
}

async fn try_recover_device(client: &mut SpotifyClient) -> Result<()> {
    let devices = client.get_devices().await?;
    if devices.is_empty() {
        println!("{}", "No active Spotify devices found. Please open Spotify on your device.".yellow());
        return Ok(());
    }

    println!("{}", "No device is currently active. Available devices:".yellow());
    for dev in &devices {
        println!("  - {} ({})", dev.name.bold(), dev.device_type);
    }

    if let Some(first) = devices.first() {
        if let Some(ref id) = first.id {
            println!("Attempting to activate {}...", first.name.cyan());
            client.transfer_playback(id, true).await?;
            println!("{}", "Playback transferred successfully!".green());
        }
    }
    Ok(())
}
