use crate::api::SpotifyClient;
use crate::ui::format_duration;
use anyhow::{bail, Result};
use colored::Colorize;

pub async fn run_queue(
    client: &mut SpotifyClient,
    query: Option<String>,
    queue_type: &str, // "track" or "album"
) -> Result<()> {
    if let Some(q) = query {
        let q_trimmed = q.trim();

        // Special case: `spotify queue .` queues current playing track
        if q_trimmed == "." {
            let state = client.get_playback_state().await?;
            if let Some(track) = state.and_then(|s| s.item) {
                if let Some(ref uri) = track.uri {
                    client.add_to_queue(uri, None).await?;
                    println!("Queued current track: {} - {}", track.name.bold(), track.artists_str().cyan());
                    return Ok(());
                }
            }
            bail!("No track currently playing to add to queue");
        }

        // Direct URI
        if q_trimmed.starts_with("spotify:track:") {
            client.add_to_queue(q_trimmed, None).await?;
            println!("Added URI to queue: {}", q_trimmed.cyan());
            return Ok(());
        }

        // Search and queue
        println!("Searching for {} to queue: '{}'...", queue_type, q_trimmed.cyan());
        let results = client.search(q_trimmed, queue_type, 5, 0).await?;

        if queue_type == "album" {
            if let Some(albums) = results.albums {
                if let Some(first) = albums.items.first() {
                    println!("Queueing album: {}", first.name.bold());
                    if let Some(ref uri) = first.uri {
                        client.add_to_queue(uri, None).await?;
                        println!("{}", "✓ Album added to queue!".green());
                    }
                } else {
                    println!("{}", "No albums found.".yellow());
                }
            }
        } else {
            if let Some(tracks) = results.tracks {
                if let Some(first) = tracks.items.first() {
                    if let Some(ref uri) = first.uri {
                        client.add_to_queue(uri, None).await?;
                        println!(
                            "Queued: {} - {} ({})",
                            first.name.bold().white(),
                            first.artists_str().cyan(),
                            format_duration(first.duration_ms).bright_black()
                        );
                    }
                } else {
                    println!("{}", "No tracks found.".yellow());
                }
            }
        }
    } else {
        // Display queue
        let queue_resp = client.get_queue().await?;
        println!("\n{}", "Playback Queue:".bold());

        if let Some(curr) = queue_resp.currently_playing {
            println!(
                "  {} {} - {}",
                "Now Playing:".green().bold(),
                curr.name.white().bold(),
                curr.artists_str().cyan()
            );
        }

        if queue_resp.queue.is_empty() {
            println!("  {}", "Queue is empty.".bright_black());
        } else {
            println!("  {}", "Up Next:".bright_black());
            for (idx, track) in queue_resp.queue.iter().take(15).enumerate() {
                println!(
                    "    {}. {} - {} [{}]",
                    idx + 1,
                    track.name.white(),
                    track.artists_str().cyan(),
                    format_duration(track.duration_ms).bright_black()
                );
            }
            if queue_resp.queue.len() > 15 {
                println!("    ... and {} more tracks in queue", queue_resp.queue.len() - 15);
            }
            println!();

            // Interactive track selection from queue
            let mut options = vec!["Exit queue".to_string()];
            for (idx, track) in queue_resp.queue.iter().take(15).enumerate() {
                options.push(format!(
                    "Play {}. {} - {}",
                    idx + 1,
                    track.name,
                    track.artists_str()
                ));
            }

            if let Ok(choice) = inquire::Select::new("Select a track from queue to play directly:", options).prompt() {
                if choice != "Exit queue" {
                    let parts: Vec<&str> = choice.split_whitespace().collect();
                    if let Some(num_str) = parts.get(1) {
                        let num_clean = num_str.trim_end_matches('.');
                        if let Ok(idx) = num_clean.parse::<usize>() {
                            if idx > 0 && idx <= queue_resp.queue.len() {
                                let target_track = &queue_resp.queue[idx - 1];
                                if let Some(ref uri) = target_track.uri {
                                    client.play(None, Some(vec![uri.clone()]), None, None).await?;
                                    println!(
                                        "▶ Now playing from queue: {} - {}",
                                        target_track.name.bold().green(),
                                        target_track.artists_str().cyan()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        println!();
    }

    Ok(())
}
