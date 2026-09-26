use crate::api::errors::SpotifyError;
use crate::api::SpotifyClient;
use crate::config::StatusSettings;
use crate::models::{PlaybackState, QueueResponse, Track};
use crate::ui::{format_duration, print_playback_status};
use anyhow::Result;
use colored::Colorize;
use crossterm::cursor::{Hide, MoveTo, MoveToColumn, MoveUp, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use std::io::{stdout, Write};
use std::time::{Duration, Instant};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn visible_width(s: &str) -> usize {
    strip_ansi(s).width()
}

fn truncate_str(s: &str, max_len: usize) -> String {
    let total_width = s.width();
    if total_width <= max_len {
        return s.to_string();
    }

    let target_width = max_len.saturating_sub(2);
    let mut current_width = 0;
    let mut truncated = String::new();
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if current_width + cw > target_width {
            break;
        }
        truncated.push(c);
        current_width += cw;
    }
    format!("{}..", truncated)
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
    let mut show_queue = false;
    let mut queue_cache: Option<QueueResponse> = None;
    let mut last_rendered_lines: u16 = PLAYER_LINES;

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

        let mut force_render = false;

        // Song completed check: fetch next song from Spotify API
        if state.is_playing && state.current_ms >= state.duration_ms && state.duration_ms > 0 {
            tokio::time::sleep(Duration::from_millis(600)).await;
            if let Ok(Some(fresh)) = client.get_playback_state().await {
                state.update_from_playback(&fresh);
                if show_queue {
                    queue_cache = client.get_queue().await.ok();
                }
                force_render = true;
            }
        }

        // Periodic API sync (every 20s) to prevent clock drift
        if now.duration_since(state.last_api_sync).as_secs() >= 20 {
            if let Ok(Some(fresh)) = client.get_playback_state().await {
                state.update_from_playback(&fresh);
                if show_queue {
                    queue_cache = client.get_queue().await.ok();
                }
                force_render = true;
            }
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
            let key_str = match code {
                KeyCode::Left => "Left".to_string(),
                KeyCode::Right => "Right".to_string(),
                KeyCode::Up => "Up".to_string(),
                KeyCode::Down => "Down".to_string(),
                KeyCode::Esc => "Esc".to_string(),
                KeyCode::Enter => "Enter".to_string(),
                KeyCode::Char(' ') => "Space".to_string(),
                KeyCode::Char(c) => c.to_string(),
                _ => String::new(),
            };

            let action = client
                .config
                .status_settings
                .find_action(&key_str)
                .map(|s| s.to_string());

            match action.as_deref() {
                Some("seek_forward") => {
                    let target_ms = (state.current_ms + (seek_step as u64 * 1000)).min(state.duration_ms);
                    state.current_ms = target_ms;
                    state.last_tick = Instant::now();
                    let _ = client.seek(target_ms, None).await;
                    force_render = true;
                }
                Some("seek_backward") => {
                    let target_ms = state.current_ms.saturating_sub(seek_step as u64 * 1000);
                    state.current_ms = target_ms;
                    state.last_tick = Instant::now();
                    let _ = client.seek(target_ms, None).await;
                    force_render = true;
                }
                Some("vol_up") => {
                    state.volume_percent = (state.volume_percent + vol_step).min(100);
                    let _ = client.set_volume(state.volume_percent, None).await;
                    force_render = true;
                }
                Some("vol_down") => {
                    state.volume_percent = state.volume_percent.saturating_sub(vol_step);
                    let _ = client.set_volume(state.volume_percent, None).await;
                    force_render = true;
                }
                Some("toggle") => {
                    if state.is_playing {
                        let _ = client.pause(None).await;
                        state.is_playing = false;
                    } else {
                        let _ = client.play(None, None, None, None).await;
                        state.is_playing = true;
                        state.last_tick = Instant::now();
                    }
                    force_render = true;
                }
                Some("next") => {
                    let _ = client.next(None).await;
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    if let Ok(Some(fresh)) = client.get_playback_state().await {
                        state.update_from_playback(&fresh);
                    }
                    if show_queue {
                        queue_cache = client.get_queue().await.ok();
                    }
                    force_render = true;
                }
                Some("prev") => {
                    let _ = client.previous(None).await;
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    if let Ok(Some(fresh)) = client.get_playback_state().await {
                        state.update_from_playback(&fresh);
                    }
                    if show_queue {
                        queue_cache = client.get_queue().await.ok();
                    }
                    force_render = true;
                }
                Some("shuffle") => {
                    state.shuffle_state = !state.shuffle_state;
                    let _ = client.set_shuffle(state.shuffle_state, None).await;
                    force_render = true;
                }
                Some("repeat") => {
                    state.repeat_state = match state.repeat_state.as_str() {
                        "off" => "context",
                        "context" => "track",
                        _ => "off",
                    }
                    .to_string();
                    let _ = client.set_repeat(&state.repeat_state, None).await;
                    force_render = true;
                }
                Some("queue") => {
                    show_queue = !show_queue;
                    if show_queue {
                        queue_cache = client.get_queue().await.ok();
                    }
                    force_render = true;
                }
                Some("settings") => {
                    let _ = crossterm::execute!(stdout, Show);
                    println!();
                    manage_player_settings(client).await?;
                    let _ = crossterm::execute!(stdout, Clear(ClearType::All), MoveTo(0, 0));
                    println!("\n{}", "── Spotify Interactive Real-Time Player ──".green().bold());
                    let _ = crossterm::execute!(stdout, Hide);
                    first_render = true;
                    force_render = true;
                    last_rendered_lines = PLAYER_LINES;
                }
                Some("exit") => {
                    let _ = crossterm::execute!(stdout, Show);
                    println!("\n\n{}", "Exited interactive player.".bright_black());
                    break;
                }
                _ => {}
            }
        }

        let current_display_sec = state.current_ms / 1000;
        if first_render || force_render || current_display_sec != last_display_sec {
            let queue_slice = if show_queue {
                queue_cache.as_ref().map(|q| q.queue.as_slice())
            } else {
                None
            };
            last_rendered_lines = render_player(
                &state,
                &client.config.status_settings,
                vol_step,
                queue_slice,
                show_queue,
                first_render,
                last_rendered_lines,
            );
            first_render = false;
            last_display_sec = current_display_sec;
        }
    }

    let _ = crossterm::execute!(stdout, Show);
    Ok(())
}

