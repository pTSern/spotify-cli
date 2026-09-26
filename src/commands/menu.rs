use crate::api::SpotifyClient;
use crate::commands::{devices, library, playback, queue, search, test, volume};
use anyhow::Result;
use colored::Colorize;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, ContentArrangement, Table};

struct CommandEntry {
    category: &'static str,
    command: &'static str,
    aliases: &'static str,
    example: &'static str,
    description: &'static str,
}

const COMMANDS: &[CommandEntry] = &[
    // Playback
    CommandEntry {
        category: "Playback",
        command: "status",
        aliases: "st, info",
        example: "spotify-cli status",
        description: "Display currently playing song with live progress bar",
    },
    CommandEntry {
        category: "Playback",
        command: "play [query]",
        aliases: "p, resume",
        example: "spotify-cli play bohemian rhapsody",
        description: "Resume playback, search & play track, album, or playlist",
    },
    CommandEntry {
        category: "Playback",
        command: "pause",
        aliases: "-",
        example: "spotify-cli pause",
        description: "Pause playback on active device",
    },
    CommandEntry {
        category: "Playback",
        command: "toggle",
        aliases: "t",
        example: "spotify-cli toggle",
        description: "Toggle play/pause state",
    },
    CommandEntry {
        category: "Playback",
        command: "next",
        aliases: "n, skip",
        example: "spotify-cli next",
        description: "Skip to next track",
    },
    CommandEntry {
        category: "Playback",
        command: "previous",
        aliases: "prev, b",
        example: "spotify-cli prev",
        description: "Skip to previous track",
    },
    CommandEntry {
        category: "Playback",
        command: "seek <time>",
        aliases: "-",
        example: "spotify-cli seek 1:30 | +15 | -10",
        description: "Seek to timestamp (mm:ss) or relative seconds (+/-)",
    },
    CommandEntry {
        category: "Playback",
        command: "shuffle [on|off]",
        aliases: "sh",
        example: "spotify-cli shuffle on",
        description: "Toggle or set shuffle mode",
    },
    CommandEntry {
        category: "Playback",
        command: "repeat [mode]",
        aliases: "rep",
        example: "spotify-cli repeat track",
        description: "Cycle or set repeat mode (track, all, off)",
    },

    // Volume
    CommandEntry {
        category: "Volume",
        command: "volume",
        aliases: "vol, v",
        example: "spotify-cli vol",
        description: "Interactive volume selector with keyboard shortcuts",
    },
    CommandEntry {
        category: "Volume",
        command: "volume <amt>",
        aliases: "vol <amt>",
        example: "spotify-cli vol 80",
        description: "Set volume directly (0-100%)",
    },
    CommandEntry {
        category: "Volume",
        command: "volume up <amt>",
        aliases: "vol up",
        example: "spotify-cli vol up 10",
        description: "Increase volume by specified percentage",
    },
    CommandEntry {
        category: "Volume",
        command: "volume down <amt>",
        aliases: "vol down",
        example: "spotify-cli vol down 5",
        description: "Decrease volume by specified percentage",
    },
    CommandEntry {
        category: "Volume",
        command: "volume to <amt>",
        aliases: "vol to",
        example: "spotify-cli vol to 60",
        description: "Set volume to exact level (0-100%)",
    },

    // Devices
    CommandEntry {
        category: "Devices",
        command: "devices",
        aliases: "dev, d",
        example: "spotify-cli devices",
        description: "List active devices & prompt to switch",
    },
    CommandEntry {
        category: "Devices",
        command: "devices -s <name>",
        aliases: "dev --switch",
        example: "spotify-cli dev -s PC",
        description: "Switch playback to device matching name",
    },
    CommandEntry {
        category: "Devices",
        command: "devices -i",
        aliases: "dev --interactive",
        example: "spotify-cli dev -i",
        description: "Interactively choose device from menu",
    },

    // Search & Queue
    CommandEntry {
        category: "Search & Queue",
        command: "search <query>",
        aliases: "s, find",
        example: "spotify-cli search Queen",
        description: "Interactive search table with Play, Queue, Save actions",
    },
    CommandEntry {
        category: "Search & Queue",
        command: "queue",
        aliases: "q",
        example: "spotify-cli queue",
        description: "Display upcoming queue & prompt to play a track directly",
    },
    CommandEntry {
        category: "Search & Queue",
        command: "queue <query>",
        aliases: "q",
        example: "spotify-cli queue hotel california",
        description: "Search and add track or album to queue",
    },
    CommandEntry {
        category: "Search & Queue",
        command: "queue .",
        aliases: "q .",
        example: "spotify-cli queue .",
        description: "Add currently playing track to queue again",
    },

    // Library
    CommandEntry {
        category: "Library",
        command: "save",
        aliases: "-",
        example: "spotify-cli save",
        description: "Save currently playing track to Liked Songs",
    },
    CommandEntry {
        category: "Library",
        command: "save --album",
        aliases: "-",
        example: "spotify-cli save --album",
        description: "Save currently playing album to your library",
    },
    CommandEntry {
        category: "Library",
        command: "history",
        aliases: "hist, recent",
        example: "spotify-cli history -l 20",
        description: "List your recently played tracks",
    },
    CommandEntry {
        category: "Library",
        command: "top",
        aliases: "-",
        example: "spotify-cli top [--artists]",
        description: "List your top tracks or top artists",
    },

    // Diagnostics & Tests
    CommandEntry {
        category: "Diagnostics",
        command: "test",
        aliases: "tst, check",
        example: "spotify-cli test",
        description: "Interactive system test runner with logging to logs/",
    },
    CommandEntry {
        category: "Diagnostics",
        command: "test <filter>",
        aliases: "test all",
        example: "spotify-cli test [playback|volume|devices|search|library|all]",
        description: "Direct CLI test runner for specific subsystem",
    },

    // Auth & Setup
    CommandEntry {
        category: "Auth",
        command: "auth setup",
        aliases: "auth init",
        example: "spotify-cli auth setup",
        description: "Configure Spotify Developer App Client ID",
    },
    CommandEntry {
        category: "Auth",
        command: "auth login",
        aliases: "-",
        example: "spotify-cli auth login",
        description: "Authorize via browser using PKCE flow",
    },
    CommandEntry {
        category: "Auth",
        command: "auth status",
        aliases: "-",
        example: "spotify-cli auth status",
        description: "Show currently logged-in account details",
    },
    CommandEntry {
        category: "Auth",
        command: "auth logout",
        aliases: "-",
        example: "spotify-cli auth logout",
        description: "Log out and delete local cached tokens",
    },
];

