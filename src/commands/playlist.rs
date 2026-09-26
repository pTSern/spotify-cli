use crate::api::SpotifyClient;
use crate::models::{Playlist, Track};
use crate::ui::format_duration;
use anyhow::{bail, Context, Result};
use colored::Colorize;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, Row, Table};

#[derive(Debug, Clone)]
pub enum PlaylistSubcommand {
    List { limit: u32 },
    Play { query: Option<String>, shuffle: bool },
    Tracks { playlist: Option<String> },
    Create {
        name: Option<String>,
        description: Option<String>,
        public: bool,
        collaborative: bool,
    },
    Add {
        playlist: Option<String>,
        song: Option<String>,
        current: bool,
    },
    Mix {
        playlist: Option<String>,
        state: Option<String>,
    },
    Edit {
        playlist: Option<String>,
        name: Option<String>,
        description: Option<String>,
        public: Option<bool>,
        collaborative: Option<bool>,
    },
}

pub async fn run_playlist(
    client: &mut SpotifyClient,
    subcommand: Option<PlaylistSubcommand>,
) -> Result<()> {
    match subcommand {
        Some(PlaylistSubcommand::List { limit }) => run_list_playlists(client, limit).await,
        Some(PlaylistSubcommand::Play { query, shuffle }) => {
            run_play_playlist(client, query.as_deref(), shuffle).await
        }
        Some(PlaylistSubcommand::Tracks { playlist }) => {
            run_view_tracks(client, playlist.as_deref()).await
        }
        Some(PlaylistSubcommand::Create {
            name,
            description,
            public,
            collaborative,
        }) => {
            run_create_playlist(
                client,
                name.as_deref(),
                description.as_deref(),
                public,
                collaborative,
            )
            .await
        }
        Some(PlaylistSubcommand::Add {
            playlist,
            song,
            current,
        }) => run_add_song(client, playlist.as_deref(), song.as_deref(), current).await,
        Some(PlaylistSubcommand::Mix { playlist, state }) => {
            run_toggle_mix(client, playlist.as_deref(), state.as_deref()).await
        }
        Some(PlaylistSubcommand::Edit {
            playlist,
            name,
            description,
            public,
            collaborative,
        }) => {
            run_edit_playlist(
                client,
                playlist.as_deref(),
                name.as_deref(),
                description.as_deref(),
                public,
                collaborative,
            )
            .await
        }
        None => run_interactive_playlist_hub(client).await,
    }
}

async fn run_interactive_playlist_hub(client: &mut SpotifyClient) -> Result<()> {
    loop {
        println!("\n{}", "── Spotify Playlists Hub ──".bold().green());
        let options = vec![
            "1. Select & Play a Playlist",
            "2. List All My Playlists",
            "3. View Tracks in a Playlist",
            "4. Create a New Playlist",
            "5. Add Song to a Playlist (Current or Search)",
            "6. Toggle Mix / Collaborative on a Playlist",
            "7. Edit Playlist (Name, Description, Privacy)",
            "8. Exit Playlist Hub",
        ];

        let choice = inquire::Select::new("Choose an action:", options).prompt()?;

        if choice.starts_with("1.") {
            run_play_playlist(client, None, false).await?;
        } else if choice.starts_with("2.") {
            run_list_playlists(client, 50).await?;
        } else if choice.starts_with("3.") {
            run_view_tracks(client, None).await?;
        } else if choice.starts_with("4.") {
            run_create_playlist(client, None, None, false, false).await?;
        } else if choice.starts_with("5.") {
            run_add_song(client, None, None, false).await?;
        } else if choice.starts_with("6.") {
            run_toggle_mix(client, None, None).await?;
        } else if choice.starts_with("7.") {
            run_edit_playlist(client, None, None, None, None, None).await?;
        } else {
            break;
        }
    }
    Ok(())
}

