use crate::api::SpotifyClient;
use crate::models::{Album, Artist, Playlist, Track};
use crate::ui::format_duration;
use anyhow::{Context, Result};
use colored::Colorize;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, ContentArrangement, Table};

pub async fn run_search(
    client: &mut SpotifyClient,
    query: &str,
    search_type: &str, // "track", "album", "artist", "playlist"
    limit: u32,
) -> Result<()> {
    let mut offset = 0;
    println!("Searching for {} '{}'...", search_type, query.cyan());

    loop {
        let results = client.search(query, search_type, limit, offset).await?;

        match search_type {
            "album" => {
                let albums = match results.albums {
                    Some(a) if !a.items.is_empty() => a,
                    _ => {
                        println!("{}", "No albums found.".yellow());
                        return Ok(());
                    }
                };
                display_albums_table(&albums.items, offset);
                if !handle_album_interaction(client, &albums.items, &mut offset, limit, albums.total).await? {
                    break;
                }
            }
            "artist" => {
                let artists = match results.artists {
                    Some(a) if !a.items.is_empty() => a,
                    _ => {
                        println!("{}", "No artists found.".yellow());
                        return Ok(());
                    }
                };
                display_artists_table(&artists.items, offset);
                break;
            }
            "playlist" => {
                let playlists = match results.playlists {
                    Some(p) if !p.items.is_empty() => p,
                    _ => {
                        println!("{}", "No playlists found.".yellow());
                        return Ok(());
                    }
                };
                display_playlists_table(&playlists.items, offset);
                if !handle_playlist_interaction(client, &playlists.items, &mut offset, limit, playlists.total).await? {
                    break;
                }
            }
            _ => {
                // Default: track
                let tracks = match results.tracks {
                    Some(t) if !t.items.is_empty() => t,
                    _ => {
                        println!("{}", "No tracks found.".yellow());
                        return Ok(());
                    }
                };
                display_tracks_table(&tracks.items, offset);
                if !handle_track_interaction(client, &tracks.items, &mut offset, limit, tracks.total).await? {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn display_tracks_table(tracks: &[Track], offset: u32) {
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
            Cell::new("Time").fg(Color::DarkGrey),
        ]);

    for (idx, track) in tracks.iter().enumerate() {
        let album_name = track.album.as_ref().map(|a| a.name.as_str()).unwrap_or("");
        table.add_row(vec![
            Cell::new(format!("{}", offset + idx as u32 + 1)),
            Cell::new(&track.name),
            Cell::new(track.artists_str()),
            Cell::new(album_name),
            Cell::new(format_duration(track.duration_ms)),
        ]);
    }

    println!("\n{table}\n");
}

fn display_albums_table(albums: &[Album], offset: u32) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("#").fg(Color::Cyan),
            Cell::new("Album").fg(Color::Green),
            Cell::new("Artist").fg(Color::Yellow),
            Cell::new("Tracks").fg(Color::DarkGrey),
            Cell::new("Released").fg(Color::DarkGrey),
        ]);

    for (idx, album) in albums.iter().enumerate() {
        let artists = album
            .artists
            .as_ref()
            .map(|a| a.iter().map(|x| x.name.as_str()).collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        let total = album.total_tracks.map(|t| t.to_string()).unwrap_or_default();
        let release = album.release_date.as_deref().unwrap_or("");

        table.add_row(vec![
            Cell::new(format!("{}", offset + idx as u32 + 1)),
            Cell::new(&album.name),
            Cell::new(artists),
            Cell::new(total),
            Cell::new(release),
        ]);
    }

    println!("\n{table}\n");
}

fn display_artists_table(artists: &[Artist], offset: u32) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("#").fg(Color::Cyan),
            Cell::new("Artist").fg(Color::Green),
            Cell::new("Followers").fg(Color::Yellow),
            Cell::new("Genres").fg(Color::DarkGrey),
        ]);

    for (idx, artist) in artists.iter().enumerate() {
        let followers = artist
            .followers
            .as_ref()
            .and_then(|f| f.total)
            .map(|n| format!("{n}"))
            .unwrap_or_default();
        let genres = artist.genres.as_ref().map(|g| g.join(", ")).unwrap_or_default();

        table.add_row(vec![
            Cell::new(format!("{}", offset + idx as u32 + 1)),
            Cell::new(&artist.name),
            Cell::new(followers),
            Cell::new(genres),
        ]);
    }

    println!("\n{table}\n");
}

fn display_playlists_table(playlists: &[Playlist], offset: u32) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("#").fg(Color::Cyan),
            Cell::new("Playlist").fg(Color::Green),
            Cell::new("Owner").fg(Color::Yellow),
            Cell::new("Tracks").fg(Color::DarkGrey),
        ]);

    for (idx, pl) in playlists.iter().enumerate() {
        let owner = pl.owner.as_ref().and_then(|o| o.display_name.as_deref()).unwrap_or("");
        let tracks = pl.tracks.as_ref().and_then(|t| t.total).map(|n| n.to_string()).unwrap_or_default();

        table.add_row(vec![
            Cell::new(format!("{}", offset + idx as u32 + 1)),
            Cell::new(&pl.name),
            Cell::new(owner),
            Cell::new(tracks),
        ]);
    }

    println!("\n{table}\n");
}

