pub mod progress;

use crate::models::{Device, PlaybackState};
use colored::Colorize;

pub fn format_duration(ms: u64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

pub fn print_playback_status(state: &PlaybackState) {
    let play_symbol = if state.is_playing {
        "▶ Playing".green().bold()
    } else {
        "⏸ Paused".yellow().bold()
    };

    println!("\n{}", play_symbol);

    if let Some(ref track) = state.item {
        println!("  {} {}", "Track: ".bright_black(), track.name.bold().white());
        println!("  {} {}", "Artist:".bright_black(), track.artists_str().cyan());
        if let Some(ref album) = track.album {
            println!("  {} {}", "Album: ".bright_black(), album.name.italic());
        }

        let progress = state.progress_ms.unwrap_or(0);
        let bar = progress::render_progress_bar(progress, track.duration_ms, 30);
        println!(
            "  {} {} {} / {}",
            "Time:  ".bright_black(),
            bar,
            format_duration(progress).cyan(),
            format_duration(track.duration_ms).bright_black()
        );
    } else {
        println!("  {}", "No track currently playing".bright_black());
    }

    if let Some(ref dev) = state.device {
        let vol = dev
            .volume_percent
            .map(|v| format!("{}%", v))
            .unwrap_or_else(|| "N/A".to_string());

        let shuffle_str = if state.shuffle_state {
            "🔀 on".green()
        } else {
            "🔀 off".bright_black()
        };

        let repeat_str = match state.repeat_state.as_str() {
            "track" => "🔂 track".green(),
            "context" => "🔁 all".green(),
            _ => "🔁 off".bright_black(),
        };

        println!(
            "  {} {} ({}) | Vol: {} | {} | {}",
            "Device:".bright_black(),
            dev.name.bold(),
            dev.device_type,
            vol.magenta(),
            shuffle_str,
            repeat_str
        );
    }

    println!();
}

pub fn print_device_list(devices: &[Device]) {
    println!("\n{}", "Available Spotify Devices:".bold());
    if devices.is_empty() {
        println!("  {}", "No devices found. Open Spotify on any computer or phone.".yellow());
        return;
    }

    for dev in devices {
        let prefix = if dev.is_active {
            "● (active)".green().bold()
        } else {
            "○         ".bright_black()
        };
        let vol = dev
            .volume_percent
            .map(|v| format!("{}%", v))
            .unwrap_or_default();
        println!(
            "  {} {} - {} [Volume: {}]",
            prefix,
            dev.name.white().bold(),
            dev.device_type.cyan(),
            vol
        );
    }
    println!();
}