pub async fn run_list_playlists(client: &mut SpotifyClient, limit: u32) -> Result<()> {
    println!("{}", "Fetching your playlists...".bright_black());
    let page = client.get_user_playlists(limit, 0).await?;

    if page.items.is_empty() {
        println!("{}", "No playlists found in your account.".yellow());
        return Ok(());
    }

    display_playlists_table(&page.items);
    println!(
        "\n{}",
        format!("Total playlists: {}", page.total).bright_black()
    );
    Ok(())
}

fn display_playlists_table(playlists: &[Playlist]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    table.set_header(vec![
        Cell::new("#").fg(Color::Cyan),
        Cell::new("Name").fg(Color::Green),
        Cell::new("Tracks").fg(Color::Yellow),
        Cell::new("Owner").fg(Color::White),
        Cell::new("Privacy").fg(Color::Magenta),
        Cell::new("Mix / Collab").fg(Color::Blue),
        Cell::new("Spotify ID").fg(Color::DarkGrey),
    ]);

    for (idx, pl) in playlists.iter().enumerate() {
        let tracks_count = pl
            .tracks
            .as_ref()
            .and_then(|t| t.total)
            .map(|t| t.to_string())
            .unwrap_or_else(|| "0".to_string());

        let owner_name = pl
            .owner
            .as_ref()
            .and_then(|o| o.display_name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let privacy = if pl.public.unwrap_or(false) {
            "Public".green()
        } else {
            "Private".bright_black()
        };

        let collab = if pl.collaborative.unwrap_or(false) {
            "Mix (Collab)".cyan().bold()
        } else {
            "Solo".bright_black()
        };

        table.add_row(Row::from(vec![
            Cell::new(idx + 1),
            Cell::new(&pl.name),
            Cell::new(tracks_count),
            Cell::new(owner_name),
            Cell::new(privacy),
            Cell::new(collab),
            Cell::new(&pl.id),
        ]));
    }

    println!("{table}");
}

async fn select_user_playlist(
    client: &mut SpotifyClient,
    query: Option<&str>,
    prompt_message: &str,
) -> Result<Playlist> {
    let page = client.get_user_playlists(50, 0).await?;
    if page.items.is_empty() {
        bail!("No playlists found in your account. Create one with `spotify-cli playlist create`.");
    }

    if let Some(q) = query {
        let q_trimmed = q.trim();
        // Check 1-based index
        if let Ok(idx) = q_trimmed.parse::<usize>() {
            if idx >= 1 && idx <= page.items.len() {
                return Ok(page.items[idx - 1].clone());
            }
        }

        // Check exact ID match
        if let Some(p) = page.items.iter().find(|p| p.id == q_trimmed || p.uri == q_trimmed) {
            return Ok(p.clone());
        }

        // Check case-insensitive name match
        let q_lower = q_trimmed.to_lowercase();
        if let Some(p) = page.items.iter().find(|p| p.name.to_lowercase() == q_lower) {
            return Ok(p.clone());
        }
        if let Some(p) = page.items.iter().find(|p| p.name.to_lowercase().contains(&q_lower)) {
            return Ok(p.clone());
        }

        println!("{}", format!("No playlist matched '{}'. Please select from list:", q_trimmed).yellow());
    }

    let choices: Vec<String> = page
        .items
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let count = p.tracks.as_ref().and_then(|t| t.total).unwrap_or(0);
            let collab_badge = if p.collaborative.unwrap_or(false) { " [Mix]" } else { "" };
            format!("{}. {} ({} tracks){}", i + 1, p.name, count, collab_badge)
        })
        .collect();

    let selected = inquire::Select::new(prompt_message, choices).prompt()?;
    let idx = selected.split('.').next().unwrap().parse::<usize>()? - 1;
    Ok(page.items[idx].clone())
}

pub async fn run_play_playlist(
    client: &mut SpotifyClient,
    query: Option<&str>,
    mut shuffle: bool,
) -> Result<()> {
    let playlist = select_user_playlist(client, query, "Select playlist to play:").await?;

    if !shuffle && query.is_none() {
        let play_mode = inquire::Select::new(
            "How would you like to play this playlist?",
            vec!["1. Normal order", "2. Shuffle / Mix mode"],
        )
        .prompt()?;
        if play_mode.starts_with("2.") {
            shuffle = true;
        }
    }

    if shuffle {
        let _ = client.set_shuffle(true, None).await;
        println!("{}", "🔀 Shuffle / Mix mode enabled.".cyan());
    }

    client.play(Some(playlist.uri.clone()), None, None, None).await?;
    println!(
        "▶ Now playing playlist: {} ({})",
        playlist.name.bold().green(),
        playlist.uri.bright_black()
    );

    Ok(())
}