async fn handle_track_interaction(
    client: &mut SpotifyClient,
    tracks: &[Track],
    offset: &mut u32,
    limit: u32,
    total: u32,
) -> Result<bool> {
    let mut options = vec!["Play a track", "Queue a track", "Save a track"];
    if *offset + limit < total {
        options.push("Next page");
    }
    options.push("Exit search");

    let choice = inquire::Select::new("Action:", options).prompt().context("Cancelled")?;

    match choice {
        "Play a track" => {
            let track_labels: Vec<String> = tracks
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {} - {}", i + 1, t.name, t.artists_str()))
                .collect();
            let selected = inquire::Select::new("Select track to play:", track_labels).prompt()?;
            let idx = selected.split('.').next().unwrap().parse::<usize>()? - 1;
            let track = &tracks[idx];

            if let Some(ref album) = track.album {
                client.play(album.uri.clone(), None, track.uri.clone(), None).await?;
            } else {
                client.play(None, Some(vec![track.uri.clone().unwrap()]), None, None).await?;
            }
            println!("▶ Now playing: {} - {}", track.name.bold(), track.artists_str().cyan());
            Ok(false)
        }
        "Queue a track" => {
            let track_labels: Vec<String> = tracks
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {} - {}", i + 1, t.name, t.artists_str()))
                .collect();
            let selected = inquire::Select::new("Select track to queue:", track_labels).prompt()?;
            let idx = selected.split('.').next().unwrap().parse::<usize>()? - 1;
            let track = &tracks[idx];

            if let Some(ref uri) = track.uri {
                client.add_to_queue(uri, None).await?;
                println!("✓ Queued: {} - {}", track.name.bold(), track.artists_str().cyan());
            }
            Ok(false)
        }
        "Save a track" => {
            let track_labels: Vec<String> = tracks
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {} - {}", i + 1, t.name, t.artists_str()))
                .collect();
            let selected = inquire::Select::new("Select track to save to library:", track_labels).prompt()?;
            let idx = selected.split('.').next().unwrap().parse::<usize>()? - 1;
            let track = &tracks[idx];

            if let Some(ref id) = track.id {
                client.save_tracks(&[id.as_str()]).await?;
                println!("✓ Saved to Liked Songs: {} - {}", track.name.bold(), track.artists_str().cyan());
            }
            Ok(false)
        }
        "Next page" => {
            *offset += limit;
            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn handle_album_interaction(
    client: &mut SpotifyClient,
    albums: &[Album],
    offset: &mut u32,
    limit: u32,
    total: u32,
) -> Result<bool> {
    let mut options = vec!["Play an album", "Queue an album"];
    if *offset + limit < total {
        options.push("Next page");
    }
    options.push("Exit search");

    let choice = inquire::Select::new("Action:", options).prompt().context("Cancelled")?;

    match choice {
        "Play an album" => {
            let labels: Vec<String> = albums.iter().enumerate().map(|(i, a)| format!("{}. {}", i + 1, a.name)).collect();
            let selected = inquire::Select::new("Select album to play:", labels).prompt()?;
            let idx = selected.split('.').next().unwrap().parse::<usize>()? - 1;
            let album = &albums[idx];
            client.play(album.uri.clone(), None, None, None).await?;
            println!("▶ Now playing album: {}", album.name.bold());
            Ok(false)
        }
        "Queue an album" => {
            let labels: Vec<String> = albums.iter().enumerate().map(|(i, a)| format!("{}. {}", i + 1, a.name)).collect();
            let selected = inquire::Select::new("Select album to queue:", labels).prompt()?;
            let idx = selected.split('.').next().unwrap().parse::<usize>()? - 1;
            let album = &albums[idx];
            if let Some(ref uri) = album.uri {
                client.add_to_queue(uri, None).await?;
                println!("✓ Queued album: {}", album.name.bold());
            }
            Ok(false)
        }
        "Next page" => {
            *offset += limit;
            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn handle_playlist_interaction(
    client: &mut SpotifyClient,
    playlists: &[Playlist],
    offset: &mut u32,
    limit: u32,
    total: u32,
) -> Result<bool> {
    let mut options = vec!["Play a playlist"];
    if *offset + limit < total {
        options.push("Next page");
    }
    options.push("Exit search");

    let choice = inquire::Select::new("Action:", options).prompt().context("Cancelled")?;

    match choice {
        "Play a playlist" => {
            let labels: Vec<String> = playlists.iter().enumerate().map(|(i, p)| format!("{}. {}", i + 1, p.name)).collect();
            let selected = inquire::Select::new("Select playlist to play:", labels).prompt()?;
            let idx = selected.split('.').next().unwrap().parse::<usize>()? - 1;
            let pl = &playlists[idx];
            client.play(Some(pl.uri.clone()), None, None, None).await?;
            println!("▶ Now playing playlist: {}", pl.name.bold().cyan());
            Ok(false)
        }
        "Next page" => {
            *offset += limit;
            Ok(true)
        }
        _ => Ok(false),
    }
}