pub async fn run_menu(
    client: Option<&mut SpotifyClient>,
    category_filter: Option<String>,
) -> Result<()> {
    if let Some(ref cat) = category_filter {
        print_tables(cat);
        return Ok(());
    }

    println!("\n{}", "================================================================================".green());
    println!("                         {} {}", "SPOTIFY CLI".green().bold(), "- INTERACTIVE COMMAND HUB".white().bold());
    println!("  Choose an action to execute immediately, or view cheatsheet tables.");
    println!("{}\n", "================================================================================".green());

    let options = vec![
        "▶  Status (View current playback with progress bar)",
        "⏯  Toggle (Play / Pause playback)",
        "⏭  Next Track",
        "⏮  Previous Track",
        "🔊 Volume Controller (Interactive TUI with shortcuts)",
        "📱 Devices Manager (List & Switch device)",
        "📜 Playback Queue & Direct Play",
        "🔍 Search Spotify (Interactive results with Play/Queue)",
        "💾 Save Currently Playing Track",
        "🕒 Recently Played History",
        "⭐ Top Tracks",
        "🧪 Run Diagnostics & Automated Tests",
        "📖 View Full Command Cheatsheet",
        "🚪 Exit Menu",
    ];

    let choice = match inquire::Select::new("Select action:", options).prompt() {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    if choice.starts_with("📖") {
        print_tables("all");
        return Ok(());
    } else if choice.starts_with("🚪") {
        return Ok(());
    }

    let client = match client {
        Some(c) => c,
        None => {
            println!("{}", "You must authenticate before running actions. Run `spotify-cli auth login`.".yellow());
            return Ok(());
        }
    };

    if choice.starts_with("▶") {
        playback::run_status(client, false).await?;
    } else if choice.starts_with("⏯") {
        playback::run_toggle(client).await?;
    } else if choice.starts_with("⏭") {
        playback::run_next(client).await?;
    } else if choice.starts_with("⏮") {
        playback::run_previous(client).await?;
    } else if choice.starts_with("🔊") {
        volume::run_volume(client, None, None).await?;
    } else if choice.starts_with("📱") {
        devices::run_devices(client, None, false).await?;
    } else if choice.starts_with("📜") {
        queue::run_queue(client, None, "track").await?;
    } else if choice.starts_with("🔍") {
        let q = inquire::Text::new("Enter search query:").prompt()?;
        if !q.trim().is_empty() {
            search::run_search(client, q.trim(), "track", 10).await?;
        }
    } else if choice.starts_with("💾") {
        library::run_save(client, "track").await?;
    } else if choice.starts_with("🕒") {
        library::run_history(client, 15).await?;
    } else if choice.starts_with("⭐") {
        library::run_top(client, "tracks", 10).await?;
    } else if choice.starts_with("🧪") {
        test::run_test(client, None).await?;
    }

    Ok(())
}

fn print_tables(category_filter: &str) {
    let categories = [
        "Playback",
        "Volume",
        "Devices",
        "Search & Queue",
        "Library",
        "Diagnostics",
        "Auth",
    ];

    let filter_lower = category_filter.to_lowercase();
    for cat in categories {
        if filter_lower != "all" && !cat.to_lowercase().contains(&filter_lower) {
            continue;
        }

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("Command").fg(Color::Cyan),
                Cell::new("Aliases").fg(Color::DarkGrey),
                Cell::new("Example").fg(Color::Yellow),
                Cell::new("Description").fg(Color::White),
            ]);

        for entry in COMMANDS.iter().filter(|c| c.category == cat) {
            table.add_row(vec![
                Cell::new(entry.command).fg(Color::Cyan),
                Cell::new(entry.aliases).fg(Color::DarkGrey),
                Cell::new(entry.example).fg(Color::Yellow),
                Cell::new(entry.description),
            ]);
        }

        println!("  {}", cat.bold().white());
        println!("{table}\n");
    }
}
