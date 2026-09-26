use crate::api::SpotifyClient;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TestLogger {
    log_file: File,
    log_path: String,
    passed: u32,
    failed: u32,
    skipped: u32,
}

impl TestLogger {
    fn new() -> Result<Self> {
        let logs_dir = std::path::Path::new("logs");
        create_dir_all(logs_dir).context("Failed to create logs directory")?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let filename = format!("logs/test_run_{}.log", now);
        let mut file = File::create(&filename)
            .with_context(|| format!("Failed to create log file: {}", filename))?;

        writeln!(file, "==================================================")?;
        writeln!(file, "Spotify CLI Automated Diagnostic Test Run")?;
        writeln!(file, "Timestamp: {}", now)?;
        writeln!(file, "==================================================\n")?;

        Ok(Self {
            log_file: file,
            log_path: filename,
            passed: 0,
            failed: 0,
            skipped: 0,
        })
    }

    fn record_step(
        &mut self,
        test_name: &str,
        method: &str,
        endpoint: &str,
        payload: Option<&str>,
        status: Option<u16>,
        response_data: &str,
        is_success: bool,
    ) {
        if is_success {
            self.passed += 1;
            println!("  {} {} - {}", "[PASS]".green().bold(), test_name.bold(), format!("HTTP {}", status.unwrap_or(200)).bright_black());
        } else {
            self.failed += 1;
            println!("  {} {} - {}", "[FAIL]".red().bold(), test_name.bold(), response_data.red());
        }

        let _ = writeln!(self.log_file, "--------------------------------------------------");
        let _ = writeln!(self.log_file, "Result:   {}", if is_success { "PASS" } else { "FAIL" });
        let _ = writeln!(self.log_file, "Test:     {}", test_name);
        let _ = writeln!(self.log_file, "Request:  {} https://api.spotify.com/v1/{}", method, endpoint);
        let _ = writeln!(self.log_file, "Headers:  Authorization: Bearer [REDACTED], Content-Type: application/json");
        let _ = writeln!(self.log_file, "Payload:  {}", payload.unwrap_or("(none)"));
        if let Some(s) = status {
            let _ = writeln!(self.log_file, "Status:   {}", s);
        }
        let _ = writeln!(self.log_file, "Response:\n{}", response_data);
        let _ = writeln!(self.log_file, "--------------------------------------------------\n");
    }

    fn record_skip(&mut self, test_name: &str, reason: &str) {
        self.skipped += 1;
        println!("  {} {} - {}", "[SKIP]".yellow().bold(), test_name.bold(), reason.yellow());
        let _ = writeln!(self.log_file, "--------------------------------------------------");
        let _ = writeln!(self.log_file, "Result:   SKIP");
        let _ = writeln!(self.log_file, "Test:     {}", test_name);
        let _ = writeln!(self.log_file, "Reason:   {}", reason);
        let _ = writeln!(self.log_file, "--------------------------------------------------\n");
    }

    fn finish(&mut self) {
        println!("\n========================================================");
        let summary = format!(
            "Results: {} passed, {} failed, {} skipped",
            self.passed.to_string().green().bold(),
            self.failed.to_string().red().bold(),
            self.skipped.to_string().yellow().bold()
        );
        println!("{}", summary);
        println!("Full Diagnostic Log: {}", self.log_path.cyan());
        println!("========================================================\n");

        let _ = writeln!(self.log_file, "==================================================");
        let _ = writeln!(
            self.log_file,
            "Summary: {} passed, {} failed, {} skipped",
            self.passed, self.failed, self.skipped
        );
    }
}

pub async fn run_test(client: &mut SpotifyClient, filter_arg: Option<String>) -> Result<()> {
    let filter = match filter_arg {
        Some(f) => f.to_lowercase(),
        None => {
            let options = vec![
                "All",
                "Playback",
                "Volume",
                "Devices",
                "Search & Queue",
                "Library",
                "Exit",
            ];
            let choice = inquire::Select::new("Select test suite to run:", options)
                .prompt()
                .context("Test selection cancelled")?;
            if choice == "Exit" {
                return Ok(());
            }
            choice.to_lowercase()
        }
    };

    let mut logger = TestLogger::new()?;
    println!("\n{} Running Spotify CLI Diagnostics [{}]...\n", "▶".cyan().bold(), filter.cyan());

    let run_all = filter == "all" || filter == "6";

    // 1. Playback
    if run_all || filter.contains("play") || filter == "1" {
        test_playback(client, &mut logger).await;
    }

    // 2. Volume
    if run_all || filter.contains("vol") || filter == "2" {
        test_volume(client, &mut logger).await;
    }

    // 3. Devices
    if run_all || filter.contains("dev") || filter == "3" {
        test_devices(client, &mut logger).await;
    }

    // 4. Search & Queue
    if run_all || filter.contains("search") || filter.contains("queue") || filter == "4" {
        test_search_and_queue(client, &mut logger).await;
    }

    // 5. Library
    if run_all || filter.contains("lib") || filter == "5" {
        test_library(client, &mut logger).await;
    }

    logger.finish();
    Ok(())
}