pub async fn run_view_tracks(client: &mut SpotifyClient, query: Option<&str>) -> Result<()> {
    let playlist = select_user_playlist(client, query, "Select playlist to inspect:").await?;

    println!("{}", format!("Fetching tracks for '{}'...", playlist.name).bright_black());
    let resp = client.get_playlist_tracks(&playlist.id, 50, 0).await?;

    if resp.items.is_empty() {
        println!("{}", "This playlist has no tracks yet.".yellow());
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    table.set_header(vec![
        Cell::new("#").fg(Color::Cyan),
        Cell::new("Title").fg(Color::Green),
        Cell::new("Artist").fg(Color::Yellow),
        Cell::new("Album").fg(Color::White),
        Cell::new("Duration").fg(Color::DarkGrey),
    ]);

    let mut valid_tracks: Vec<Track> = Vec::new();

    for item in resp.items {
        if let Some(t) = item.track {
            let album_name = t.album.as_ref().map(|a| a.name.clone()).unwrap_or_default();
            let dur = format_duration(t.duration_ms);

            table.add_row(Row::from(vec![
                Cell::new(valid_tracks.len() + 1),
                Cell::new(&t.name),
                Cell::new(t.artists_str()),
                Cell::new(album_name),
                Cell::new(dur),
            ]));
            valid_tracks.push(t);
        }
    }

    println!("\n{}", format!("Playlist: {}", playlist.name).bold().green());
    println!("{table}");
    println!("{}", format!("Showing {} of {} tracks.", valid_tracks.len(), resp.total).bright_black());

    let actions = vec![
        "1. Play entire playlist",
        "2. Select a song from this playlist to play",
        "3. Back",
    ];

    let choice = inquire::Select::new("Playlist action:", actions).prompt()?;
    if choice.starts_with("1.") {
        client.play(Some(playlist.uri.clone()), None, None, None).await?;
        println!("▶ Now playing playlist: {}", playlist.name.bold().green());
    } else if choice.starts_with("2.") {
        let track_choices: Vec<String> = valid_tracks
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{}. {} - {}", i + 1, t.name, t.artists_str()))
            .collect();
        let picked = inquire::Select::new("Select track to play:", track_choices).prompt()?;
        let idx = picked.split('.').next().unwrap().parse::<usize>()? - 1;
        let t = &valid_tracks[idx];
        if let Some(ref uri) = t.uri {
            client.play(Some(playlist.uri.clone()), None, Some(uri.clone()), None).await?;
            println!("▶ Now playing: {} ({})", t.name.bold().green(), t.artists_str().cyan());
        }
    }

    Ok(())
}

pub async fn run_create_playlist(
    client: &mut SpotifyClient,
    name: Option<&str>,
    description: Option<&str>,
    mut public: bool,
    mut collaborative: bool,
) -> Result<()> {
    let final_name = match name {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => inquire::Text::new("Enter playlist name:").prompt()?,
    };

    if final_name.trim().is_empty() {
        bail!("Playlist name cannot be empty.");
    }

    let final_desc = match description {
        Some(d) => Some(d.to_string()),
        None => {
            let desc_input = inquire::Text::new("Enter description (optional, press Enter to skip):").prompt()?;
            if desc_input.trim().is_empty() {
                None
            } else {
                Some(desc_input.trim().to_string())
            }
        }
    };

    if name.is_none() {
        collaborative = inquire::Confirm::new("Enable collaborative mix mode? (allow others to add songs)")
            .with_default(false)
            .prompt()?;

        if collaborative {
            public = false; // Spotify requires collaborative playlists to be non-public
            println!("{}", "Note: Collaborative playlists are private per Spotify API rules.".bright_black());
        } else {
            public = inquire::Confirm::new("Make playlist public on your profile?")
                .with_default(false)
                .prompt()?;
        }
    }

    println!("{}", "Fetching user profile...".bright_black());
    let user = client.get_current_user().await?;

    println!("{}", format!("Creating playlist '{}'...", final_name).bright_black());
    let created = client
        .create_playlist(
            &user.id,
            &final_name,
            final_desc.as_deref(),
            public,
            collaborative,
        )
        .await?;

    println!(
        "\n{}",
        format!("✓ Playlist '{}' created successfully!", created.name).bold().green()
    );
    println!("  ID:  {}", created.id.cyan());
    println!("  URI: {}", created.uri.bright_black());
    println!(
        "  Mode: {} | {}",
        if public { "Public".green() } else { "Private".bright_black() },
        if collaborative { "Mix / Collab (Enabled)".cyan() } else { "Solo".bright_black() }
    );

    Ok(())
}

pub async fn run_add_song(
    client: &mut SpotifyClient,
    playlist_query: Option<&str>,
    song_query: Option<&str>,
    current: bool,
) -> Result<()> {
    let target_track: Track = if current {
        let state = client.get_playback_state().await?.context("No active playback session found.")?;
        state.item.context("No track is currently playing.")?
    } else if let Some(q) = song_query {
        let q_trimmed = q.trim();
        if q_trimmed.starts_with("spotify:track:") {
            Track {
                id: None,
                name: q_trimmed.to_string(),
                uri: Some(q_trimmed.to_string()),
                duration_ms: 0,
                track_number: None,
                artists: Vec::new(),
                album: None,
                external_urls: None,
            }
        } else {
            println!("{}", format!("Searching for '{}'...", q_trimmed).bright_black());
            let res = client.search(q_trimmed, "track", 5, 0).await?;
            let items = res.tracks.map(|t| t.items).unwrap_or_default();
            if items.is_empty() {
                bail!("No tracks found for '{}'", q_trimmed);
            }
            if items.len() == 1 {
                items[0].clone()
            } else {
                let choices: Vec<String> = items
                    .iter()
                    .enumerate()
                    .map(|(i, t)| format!("{}. {} - {} ({})", i + 1, t.name, t.artists_str(), format_duration(t.duration_ms)))
                    .collect();
                let picked = inquire::Select::new("Select song to add:", choices).prompt()?;
                let idx = picked.split('.').next().unwrap().parse::<usize>()? - 1;
                items[idx].clone()
            }
        }
    } else {
        let source_choice = inquire::Select::new(
            "Select track source:",
            vec!["1. Currently playing track", "2. Search for a song"],
        )
        .prompt()?;

        if source_choice.starts_with("1.") {
            let state = client.get_playback_state().await?.context("No active playback session found.")?;
            state.item.context("No track is currently playing.")?
        } else {
            let search_term = inquire::Text::new("Enter song title or artist to search:").prompt()?;
            println!("{}", format!("Searching for '{}'...", search_term).bright_black());
            let res = client.search(&search_term, "track", 5, 0).await?;
            let items = res.tracks.map(|t| t.items).unwrap_or_default();
            if items.is_empty() {
                bail!("No tracks found for '{}'", search_term);
            }
            let choices: Vec<String> = items
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {} - {} ({})", i + 1, t.name, t.artists_str(), format_duration(t.duration_ms)))
                .collect();
            let picked = inquire::Select::new("Select song to add:", choices).prompt()?;
            let idx = picked.split('.').next().unwrap().parse::<usize>()? - 1;
            items[idx].clone()
        }
    };

    let track_uri = target_track.uri.clone().context("Track has no Spotify URI.")?;

    let playlist = select_user_playlist(client, playlist_query, "Select destination playlist:").await?;

    println!(
        "{}",
        format!("Adding '{}' to playlist '{}'...", target_track.name, playlist.name).bright_black()
    );
    client.add_tracks_to_playlist(&playlist.id, &[&track_uri]).await?;

    println!(
        "\n{}",
        format!("✓ Added '{}' by {} to playlist '{}'!", target_track.name.bold(), target_track.artists_str().cyan(), playlist.name.green()).green()
    );

    Ok(())
}

pub async fn run_toggle_mix(
    client: &mut SpotifyClient,
    playlist_query: Option<&str>,
    state_arg: Option<&str>,
) -> Result<()> {
    let playlist = select_user_playlist(client, playlist_query, "Select playlist to configure mix/collaborative:").await?;

    let current_collab = playlist.collaborative.unwrap_or(false);
    let new_collab = match state_arg {
        Some("on") | Some("enable") | Some("true") => true,
        Some("off") | Some("disable") | Some("false") => false,
        _ => {
            let prompt = format!(
                "Current mix/collaborative status is [{}]. Enable mix (collaborative) mode?",
                if current_collab { "ENABLED" } else { "DISABLED" }
            );
            inquire::Confirm::new(&prompt).with_default(!current_collab).prompt()?
        }
    };

    // If collaborative is true, public must be false in Spotify API
    let new_public = if new_collab {
        Some(false)
    } else {
        playlist.public
    };

    println!("{}", format!("Updating mix settings for '{}'...", playlist.name).bright_black());
    client
        .change_playlist_details(
            &playlist.id,
            None,
            None,
            new_public,
            Some(new_collab),
        )
        .await?;

    let status_str = if new_collab {
        "ENABLED (others can now add and mix tracks)".cyan().bold()
    } else {
        "DISABLED (solo mode)".bright_black()
    };

    println!(
        "\n{}",
        format!("✓ Playlist '{}' mix / collaborative mode is now: {}", playlist.name.green(), status_str).green()
    );

    Ok(())
}

pub async fn run_edit_playlist(
    client: &mut SpotifyClient,
    playlist_query: Option<&str>,
    name: Option<&str>,
    description: Option<&str>,
    public: Option<bool>,
    collaborative: Option<bool>,
) -> Result<()> {
    let playlist = select_user_playlist(client, playlist_query, "Select playlist to edit:").await?;

    let is_interactive = name.is_none() && description.is_none() && public.is_none() && collaborative.is_none();

    let (new_name, new_desc, new_public, new_collab) = if is_interactive {
        println!("\n{}", format!("Editing playlist: {}", playlist.name).bold().green());
        println!("Current description: {}", playlist.description.as_deref().unwrap_or("(none)"));
        println!("Current privacy:     {}", if playlist.public.unwrap_or(false) { "Public" } else { "Private" });
        println!("Current mix/collab:  {}", if playlist.collaborative.unwrap_or(false) { "Enabled" } else { "Disabled" });
        println!();

        let edit_name = inquire::Text::new("New playlist name (press Enter to keep current):")
            .with_default(&playlist.name)
            .prompt()?;

        let edit_desc = inquire::Text::new("New description (press Enter to keep current):")
            .with_default(playlist.description.as_deref().unwrap_or(""))
            .prompt()?;

        let edit_collab = inquire::Confirm::new("Enable collaborative mix mode?")
            .with_default(playlist.collaborative.unwrap_or(false))
            .prompt()?;

        let edit_public = if edit_collab {
            false
        } else {
            inquire::Confirm::new("Make playlist public on profile?")
                .with_default(playlist.public.unwrap_or(false))
                .prompt()?
        };

        (
            Some(edit_name),
            if edit_desc.trim().is_empty() { None } else { Some(edit_desc) },
            Some(edit_public),
            Some(edit_collab),
        )
    } else {
        (
            name.map(|s| s.to_string()),
            description.map(|s| s.to_string()),
            public,
            collaborative,
        )
    };

    println!("{}", format!("Updating playlist '{}'...", playlist.name).bright_black());
    client
        .change_playlist_details(
            &playlist.id,
            new_name.as_deref(),
            new_desc.as_deref(),
            new_public,
            new_collab,
        )
        .await?;

    println!(
        "\n{}",
        format!("✓ Playlist '{}' updated successfully!", new_name.as_deref().unwrap_or(&playlist.name)).bold().green()
    );

    Ok(())
}
