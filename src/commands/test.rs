use crate::api::SpotifyClient;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestLogger {
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

    fn record_pass(&mut self, test_name: &str, details: &str) {
        self.passed += 1;
        println!("  {} {} - {}", "[PASS]".green().bold(), test_name.bold(), details.bright_black());
        let _ = writeln!(self.log_file, "[PASS] {} | Details: {}", test_name, details);
    }

    fn record_fail(&mut self, test_name: &str, error: &str) {
        self.failed += 1;
        println!("  {} {} - {}", "[FAIL]".red().bold(), test_name.bold(), error.red());
        let _ = writeln!(self.log_file, "[FAIL] {} | Error: {}", test_name, error);
    }

    fn record_skip(&mut self, test_name: &str, reason: &str) {
        self.skipped += 1;
        println!("  {} {} - {}", "[SKIP]".yellow().bold(), test_name.bold(), reason.yellow());
        let _ = writeln!(self.log_file, "[SKIP] {} | Reason: {}", test_name, reason);
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
        println!("Detailed log: {}", self.log_path.cyan());
        println!("========================================================\n");

        let _ = writeln!(self.log_file, "\n==================================================");
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
            let info = match state {
                Some(ref s) => {
                    let playing = if s.is_playing { "Playing" } else { "Paused" };
                    let track = s.item.as_ref().map(|t| t.name.as_str()).unwrap_or("none");
                    format!("Status: {}, Track: {}", playing, track)
                }
                None => "No active playback session".to_string(),
            };
            log.record_pass("Playback State Query (GET /v1/me/player)", &info);

            // Test 2: If playing or paused, test pause/play toggle
            if let Some(s) = state {
                let initial_playing = s.is_playing;
                if initial_playing {
                    match client.pause(None).await {
                        Ok(_) => {
                            log.record_pass("Pause Command (PUT /v1/me/player/pause)", "HTTP 204 No Content");
                            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                            // Resume back
                            match client.play(None, None, None, None).await {
                                Ok(_) => log.record_pass("Resume Command (PUT /v1/me/player/play)", "State restored"),
                                Err(e) => log.record_fail("Resume Command", &e.to_string()),
                            }
                        }
                        Err(e) => log.record_fail("Pause Command", &e.to_string()),
                    }
                } else {
                    match client.play(None, None, None, None).await {
                        Ok(_) => {
                            log.record_pass("Play Command (PUT /v1/me/player/play)", "HTTP 204 No Content");
                            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                            // Pause back
                            match client.pause(None).await {
                                Ok(_) => log.record_pass("Pause Command (PUT /v1/me/player/pause)", "State restored"),
                                Err(e) => log.record_fail("Pause Command", &e.to_string()),
                            }
                        }
                        Err(e) => log.record_fail("Play Command", &e.to_string()),
                    }
                }
            } else {
                log.record_skip("Playback Control (Play/Pause)", "No active Spotify device to control");
            }
        }
        Err(e) => log.record_fail("Playback State Query", &e.to_string()),
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
                            log.record_pass(
                                "Volume Adjustment (PUT /v1/me/player/volume)",
                                &format!("Changed volume from {}% to {}%", orig_vol, test_vol),
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

                            // Restore original volume
                            match client.set_volume(orig_vol, None).await {
                                Ok(_) => log.record_pass("Volume Restoration", &format!("Restored to {}%", orig_vol)),
                                Err(e) => log.record_fail("Volume Restoration", &e.to_string()),
                            }
                        }
                        Err(e) => log.record_fail("Volume Adjustment", &e.to_string()),
                    }
                } else {
                    log.record_skip("Volume Adjustment", "Device does not report volume percentage");
                }
            } else {
                log.record_skip("Volume Adjustment", "No active device found");
            }
        }
        Ok(None) => log.record_skip("Volume Adjustment", "No active playback session"),
        Err(e) => log.record_fail("Volume Query", &e.to_string()),
    }
    println!();
}

async fn test_devices(client: &mut SpotifyClient, log: &mut TestLogger) {
    println!("{}", "Suite: Devices".bold().white());

    match client.get_devices().await {
        Ok(devices) => {
            let count = devices.len();
            let names: Vec<String> = devices.iter().map(|d| format!("{}({})", d.name, d.device_type)).collect();
            log.record_pass(
                "Device Enumeration (GET /v1/me/player/devices)",
                &format!("Found {} device(s): {}", count, names.join(", ")),
            );
        }
        Err(e) => log.record_fail("Device Enumeration", &e.to_string()),
    }
    println!();
}

async fn test_search_and_queue(client: &mut SpotifyClient, log: &mut TestLogger) {
    println!("{}", "Suite: Search & Queue".bold().white());

    // Search Track
    match client.search("Bohemian Rhapsody", "track", 5, 0).await {
        Ok(res) => {
            let total = res.tracks.as_ref().map(|t| t.items.len()).unwrap_or(0);
            if total > 0 {
                log.record_pass("Track Search (GET /v1/search?type=track)", &format!("Returned {} results", total));
            } else {
                log.record_fail("Track Search", "Expected >0 results for 'Bohemian Rhapsody'");
            }
        }
        Err(e) => log.record_fail("Track Search", &e.to_string()),
    }

    // Search Album
    match client.search("Abbey Road", "album", 5, 0).await {
        Ok(res) => {
            let total = res.albums.as_ref().map(|a| a.items.len()).unwrap_or(0);
            if total > 0 {
                log.record_pass("Album Search (GET /v1/search?type=album)", &format!("Returned {} results", total));
            } else {
                log.record_fail("Album Search", "Expected >0 results for 'Abbey Road'");
            }
        }
        Err(e) => log.record_fail("Album Search", &e.to_string()),
    }

    // Query Queue
    match client.get_queue().await {
        Ok(q) => {
            let curr = q.currently_playing.map(|t| t.name).unwrap_or_else(|| "none".to_string());
            log.record_pass(
                "Queue Inspection (GET /v1/me/player/queue)",
                &format!("Now playing: {}, Queue size: {}", curr, q.queue.len()),
            );
        }
        Err(e) => log.record_fail("Queue Inspection", &e.to_string()),
    }
    println!();
}

async fn test_library(client: &mut SpotifyClient, log: &mut TestLogger) {
    println!("{}", "Suite: Library".bold().white());

    // History
    match client.get_recently_played(5).await {
        Ok(hist) => {
            log.record_pass(
                "Recently Played (GET /v1/me/player/recently-played)",
                &format!("Returned {} items", hist.items.len()),
            );
        }
        Err(e) => log.record_fail("Recently Played", &e.to_string()),
    }

    // Top Tracks
    match client.get_user_top_tracks(5).await {
        Ok(tracks) => {
            log.record_pass(
                "Top Tracks (GET /v1/me/top/tracks)",
                &format!("Returned {} items", tracks.items.len()),
            );
        }
        Err(e) => log.record_fail("Top Tracks", &e.to_string()),
    }

    // Top Artists
    match client.get_user_top_artists(5).await {
        Ok(artists) => {
            log.record_pass(
                "Top Artists (GET /v1/me/top/artists)",
                &format!("Returned {} items", artists.items.len()),
            );
        }
        Err(e) => log.record_fail("Top Artists", &e.to_string()),
    }
    println!();
}