async fn test_playback(client: &mut SpotifyClient, log: &mut TestLogger) {
    println!("{}", "Suite: Playback".bold().white());

    // Test 1: Get playback state
    match client.get_playback_state().await {
        Ok(state) => {
            let json_str = serde_json::to_string_pretty(&state).unwrap_or_default();
            log.record_step(
                "Playback State Query",
                "GET",
                "me/player",
                None,
                Some(200),
                &json_str,
                true,
            );

            // Test 2: If playing or paused, test pause/play toggle
            if let Some(s) = state {
                let initial_playing = s.is_playing;
                if initial_playing {
                    match client.pause(None).await {
                        Ok(_) => {
                            log.record_step(
                                "Pause Playback",
                                "PUT",
                                "me/player/pause",
                                Some("Content-Length: 0"),
                                Some(204),
                                "(empty response - paused)",
                                true,
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                            // Resume back
                            match client.play(None, None, None, None).await {
                                Ok(_) => log.record_step(
                                    "Resume Playback (Restore)",
                                    "PUT",
                                    "me/player/play",
                                    Some("Content-Length: 0"),
                                    Some(204),
                                    "(empty response - resumed)",
                                    true,
                                ),
                                Err(e) => log.record_step(
                                    "Resume Playback (Restore)",
                                    "PUT",
                                    "me/player/play",
                                    None,
                                    None,
                                    &e.to_string(),
                                    false,
                                ),
                            }
                        }
                        Err(e) => log.record_step(
                            "Pause Playback",
                            "PUT",
                            "me/player/pause",
                            None,
                            None,
                            &e.to_string(),
                            false,
                        ),
                    }
                } else {
                    match client.play(None, None, None, None).await {
                        Ok(_) => {
                            log.record_step(
                                "Resume Playback",
                                "PUT",
                                "me/player/play",
                                Some("Content-Length: 0"),
                                Some(204),
                                "(empty response - resumed)",
                                true,
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                            // Pause back
                            match client.pause(None).await {
                                Ok(_) => log.record_step(
                                    "Pause Playback (Restore)",
                                    "PUT",
                                    "me/player/pause",
                                    Some("Content-Length: 0"),
                                    Some(204),
                                    "(empty response - paused)",
                                    true,
                                ),
                                Err(e) => log.record_step(
                                    "Pause Playback (Restore)",
                                    "PUT",
                                    "me/player/pause",
                                    None,
                                    None,
                                    &e.to_string(),
                                    false,
                                ),
                            }
                        }
                        Err(e) => log.record_step(
                            "Resume Playback",
                            "PUT",
                            "me/player/play",
                            None,
                            None,
                            &e.to_string(),
                            false,
                        ),
                    }
                }
            } else {
                log.record_skip("Playback Control (Play/Pause)", "No active Spotify device to control");
            }
        }
        Err(e) => log.record_step(
            "Playback State Query",
            "GET",
            "me/player",
            None,
            None,
            &e.to_string(),
            false,
        ),
    }
    println!();
}

async fn test_volume(client: &mut SpotifyClient, log: &mut TestLogger) {
    println!("{}", "Suite: Volume".bold().white());

    match client.get_playback_state().await {
        Ok(Some(s)) => {
            if let Some(dev) = s.device {
                if let Some(orig_vol) = dev.volume_percent {
                    let test_vol = if orig_vol >= 10 { orig_vol - 5 } else { orig_vol + 5 };

                    // Set test volume
                    match client.set_volume(test_vol, None).await {
                        Ok(_) => {
                            log.record_step(
                                "Volume Adjustment",
                                "PUT",
                                &format!("me/player/volume?volume_percent={}", test_vol),
                                Some("Content-Length: 0"),
                                Some(204),
                                &format!("Volume set to {}%", test_vol),
                                true,
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

                            // Restore original volume
                            match client.set_volume(orig_vol, None).await {
                                Ok(_) => log.record_step(
                                    "Volume Restoration",
                                    "PUT",
                                    &format!("me/player/volume?volume_percent={}", orig_vol),
                                    Some("Content-Length: 0"),
                                    Some(204),
                                    &format!("Volume restored to {}%", orig_vol),
                                    true,
                                ),
                                Err(e) => log.record_step(
                                    "Volume Restoration",
                                    "PUT",
                                    &format!("me/player/volume?volume_percent={}", orig_vol),
                                    None,
                                    None,
                                    &e.to_string(),
                                    false,
                                ),
                            }
                        }
                        Err(e) => log.record_step(
                            "Volume Adjustment",
                            "PUT",
                            &format!("me/player/volume?volume_percent={}", test_vol),
                            None,
                            None,
                            &e.to_string(),
                            false,
                        ),
                    }
                } else {
                    log.record_skip("Volume Adjustment", "Device does not report volume percentage");
                }
            } else {
                log.record_skip("Volume Adjustment", "No active device found");
            }
        }
        Ok(None) => log.record_skip("Volume Adjustment", "No active playback session"),
        Err(e) => log.record_step(
            "Volume Query",
            "GET",
            "me/player",
            None,
            None,
            &e.to_string(),
            false,
        ),
    }
    println!();
}

async fn test_devices(client: &mut SpotifyClient, log: &mut TestLogger) {
    println!("{}", "Suite: Devices".bold().white());

    match client.get_devices().await {
        Ok(devices) => {
            let json_str = serde_json::to_string_pretty(&devices).unwrap_or_default();
            log.record_step(
                "Device Enumeration",
                "GET",
                "me/player/devices",
                None,
                Some(200),
                &json_str,
                true,
            );
        }
        Err(e) => log.record_step(
            "Device Enumeration",
            "GET",
            "me/player/devices",
            None,
            None,
            &e.to_string(),
            false,
        ),
    }
    println!();
}

async fn test_search_and_queue(client: &mut SpotifyClient, log: &mut TestLogger) {
    println!("{}", "Suite: Search & Queue".bold().white());

    // Search Track
    match client.search("Bohemian Rhapsody", "track", 5, 0).await {
        Ok(res) => {
            let total = res.tracks.as_ref().map(|t| t.items.len()).unwrap_or(0);
            let json_str = serde_json::to_string_pretty(&res.tracks).unwrap_or_default();
            if total > 0 {
                log.record_step(
                    "Track Search",
                    "GET",
                    "search?q=Bohemian%20Rhapsody&type=track&limit=5&offset=0",
                    None,
                    Some(200),
                    &json_str,
                    true,
                );
            } else {
                log.record_step(
                    "Track Search",
                    "GET",
                    "search?q=Bohemian%20Rhapsody&type=track&limit=5&offset=0",
                    None,
                    Some(200),
                    "No tracks returned in search response",
                    false,
                );
            }
        }
        Err(e) => log.record_step(
            "Track Search",
            "GET",
            "search?q=Bohemian%20Rhapsody&type=track",
            None,
            None,
            &e.to_string(),
            false,
        ),
    }

    // Search Album
    match client.search("Abbey Road", "album", 5, 0).await {
        Ok(res) => {
            let total = res.albums.as_ref().map(|a| a.items.len()).unwrap_or(0);
            let json_str = serde_json::to_string_pretty(&res.albums).unwrap_or_default();
            if total > 0 {
                log.record_step(
                    "Album Search",
                    "GET",
                    "search?q=Abbey%20Road&type=album&limit=5&offset=0",
                    None,
                    Some(200),
                    &json_str,
                    true,
                );
            } else {
                log.record_step(
                    "Album Search",
                    "GET",
                    "search?q=Abbey%20Road&type=album&limit=5&offset=0",
                    None,
                    Some(200),
                    "No albums returned in search response",
                    false,
                );
            }
        }
        Err(e) => log.record_step(
            "Album Search",
            "GET",
            "search?q=Abbey%20Road&type=album",
            None,
            None,
            &e.to_string(),
            false,
        ),
    }

    // Query Queue
    match client.get_queue().await {
        Ok(q) => {
            let json_str = serde_json::to_string_pretty(&q).unwrap_or_default();
            log.record_step(
                "Queue Inspection",
                "GET",
                "me/player/queue",
                None,
                Some(200),
                &json_str,
                true,
            );
        }
        Err(e) => log.record_step(
            "Queue Inspection",
            "GET",
            "me/player/queue",
            None,
            None,
            &e.to_string(),
            false,
        ),
    }
    println!();
}

async fn test_library(client: &mut SpotifyClient, log: &mut TestLogger) {
    println!("{}", "Suite: Library".bold().white());

    // History
    match client.get_recently_played(5).await {
        Ok(hist) => {
            let json_str = serde_json::to_string_pretty(&hist).unwrap_or_default();
            log.record_step(
                "Recently Played",
                "GET",
                "me/player/recently-played?limit=5",
                None,
                Some(200),
                &json_str,
                true,
            );
        }
        Err(e) => log.record_step(
            "Recently Played",
            "GET",
            "me/player/recently-played?limit=5",
            None,
            None,
            &e.to_string(),
            false,
        ),
    }

    // Top Tracks
    match client.get_user_top_tracks(5).await {
        Ok(tracks) => {
            let json_str = serde_json::to_string_pretty(&tracks).unwrap_or_default();
            log.record_step(
                "Top Tracks",
                "GET",
                "me/top/tracks?limit=5",
                None,
                Some(200),
                &json_str,
                true,
            );
        }
        Err(e) => log.record_step(
            "Top Tracks",
            "GET",
            "me/top/tracks?limit=5",
            None,
            None,
            &e.to_string(),
            false,
        ),
    }

    // Top Artists
    match client.get_user_top_artists(5).await {
        Ok(artists) => {
            let json_str = serde_json::to_string_pretty(&artists).unwrap_or_default();
            log.record_step(
                "Top Artists",
                "GET",
                "me/top/artists?limit=5",
                None,
                Some(200),
                &json_str,
                true,
            );
        }
        Err(e) => log.record_step(
            "Top Artists",
            "GET",
            "me/top/artists?limit=5",
            None,
            None,
            &e.to_string(),
            false,
        ),
    }
    println!();
}
