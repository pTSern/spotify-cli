use crate::api::errors::SpotifyError;
use crate::api::SpotifyClient;
use crate::models::PlaybackState;
use crate::ui::{format_duration, print_playback_status};
use anyhow::Result;
use colored::Colorize;
use crossterm::cursor::{Hide, MoveToColumn, MoveUp, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use std::io::{stdout, Write};
use std::time::{Duration, Instant};

struct LivePlayerState {
    track_name: String,
    artists: String,
    album: String,
    duration_ms: u64,
    current_ms: u64,
    last_tick: Instant,
    is_playing: bool,
    device_name: String,
    volume_percent: u32,
    shuffle_state: bool,
    repeat_state: String,
    last_api_sync: Instant,
}

impl LivePlayerState {
    fn from_playback(state: &PlaybackState) -> Self {
        let (name, artists, album, duration) = match state.item {
            Some(ref t) => (
                t.name.clone(),
                t.artists_str(),
                t.album.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
                t.duration_ms,
            ),
            None => ("No Track".to_string(), "Unknown".to_string(), "".to_string(), 0),
        };

        let (device_name, volume) = match state.device {
            Some(ref d) => (d.name.clone(), d.volume_percent.unwrap_or(50)),
            None => ("Active Device".to_string(), 50),
        };

        Self {
            track_name: name,
            artists,
            album,
            duration_ms: duration,
            current_ms: state.progress_ms.unwrap_or(0),
            last_tick: Instant::now(),
            is_playing: state.is_playing,
            device_name,
            volume_percent: volume,
            shuffle_state: state.shuffle_state,
            repeat_state: state.repeat_state.clone(),
            last_api_sync: Instant::now(),
        }
    }

    fn update_from_playback(&mut self, state: &PlaybackState) {
        if let Some(ref t) = state.item {
            self.track_name = t.name.clone();
            self.artists = t.artists_str();
            self.album = t.album.as_ref().map(|a| a.name.clone()).unwrap_or_default();
            self.duration_ms = t.duration_ms;
        }
        if let Some(ref d) = state.device {
            self.device_name = d.name.clone();
            self.volume_percent = d.volume_percent.unwrap_or(self.volume_percent);
        }
        self.is_playing = state.is_playing;
        self.shuffle_state = state.shuffle_state;
        self.repeat_state = state.repeat_state.clone();
        self.current_ms = state.progress_ms.unwrap_or(self.current_ms);
        self.last_tick = Instant::now();
        self.last_api_sync = Instant::now();
    }
}

pub async fn run_status(client: &mut SpotifyClient, static_mode: bool) -> Result<()> {
    if static_mode {
        match client.get_playback_state().await {
            Ok(Some(state)) => {
                print_playback_status(&state);
            }
            Ok(None) => {
                println!("{}", "No playback active. Start music on Spotify or run `spotify-cli play`.".yellow());
            }
            Err(e) => {
                eprintln!("{}: {}", "Error retrieving playback status".red(), e);
            }
        }
        return Ok(());
    }

    run_interactive_status(client).await
}

async fn run_interactive_status(client: &mut SpotifyClient) -> Result<()> {
    let initial_state = match client.get_playback_state().await? {
        Some(s) => s,
        None => {
            println!("{}", "No active playback session found. Start playback on Spotify first.".yellow());
            return Ok(());
        }
    };

    let mut state = LivePlayerState::from_playback(&initial_state);
    let mut stdout = stdout();

    println!("\n{}", "── Spotify Interactive Real-Time Player ──".green().bold());

    let mut first_render = true;
    let mut last_display_sec: u64 = u64::MAX;

    let _ = crossterm::execute!(stdout, Hide);

    loop {
        let seek_step = client.config.status_settings.seek_step;
        let vol_step = client.config.volume_settings.step;

        // Calculate progress update
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_tick).as_millis() as u64;
        if state.is_playing {
            state.current_ms = (state.current_ms + elapsed).min(state.duration_ms);
        }
        state.last_tick = now;

        // Song completed check: fetch next song from Spotify API
        if state.is_playing && state.current_ms >= state.duration_ms && state.duration_ms > 0 {
            tokio::time::sleep(Duration::from_millis(600)).await;
            if let Ok(Some(fresh)) = client.get_playback_state().await {
                state.update_from_playback(&fresh);
            }
        }

        // Periodic API sync (every 20s) to prevent clock drift
        if now.duration_since(state.last_api_sync).as_secs() >= 20 {
            if let Ok(Some(fresh)) = client.get_playback_state().await {
                state.update_from_playback(&fresh);
            }
        }

        let current_display_sec = state.current_ms / 1000;
        if first_render || current_display_sec != last_display_sec {
            render_player(&state, seek_step, vol_step, first_render);
            first_render = false;
            last_display_sec = current_display_sec;
        }

        // Poll for keyboard input (non-blocking tick 200ms)
        enable_raw_mode()?;
        let has_event = event::poll(Duration::from_millis(200))?;
        let key_code = if has_event {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind == KeyEventKind::Press {
                    Some(key_event.code)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        disable_raw_mode()?;

        if let Some(code) = key_code {
            match code {
                // Seek forward
                KeyCode::Right => {
                    let target_ms = (state.current_ms + (seek_step as u64 * 1000)).min(state.duration_ms);
                    state.current_ms = target_ms;
                    state.last_tick = Instant::now();
                    let _ = client.seek(target_ms, None).await;
                    render_player(&state, seek_step, vol_step, false);
                }
                // Seek backward
                KeyCode::Left => {
                    let target_ms = state.current_ms.saturating_sub(seek_step as u64 * 1000);
                    state.current_ms = target_ms;
                    state.last_tick = Instant::now();
                    let _ = client.seek(target_ms, None).await;
                    render_player(&state, seek_step, vol_step, false);
                }
                // Volume Up
                KeyCode::Up => {
                    state.volume_percent = (state.volume_percent + vol_step).min(100);
                    let _ = client.set_volume(state.volume_percent, None).await;
                    render_player(&state, seek_step, vol_step, false);
                }
                // Volume Down
                KeyCode::Down => {
                    state.volume_percent = state.volume_percent.saturating_sub(vol_step);
                    let _ = client.set_volume(state.volume_percent, None).await;
                    render_player(&state, seek_step, vol_step, false);
                }
                // Toggle Play/Pause
                KeyCode::Char(' ') | KeyCode::Char('t') => {
                    if state.is_playing {
                        let _ = client.pause(None).await;
                        state.is_playing = false;
                    } else {
                        let _ = client.play(None, None, None, None).await;
                        state.is_playing = true;
                        state.last_tick = Instant::now();
                    }
                    render_player(&state, seek_step, vol_step, false);
                }
                // Next Track
                KeyCode::Char('n') => {
                    let _ = client.next(None).await;
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    if let Ok(Some(fresh)) = client.get_playback_state().await {
                        state.update_from_playback(&fresh);
                    }
                    render_player(&state, seek_step, vol_step, false);
                }
                // Previous Track
                KeyCode::Char('p') => {
                    let _ = client.previous(None).await;
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    if let Ok(Some(fresh)) = client.get_playback_state().await {
                        state.update_from_playback(&fresh);
                    }
                    render_player(&state, seek_step, vol_step, false);
                }
                // Toggle Shuffle
                KeyCode::Char('f') => {
                    state.shuffle_state = !state.shuffle_state;
                    let _ = client.set_shuffle(state.shuffle_state, None).await;
                    render_player(&state, seek_step, vol_step, false);
                }
                // Cycle Repeat
                KeyCode::Char('r') => {
                    state.repeat_state = match state.repeat_state.as_str() {
                        "off" => "context",
                        "context" => "track",
                        _ => "off",
                    }
                    .to_string();
                    let _ = client.set_repeat(&state.repeat_state, None).await;
                    render_player(&state, seek_step, vol_step, false);
                }
                // Settings
                KeyCode::Char('s') => {
                    let _ = crossterm::execute!(stdout, Show);
                    println!();
                    manage_player_settings(client).await?;
                    let _ = crossterm::execute!(stdout, Hide);
                    first_render = true;
                }
                // Exit
                KeyCode::Esc | KeyCode::Char('q') => {
                    let _ = crossterm::execute!(stdout, Show);
                    println!("\n\n{}", "Exited interactive player.".bright_black());
                    break;
                }
                _ => {}
            }
        }
    }

    let _ = crossterm::execute!(stdout, Show);
    Ok(())
}

fn render_player(state: &LivePlayerState, seek_step: u32, vol_step: u32, first_render: bool) {
    let play_badge = if state.is_playing {
        "▶ Playing".green().bold()
    } else {
        "⏸ Paused".yellow().bold()
    };

    let shuffle_badge = if state.shuffle_state {
        "🔀 on".green()
    } else {
        "🔀 off".bright_black()
    };

    let repeat_badge = match state.repeat_state.as_str() {
        "track" => "🔂 track".green(),
        "context" => "🔁 all".green(),
        _ => "🔁 off".bright_black(),
    };

    let bar_width: usize = 30;
    let ratio = if state.duration_ms > 0 {
        (state.current_ms as f64 / state.duration_ms as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled_len = ((bar_width as f64) * ratio).round() as usize;
    let unfilled_len = bar_width.saturating_sub(filled_len);
    let filled = "━".repeat(filled_len).green().bold();
    let unfilled = "─".repeat(unfilled_len).bright_black();

    let mut out = stdout();
    if !first_render {
        let _ = crossterm::execute!(out, MoveUp(8), MoveToColumn(0), Clear(ClearType::FromCursorDown));
    }

    println!("  {} {}", play_badge, "(Interactive Player)".bright_black());
    println!("  {} {}", "Track:  ".bright_black(), state.track_name.bold().white());
    println!("  {} {}", "Artist: ".bright_black(), state.artists.cyan());
    println!("  {} {}", "Album:  ".bright_black(), state.album.italic());
    println!(
        "  {} [{}{}] {} / {}",
        "Time:   ".bright_black(),
        filled,
        unfilled,
        format_duration(state.current_ms).cyan(),
        format_duration(state.duration_ms).bright_black()
    );
    println!(
        "  {} {} | Vol: {} | {} | {}",
        "Device: ".bright_black(),
        state.device_name.bold(),
        format!("{}%", state.volume_percent).magenta(),
        shuffle_badge,
        repeat_badge
    );
    println!();
    println!(
        "  {} seek {}s   {} vol {}%   {} play/pause   {} next/prev   {} shuffle   {} repeat   {} settings   {} exit",
        "[←/→]".bright_black(),
        seek_step,
        "[↑/↓]".bright_black(),
        vol_step,
        "[Space]".bright_black(),
        "[n/p]".bright_black(),
        "[f]".bright_black(),
        "[r]".bright_black(),
        "[s]".bright_black(),
        "[Esc/q]".bright_black(),
    );
    let _ = out.flush();
}

async fn manage_player_settings(client: &mut SpotifyClient) -> Result<()> {
    loop {
        println!("\n{}", "── Player Settings ──".bold().yellow());
        let options = vec![
            format!("1. Change Seek Step (Current: {}s)", client.config.status_settings.seek_step),
            format!("2. Change Volume Step (Current: {}%)", client.config.volume_settings.step),
            "3. Return to Player".to_string(),
        ];

        let choice = inquire::Select::new("Select settings option:", options).prompt()?;

        if choice.starts_with("1.") {
            let new_step: u32 = inquire::CustomType::new("Enter seek step in seconds (1-60):")
                .with_default(client.config.status_settings.seek_step)
                .with_error_message("Please enter a number between 1 and 60")
                .prompt()?;
            client.config.status_settings.seek_step = new_step.clamp(1, 60);
            client.config.save()?;
            println!("{}", format!("✓ Seek step updated to {}s", client.config.status_settings.seek_step).green());
        } else if choice.starts_with("2.") {
            let new_step: u32 = inquire::CustomType::new("Enter volume step percentage (1-50):")
                .with_default(client.config.volume_settings.step)
                .with_error_message("Please enter a number between 1 and 50")
                .prompt()?;
            client.config.volume_settings.step = new_step.clamp(1, 50);
            client.config.save()?;
            println!("{}", format!("✓ Volume step updated to {}%", client.config.volume_settings.step).green());
        } else {
            break;
        }
    }
    println!();
    Ok(())
}

pub async fn run_play(
    client: &mut SpotifyClient,
    query: Option<String>,
    play_type: &str, // "track", "album", "playlist"
) -> Result<()> {
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
    let _ = run_status(client, true).await;
    Ok(())
}

pub async fn run_previous(client: &mut SpotifyClient) -> Result<()> {
    client.previous(None).await?;
    println!("{}", "⏮ Skipped to previous track.".green());
    tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
    let _ = run_status(client, true).await;
    Ok(())
}

pub async fn run_seek(client: &mut SpotifyClient, time_str: &str) -> Result<()> {
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
