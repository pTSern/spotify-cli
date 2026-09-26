mod api;
mod auth;
mod commands;
mod config;
pub mod logger;
mod models;
mod ui;

use api::SpotifyClient;
use clap::{Args, Parser, Subcommand};
use colored::Colorize;
use config::Config;

#[derive(Parser, Debug)]
#[command(
    name = "spotify",
    author = "Spotify CLI Team",
    version,
    about = "Control Spotify playback directly from your command line.",
    long_about = "A lightning-fast, self-contained Spotify CLI built with Rust and OAuth2 PKCE."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage Spotify authentication (login, logout, status, setup)
    Auth {
        #[command(subcommand)]
        action: AuthCommands,
    },

    /// Show current playback status (interactive real-time player by default)
    #[command(alias = "st", alias = "info")]
    Status {
        /// Print status once without opening interactive player
        #[arg(short = 's', long = "static", alias = "once")]
        r#static: bool,
    },

    /// Resume playback or search & play a track/album/playlist
    #[command(alias = "p", alias = "resume")]
    Play(PlayArgs),

    /// Pause current playback
    Pause,

    /// Toggle play / pause state
    #[command(alias = "t")]
    Toggle,

    /// Skip to the next track
    #[command(alias = "n", alias = "skip")]
    Next,

    /// Skip to the previous track
    #[command(alias = "prev", alias = "b", alias = "back")]
    Previous,

    /// Seek to a specific timestamp (e.g. '1:30', '90', '+10', '-15')
    Seek {
        /// Target timestamp in mm:ss or seconds (can use + or - for relative seek)
        time: String,
    },

    /// Adjust playback volume (e.g. 'vol up 10', 'vol down 5', 'vol to 80', or 'vol 65')
    #[command(alias = "vol", alias = "v")]
    /// Adjust playback volume (e.g. 'vol', 'vol 80', 'vol up 10', 'vol down 5', 'vol to 60')
    #[command(alias = "vol", alias = "v")]
    Volume {
        /// Action ('up', 'down', 'to') or direct number (0-100). If omitted, interactive UI opens.
        action_or_amount: Option<String>,
        /// Amount (0-100) when using 'up', 'down', or 'to'
        amount: Option<u32>,
    },

    /// Toggle or set shuffle mode
    #[command(alias = "sh")]
    Shuffle {
        /// Shuffle state ('on' or 'off')
        mode: Option<String>,
    },

    /// Set repeat mode ('track', 'all', or 'off')
    #[command(alias = "rep")]
    Repeat {
        /// Repeat mode ('track', 'all', or 'off')
        mode: Option<String>,
    },

    /// List active devices or switch playback to a specific device
    #[command(alias = "dev", alias = "d")]
    Devices {
        /// Target device name to switch to
        #[arg(short = 's', long = "switch")]
        switch_to: Option<String>,

        /// Interactively pick a device from a menu
        #[arg(short = 'i', long = "interactive")]
        interactive: bool,
    },

    /// View playback queue or add tracks/albums to queue
    #[command(alias = "q")]
    Queue(QueueArgs),

    /// Search Spotify interactively with inline actions (play, queue, save)
    #[command(alias = "s", alias = "find")]
    Search(SearchArgs),

    /// Save currently playing track or album to your library
    Save {
        /// Save the current album instead of track
        #[arg(long = "album")]
        album: bool,
    },

    /// View your recently played tracks
    #[command(alias = "hist", alias = "recent")]
    History {
        /// Number of tracks to display (default: 15)
        #[arg(short = 'l', long = "limit", default_value_t = 15)]
        limit: u32,
    },

    /// View your top tracks or top artists
    Top {
        /// Show top artists instead of top tracks
        #[arg(long = "artists")]
        artists: bool,

        /// Number of items to display (default: 10)
        #[arg(short = 'l', long = "limit", default_value_t = 10)]
        limit: u32,
    },

    /// Manage, view, create, edit, and play playlists
    #[command(alias = "pl", alias = "playlists")]
    Playlist {
        #[command(subcommand)]
        action: Option<PlaylistCommands>,
    },

    /// Display CLI command menu and cheatsheet (interactive or by category)
    #[command(alias = "m", alias = "commands", alias = "help-menu")]
    Menu {
        /// Optional category to view directly
        category: Option<String>,
    },

    /// Run automated API diagnostic tests (interactive or with filter)
    #[command(alias = "tst", alias = "check")]
    Test {
        /// Filter: playback, volume, devices, search, library, or all
        filter: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum PlaylistCommands {
    /// List your playlists
    #[command(alias = "ls")]
    List {
        /// Number of playlists to fetch (default: 50)
        #[arg(short = 'l', long = "limit", default_value_t = 50)]
        limit: u32,
    },

    /// Play a playlist
    #[command(alias = "p")]
    Play {
        /// Playlist name, number, or URI/ID (interactive selector if omitted)
        query: Option<String>,
        /// Enable shuffle / mix mode when playing
        #[arg(short = 's', long = "shuffle", alias = "mix")]
        shuffle: bool,
    },

    /// View songs inside a playlist
    Tracks {
        /// Playlist name, number, or URI/ID (interactive selector if omitted)
        playlist: Option<String>,
    },

    /// Create a new playlist
    Create {
        /// Name of the new playlist
        name: Option<String>,
        /// Description of the playlist
        #[arg(short = 'd', long = "desc")]
        description: Option<String>,
        /// Make the playlist public (default: false)
        #[arg(long = "public")]
        public: bool,
        /// Make the playlist collaborative / mix (default: false)
        #[arg(long = "collaborative", alias = "mix")]
        collaborative: bool,
    },

    /// Add a track to a playlist
    Add {
        /// Target playlist name, number, or URI/ID
        playlist: Option<String>,
        /// Song name or URI (interactive search if omitted)
        song: Option<String>,
        /// Add currently playing track
        #[arg(short = 'c', long = "current")]
        current: bool,
    },

    /// Toggle or set collaborative / mix mode on a playlist
    Mix {
        /// Playlist name, number, or URI/ID
        playlist: Option<String>,
        /// Set mix mode on/off ('on', 'off', or toggle if omitted)
        state: Option<String>,
    },

    /// Edit playlist details (name, description, privacy, mix)
    Edit {
        /// Playlist name, number, or URI/ID
        playlist: Option<String>,
        /// New name for playlist
        #[arg(long = "name")]
        name: Option<String>,
        /// New description for playlist
        #[arg(short = 'd', long = "desc")]
        description: Option<String>,
        /// Set public visibility ('true' or 'false')
        #[arg(long = "public")]
        public: Option<bool>,
        /// Set collaborative / mix mode ('true' or 'false')
        #[arg(long = "collaborative", alias = "mix")]
        collaborative: Option<bool>,
    },
}

#[derive(Subcommand, Debug)]
enum AuthCommands {
    /// Authorize Spotify CLI via your browser
    Login,
    /// Log out and clear saved credentials
    Logout,
    /// Display currently logged in user info
    Status,
    /// Configure Spotify Developer App Client ID
    #[command(alias = "init")]
    Setup,
}

#[derive(Args, Debug)]
struct PlayArgs {
    /// Track name, artist, album, playlist, or Spotify URI
    query: Vec<String>,

    /// Search and play an album
    #[arg(long = "album")]
    album: bool,

    /// Search and play a playlist
    #[arg(long = "playlist")]
    playlist: bool,
}

#[derive(Args, Debug)]
struct QueueArgs {
    /// Song/album name or '.' for currently playing song
    query: Vec<String>,

    /// Queue an album instead of a track
    #[arg(long = "album")]
    album: bool,
}

#[derive(Args, Debug)]
struct SearchArgs {
    /// Search query
    query: Vec<String>,

    /// Search for an album
    #[arg(long = "album")]
    album: bool,

    /// Search for an artist
    #[arg(long = "artist")]
    artist: bool,

    /// Search for a playlist
    #[arg(long = "playlist")]
    playlist: bool,

    /// Number of items per page (default: 10)
    #[arg(short = 'l', long = "limit", default_value_t = 10)]
    limit: u32,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let mut config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("{}: {}", "Error loading configuration".red(), e);
            Config::default()
        }
    };

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            // Default with no args: show status if logged in, else show help
            if config.access_token.is_some() {
                Commands::Status { r#static: false }
            } else {
                println!("{}", "Welcome to Spotify CLI!".green().bold());
                println!("Run `spotify-cli auth login` to connect your Spotify account.");
                println!("Run `spotify-cli menu` to see all available commands.\n");
                return;
            }
        }
    };

    // Menu can be shown with or without login
    if let Commands::Menu { category } = command {
        let res = if config.access_token.is_some() {
            let mut client = SpotifyClient::new(config);
            commands::menu::run_menu(Some(&mut client), category).await
        } else {
            commands::menu::run_menu(None, category).await
        };
        if let Err(e) = res {
            eprintln!("{}: {}", "Menu Error".red(), e);
            logger::log_error("Menu Execution", &e.to_string(), None);
        }
        return;
    }

    // Dispatch Auth commands early (since they may modify config before client initialization)
    if let Commands::Auth { action } = command {
        let res = match action {
            AuthCommands::Login => commands::auth::run_login(&mut config).await,
            AuthCommands::Logout => commands::auth::run_logout(&mut config).await,
            AuthCommands::Setup => commands::auth::run_setup(&mut config).await,
            AuthCommands::Status => {
                let mut client = SpotifyClient::new(config);
                commands::auth::run_status(&mut client).await
            }
        };

        if let Err(e) = res {
            eprintln!("{}: {}", "Auth Error".red(), e);
            logger::log_error("Auth Command", &e.to_string(), None);
            std::process::exit(1);
        }
        return;
    }

    // Verify token exists for playback/library commands
    if config.access_token.is_none() {
        eprintln!("{}", "Not authenticated. Please run `spotify auth login` first.".yellow());
        std::process::exit(1);
    }

    let mut client = SpotifyClient::new(config);

    let res = match command {
        Commands::Auth { .. } => unreachable!(),
        Commands::Menu { .. } => unreachable!(),
        Commands::Test { filter } => commands::test::run_test(&mut client, filter).await,
        Commands::Playlist { action } => {
            let sub = action.map(|a| match a {
                PlaylistCommands::List { limit } => commands::playlist::PlaylistSubcommand::List { limit },
                PlaylistCommands::Play { query, shuffle } => commands::playlist::PlaylistSubcommand::Play { query, shuffle },
                PlaylistCommands::Tracks { playlist } => commands::playlist::PlaylistSubcommand::Tracks { playlist },
                PlaylistCommands::Create { name, description, public, collaborative } => {
                    commands::playlist::PlaylistSubcommand::Create { name, description, public, collaborative }
                }
                PlaylistCommands::Add { playlist, song, current } => {
                    commands::playlist::PlaylistSubcommand::Add { playlist, song, current }
                }
                PlaylistCommands::Mix { playlist, state } => commands::playlist::PlaylistSubcommand::Mix { playlist, state },
                PlaylistCommands::Edit { playlist, name, description, public, collaborative } => {
                    commands::playlist::PlaylistSubcommand::Edit { playlist, name, description, public, collaborative }
                }
            });
            commands::playlist::run_playlist(&mut client, sub).await
        }
        Commands::Status { r#static } => commands::playback::run_status(&mut client, r#static).await,
        Commands::Play(args) => {
            let q = if args.query.is_empty() {
                None
            } else {
                Some(args.query.join(" "))
            };
            let play_type = if args.album {
                "album"
            } else if args.playlist {
                "playlist"
            } else {
                "track"
            };
            commands::playback::run_play(&mut client, q, play_type).await
        }
        Commands::Pause => commands::playback::run_pause(&mut client).await,
        Commands::Toggle => commands::playback::run_toggle(&mut client).await,
        Commands::Next => commands::playback::run_next(&mut client).await,
        Commands::Previous => commands::playback::run_previous(&mut client).await,
        Commands::Seek { time } => commands::playback::run_seek(&mut client, &time).await,
        Commands::Volume {
            action_or_amount,
            amount,
        } => commands::volume::run_volume(&mut client, action_or_amount, amount).await,
        Commands::Shuffle { mode } => commands::playback::run_shuffle(&mut client, mode).await,
        Commands::Repeat { mode } => commands::playback::run_repeat(&mut client, mode).await,
        Commands::Devices {
            switch_to,
            interactive,
        } => commands::devices::run_devices(&mut client, switch_to, interactive).await,
        Commands::Queue(args) => {
            let q = if args.query.is_empty() {
                None
            } else {
                Some(args.query.join(" "))
            };
            let q_type = if args.album { "album" } else { "track" };
            commands::queue::run_queue(&mut client, q, q_type).await
        }
        Commands::Search(args) => {
            if args.query.is_empty() {
                eprintln!("{}", "Please provide a search keyword. e.g. `spotify search Queen`".yellow());
                return;
            }
            let q = args.query.join(" ");
            let s_type = if args.album {
                "album"
            } else if args.artist {
                "artist"
            } else if args.playlist {
                "playlist"
            } else {
                "track"
            };
            commands::search::run_search(&mut client, &q, s_type, args.limit).await
        }
        Commands::Save { album } => {
            let item_type = if album { "album" } else { "track" };
            commands::library::run_save(&mut client, item_type).await
        }
        Commands::History { limit } => commands::library::run_history(&mut client, limit).await,
        Commands::Top { artists, limit } => {
            let item_type = if artists { "artists" } else { "tracks" };
            commands::library::run_top(&mut client, item_type, limit).await
        }
    };

    if let Err(e) = res {
        eprintln!("{}: {}", "Error".red(), e);
        logger::log_error("CLI Command Execution", &e.to_string(), None);
        std::process::exit(1);
    }
}