const PLAYER_LINES: u16 = 10;

fn render_player(
    state: &LivePlayerState,
    status_settings: &StatusSettings,
    vol_step: u32,
    queue_tracks: Option<&[Track]>,
    show_queue: bool,
    first_render: bool,
    last_rendered_lines: u16,
) -> u16 {
    let term_width = crossterm::terminal::size().map(|(w, _)| w as usize).unwrap_or(80);
    let box_width = term_width.clamp(68, 76);
    let inner_width = box_width.saturating_sub(4);

    let play_badge = if state.is_playing {
        "▶ PLAYING".green().bold()
    } else {
        "⏸ PAUSED".yellow().bold()
    };

    let device_badge = format!("💻 {}", truncate_str(&state.device_name, 22)).bright_black();

    // Volume slider
    let vol_len: usize = 6;
    let vol_ratio = (state.volume_percent as f32 / 100.0).clamp(0.0, 1.0);
    let vol_filled = ((vol_len as f32) * vol_ratio).round() as usize;
    let vol_empty = vol_len.saturating_sub(vol_filled);
    let vol_slider = format!(
        "🔊 {:>3}% [{}{}]",
        state.volume_percent,
        "━".repeat(vol_filled).magenta(),
        "─".repeat(vol_empty).bright_black()
    );

    let title_line = format!("🎵 {}", truncate_str(&state.track_name, inner_width.saturating_sub(25).min(38))).bold().white();
    let meta_raw = if state.album.is_empty() {
        state.artists.clone()
    } else {
        format!("{} • {}", state.artists, state.album)
    };
    let meta_line = format!("   {}", truncate_str(&meta_raw, inner_width.saturating_sub(20).min(44))).cyan();
    let seek_info = format!("seek: ±{}s", status_settings.seek_step).bright_black();

    // Control buttons (Spotify layout)
    let seek_keys = status_settings.keys_for_action("seek_forward");
    let seek_back_keys = status_settings.keys_for_action("seek_backward");
    let vol_up_keys = status_settings.keys_for_action("vol_up");
    let vol_down_keys = status_settings.keys_for_action("vol_down");
    let toggle_keys = status_settings.keys_for_action("toggle");
    let next_keys = status_settings.keys_for_action("next");
    let prev_keys = status_settings.keys_for_action("prev");
    let shuffle_keys = status_settings.keys_for_action("shuffle");
    let repeat_keys = status_settings.keys_for_action("repeat");
    let queue_keys = status_settings.keys_for_action("queue");
    let settings_keys = status_settings.keys_for_action("settings");
    let exit_keys = status_settings.keys_for_action("exit");

    let seek_display = if seek_back_keys == "Left" && seek_keys == "Right" {
        "←/→".to_string()
    } else {
        format!("{}/{}", seek_back_keys, seek_keys)
    };

    let vol_display = if vol_down_keys == "Down" && vol_up_keys == "Up" {
        "↑/↓".to_string()
    } else {
        format!("{}/{}", vol_down_keys, vol_up_keys)
    };

    let track_display = if prev_keys == "p" && next_keys == "n" {
        "n/p".to_string()
    } else {
        format!("{}/{}", prev_keys, next_keys)
    };

    let queue_display = if queue_keys.is_empty() { "q".to_string() } else { queue_keys };
    let queue_badge = if show_queue {
        format!("[{}] queue (open)", queue_display).cyan().bold()
    } else {
        format!("[{}] queue", queue_display).bright_black()
    };

    let shuf_btn = if state.shuffle_state {
        format!("[🔀 on]").green().bold()
    } else {
        format!("[🔀 off]").bright_black()
    };
    let prev_btn = format!("[⏮ {}]", prev_keys).bright_black();
    let play_btn = if state.is_playing {
        format!("( ▶ {} )", toggle_keys).green().bold()
    } else {
        format!("( ⏸ {} )", toggle_keys).yellow().bold()
    };
    let next_btn = format!("[⏭ {}]", next_keys).bright_black();
    let rep_btn = match state.repeat_state.as_str() {
        "track" => format!("[🔂 track]").green().bold(),
        "context" => format!("[🔁 all]").green().bold(),
        _ => format!("[🔁 off]").bright_black(),
    };
    let controls_bar = format!("{}    {}    {}    {}    {}", shuf_btn, prev_btn, play_btn, next_btn, rep_btn);

    // Scrubber
    let scrubber_len = inner_width.saturating_sub(24).clamp(18, 30);
    let ratio = if state.duration_ms > 0 {
        (state.current_ms as f64 / state.duration_ms as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let knob_pos = ((scrubber_len as f64) * ratio).round() as usize;
    let filled = "━".repeat(knob_pos).green().bold();
    let knob = "●".white().bold();
    let unfilled = "─".repeat(scrubber_len.saturating_sub(knob_pos)).bright_black();
    let scrubber_bar = format!(
        "{}  {}{}{}  {}",
        format_duration(state.current_ms).cyan(),
        filled,
        knob,
        unfilled,
        format_duration(state.duration_ms).bright_black()
    );

    let mut out = stdout();
    if !first_render {
        let _ = crossterm::execute!(out, MoveUp(last_rendered_lines), MoveToColumn(0), Clear(ClearType::FromCursorDown));
    }

    // 1. Box Top Border
    println!("┌{}┐", "─".repeat(inner_width + 2));

    // 2. Status & Device
    let s_w = visible_width(&play_badge.to_string());
    let d_w = visible_width(&device_badge);
    let pad2 = inner_width.saturating_sub(s_w + d_w);
    println!("│ {}{}{} │", play_badge, " ".repeat(pad2), device_badge);

    // 3. Track Title & Volume Slider
    let t_w = visible_width(&title_line.to_string());
    let v_w = visible_width(&vol_slider);
    let pad3 = inner_width.saturating_sub(t_w + v_w);
    println!("│ {}{}{} │", title_line, " ".repeat(pad3), vol_slider);

    // 4. Artists, Album & Seek info
    let m_w = visible_width(&meta_line.to_string());
    let sk_w = visible_width(&seek_info);
    let pad4 = inner_width.saturating_sub(m_w + sk_w);
    println!("│ {}{}{} │", meta_line, " ".repeat(pad4), seek_info);

    // 5. Spacer
    println!("│ {} │", " ".repeat(inner_width));

    // 6. Centered Controls Bar
    let c_w = visible_width(&controls_bar);
    let pad_c_l = inner_width.saturating_sub(c_w) / 2;
    let pad_c_r = inner_width.saturating_sub(c_w + pad_c_l);
    println!("│ {}{}{} │", " ".repeat(pad_c_l), controls_bar, " ".repeat(pad_c_r));

    // 7. Centered Scrubber
    let sc_w = visible_width(&scrubber_bar);
    let pad_s_l = inner_width.saturating_sub(sc_w) / 2;
    let pad_s_r = inner_width.saturating_sub(sc_w + pad_s_l);
    println!("│ {}{}{} │", " ".repeat(pad_s_l), scrubber_bar, " ".repeat(pad_s_r));

    // 8. Box Bottom Border
    println!("└{}┘", "─".repeat(inner_width + 2));

    // 9. Keybindings Description Row 1
    println!(
        "  {} seek {}s   {} vol {}%   {} play/pause   {} track",
        format!("[{}]", seek_display).bright_black(),
        status_settings.seek_step,
        format!("[{}]", vol_display).bright_black(),
        vol_step,
        format!("[{}]", toggle_keys).bright_black(),
        format!("[{}]", track_display).bright_black(),
    );

    // 10. Keybindings Description Row 2
    println!(
        "  {} shuffle   {} repeat   {}   {} settings   {} exit",
        format!("[{}]", shuffle_keys).bright_black(),
        format!("[{}]", repeat_keys).bright_black(),
        queue_badge,
        format!("[{}]", settings_keys).bright_black(),
        format!("[{}]", exit_keys).bright_black(),
    );

    let mut lines_printed = PLAYER_LINES;

    if show_queue {
        let q_title = "┌── Next in Queue ";
        let q_w = visible_width(q_title);
        let q_border_dash = (inner_width + 2).saturating_sub(q_w);
        println!("{}{}", q_title.cyan().bold(), format!("{}┐", "─".repeat(q_border_dash)).bright_black());
        lines_printed += 1;

        match queue_tracks {
            Some(tracks) if !tracks.is_empty() => {
                for (i, t) in tracks.iter().take(5).enumerate() {
                    let num_title = format!(" {:>2}. {}", i + 1, truncate_str(&t.name, 28));
                    let artist = truncate_str(&t.artists_str(), 18);
                    let dur = format_duration(t.duration_ms);

                    let nt_w = visible_width(&num_title);
                    let a_w = visible_width(&artist);
                    let d_w = visible_width(&dur);

                    let gap1 = 32usize.saturating_sub(nt_w).max(2);
                    let gap2 = inner_width.saturating_sub(nt_w + gap1 + a_w + d_w);

                    let line_content = format!(
                        "{}{}{}{}{}",
                        num_title.white(),
                        " ".repeat(gap1),
                        artist.bright_black(),
                        " ".repeat(gap2),
                        dur.cyan()
                    );
                    println!("│ {} │", line_content);
                    lines_printed += 1;
                }
            }
            Some(_) => {
                let msg = "   (Queue is empty)".bright_black();
                let pad = inner_width.saturating_sub(visible_width(&msg.to_string()));
                println!("│ {}{} │", msg, " ".repeat(pad));
                lines_printed += 1;
            }
            None => {
                let msg = "   Loading queue...".bright_black();
                let pad = inner_width.saturating_sub(visible_width(&msg.to_string()));
                println!("│ {}{}{} │", msg, " ".repeat(pad), "");
                lines_printed += 1;
            }
        }

        println!("└{}┘", "─".repeat(inner_width + 2));
        lines_printed += 1;
    }

    let _ = out.flush();
    lines_printed
}

async fn manage_player_settings(client: &mut SpotifyClient) -> Result<()> {
    loop {
        println!("\n{}", "── Player Settings ──".bold().yellow());
        let options = vec![
            format!("1. Change Seek Step (Current: {}s)", client.config.status_settings.seek_step),
            format!("2. Change Volume Step (Current: {}%)", client.config.volume_settings.step),
            "3. View & Manage Keybindings (Add / Delete)".to_string(),
            "4. Return to Player".to_string(),
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
        } else if choice.starts_with("3.") {
            manage_status_keybindings(client)?;
        } else {
            break;
        }
    }
    println!();
    Ok(())
}

fn manage_status_keybindings(client: &mut SpotifyClient) -> Result<()> {
    loop {
        println!("\n{}", "Current Player Keybindings:".bold());
        for (i, b) in client.config.status_settings.bindings.iter().enumerate() {
            println!("  {:>2}. Key: {:<10} -> Action: {}", i + 1, format!("[{}]", b.key).cyan(), b.action.yellow());
        }

        let options = vec![
            "Add new keybinding",
            "Delete an existing keybinding",
            "Reset keybindings to default",
            "Back to settings",
        ];

        let action = inquire::Select::new("Keybinding action:", options).prompt()?;

        match action {
            "Add new keybinding" => {
                let action_choices = vec![
                    "seek_forward",
                    "seek_backward",
                    "vol_up",
                    "vol_down",
                    "toggle",
                    "next",
                    "prev",
                    "shuffle",
                    "repeat",
                    "queue",
                    "settings",
                    "exit",
                ];
                let selected_action = inquire::Select::new("Bind to which action?", action_choices).prompt()?;

                let key_input: String = inquire::Text::new("Press or type key name (e.g. d, a, w, Space, Right, Left, Up, Down, q, Esc):")
                    .with_placeholder("e.g. d")
                    .prompt()?;

                let trimmed_key = key_input.trim().to_string();
                if trimmed_key.is_empty() {
                    println!("{}", "Key cannot be empty.".red());
                    continue;
                }

                // Check if already bound
                if client.config.status_settings.bindings.iter().any(|b| b.key.to_lowercase() == trimmed_key.to_lowercase()) {
                    println!("{}", format!("Key '{}' is already bound to an action.", trimmed_key).yellow());
                    continue;
                }

                client.config.status_settings.bindings.push(crate::config::KeyBinding {
                    action: selected_action.to_string(),
                    key: trimmed_key.clone(),
                });
                client.config.save()?;
                println!("{}", format!("✓ Added [{}] for action '{}'", trimmed_key, selected_action).green());
            }
            "Delete an existing keybinding" => {
                let choices: Vec<String> = client
                    .config
                    .status_settings
                    .bindings
                    .iter()
                    .enumerate()
                    .map(|(i, b)| format!("{}. [{}] for {}", i + 1, b.key, b.action))
                    .collect();

                let to_delete = inquire::Select::new("Select keybinding to delete:", choices).prompt()?;
                let idx = to_delete.split('.').next().unwrap().parse::<usize>()? - 1;

                // Safety Rule: Do not allow deleting if it's the only key for that action!
                if !client.config.status_settings.can_delete_binding(idx) {
                    let action = &client.config.status_settings.bindings[idx].action;
                    println!(
                        "{}",
                        format!(
                            "❌ Cannot delete: At least one keybinding must remain for action '{}'!",
                            action
                        )
                        .red()
                        .bold()
                    );
                    continue;
                }

                let removed = client.config.status_settings.bindings.remove(idx);
                client.config.save()?;
                println!("{}", format!("✓ Deleted [{}] for {}", removed.key, removed.action).green());
            }
            "Reset keybindings to default" => {
                client.config.status_settings = crate::config::StatusSettings::default();
                client.config.save()?;
                println!("{}", "✓ Keybindings reset to defaults!".green());
            }
            _ => break,
        }
    }
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
