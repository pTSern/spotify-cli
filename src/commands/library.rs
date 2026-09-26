use crate::api::SpotifyClient;
use crate::ui::format_duration;
use anyhow::Result;
use colored::Colorize;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, ContentArrangement, Table};

pub async fn run_save(client: &mut SpotifyClient, item_type: &str) -> Result<()> {
    let state = client.get_playback_state().await?;
    let track = state.and_then(|s| s.item).ok_or_else(|| {
        anyhow::anyhow!("No track currently playing to save.")
    })?;

    match item_type {
        "album" => {
            let album = track.album.ok_or_else(|| anyhow::anyhow!("Track has no album information"))?;
            let id = album.id.ok_or_else(|| anyhow::anyhow!("Album ID not found"))?;
            client.save_albums(&[&id]).await?;
            println!("✓ Saved album '{}' to your library!", album.name.bold().green());
        }
        _ => {
            // Default: save track
            let id = track.id.as_ref().ok_or_else(|| anyhow::anyhow!("Track ID not found"))?;
            client.save_tracks(&[id.as_str()]).await?;
            println!("✓ Saved '{} - {}' to Liked Songs!", track.name.bold().green(), track.artists_str().cyan());
        }
    }
    Ok(())
}

pub async fn run_history(client: &mut SpotifyClient, limit: u32) -> Result<()> {
    let resp = client.get_recently_played(limit).await?;
    if resp.items.is_empty() {
        println!("{}", "No recently played tracks found.".yellow());
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("#").fg(Color::Cyan),
            Cell::new("Track").fg(Color::Green),
            Cell::new("Artist").fg(Color::Yellow),
            Cell::new("Album").fg(Color::Blue),
            Cell::new("Played At").fg(Color::DarkGrey),
        ]);

    for (idx, item) in resp.items.iter().enumerate() {
        let album = item.track.album.as_ref().map(|a| a.name.as_str()).unwrap_or("");
        let played_at = item.played_at.split('T').next().unwrap_or(&item.played_at);
        table.add_row(vec![
            Cell::new(format!("{}", idx + 1)),
            Cell::new(&item.track.name),
            Cell::new(item.track.artists_str()),
            Cell::new(album),
            Cell::new(played_at),
        ]);
    }

    println!("\n{}", "Recently Played Tracks:".bold());
    println!("{table}\n");
    Ok(())
}

pub async fn run_top(client: &mut SpotifyClient, item_type: &str, limit: u32) -> Result<()> {
    if item_type == "artists" || item_type == "artist" {
        let artists = client.get_user_top_artists(limit).await?;
        if artists.items.is_empty() {
            println!("{}", "No top artists found.".yellow());
            return Ok(());
        }

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("Rank").fg(Color::Cyan),
                Cell::new("Artist").fg(Color::Green),
                Cell::new("Genres").fg(Color::DarkGrey),
            ]);

        for (idx, artist) in artists.items.iter().enumerate() {
            let genres = artist.genres.as_ref().map(|g| g.join(", ")).unwrap_or_default();
            table.add_row(vec![
                Cell::new(format!("{}", idx + 1)),
                Cell::new(&artist.name),
                Cell::new(genres),
            ]);
        }

        println!("\n{}", "Your Top Artists:".bold());
        println!("{table}\n");
    } else {
        let tracks = client.get_user_top_tracks(limit).await?;
        if tracks.items.is_empty() {
            println!("{}", "No top tracks found.".yellow());
            return Ok(());
        }

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("Rank").fg(Color::Cyan),
                Cell::new("Track").fg(Color::Green),
                Cell::new("Artist").fg(Color::Yellow),
                Cell::new("Album").fg(Color::Blue),
                Cell::new("Time").fg(Color::DarkGrey),
            ]);

        for (idx, track) in tracks.items.iter().enumerate() {
            let album = track.album.as_ref().map(|a| a.name.as_str()).unwrap_or("");
            table.add_row(vec![
                Cell::new(format!("{}", idx + 1)),
                Cell::new(&track.name),
                Cell::new(track.artists_str()),
                Cell::new(album),
                Cell::new(format_duration(track.duration_ms)),
            ]);
        }

        println!("\n{}", "Your Top Tracks:".bold());
        println!("{table}\n");
    }

    Ok(())
}
