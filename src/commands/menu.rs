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
        command: "volume <amt>",
        aliases: "vol, v",
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
        description: "List all active/connected Spotify devices",
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
        description: "Display currently playing and upcoming queue tracks",
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

pub fn run_menu() {
    println!("\n{}", "================================================================================".green());
    println!("                         {} {}", "SPOTIFY CLI".green().bold(), "- COMMAND MENU".white().bold());
    println!("  Run any command using: {} or {}", "spotify-cli <command>".cyan(), "spotify <command>".cyan());
    println!("{}\n", "================================================================================".green());

    let categories = [
        "Playback",
        "Volume",
        "Devices",
        "Search & Queue",
        "Library",
        "Auth",
    ];

    for cat in categories {
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
