use crate::api::SpotifyClient;
use crate::config::KeyBinding;
use anyhow::{bail, Result};
use colored::Colorize;
use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use std::io::{stdout, Write};

pub async fn run_volume(
    client: &mut SpotifyClient,
    action_or_amount: Option<String>,
    maybe_amount: Option<u32>,
) -> Result<()> {
    match action_or_amount {
        Some(action_str) => {
            // Direct CLI mode
            let (target_vol, mode_desc) = match action_str.as_str() {
                "up" => {
                    let delta = maybe_amount.unwrap_or(10);
                    let state = client.get_playback_state().await?;
                    let current = state
                        .and_then(|s| s.device)
                        .and_then(|d| d.volume_percent)
                        .unwrap_or(50);
                    ((current + delta).min(100), format!("increased by {}%", delta))
                }
                "down" => {
                    let delta = maybe_amount.unwrap_or(10);
                    let state = client.get_playback_state().await?;
                    let current = state
                        .and_then(|s| s.device)
                        .and_then(|d| d.volume_percent)
                        .unwrap_or(50);
                    (current.saturating_sub(delta), format!("decreased by {}%", delta))
                }
                "to" | "set" => {
                    let val = match maybe_amount {
                        Some(v) => v.min(100),
                        None => bail!("Please specify a volume percentage (0-100), e.g. `spotify vol to 80`"),
                    };
                    (val, format!("set to {}%", val))
                }
                num_str => {
                    if let Ok(val) = num_str.parse::<u32>() {
                        (val.min(100), format!("set to {}%", val.min(100)))
                    } else {
                        bail!("Invalid volume command. Usage: `spotify vol [up|down|to] <amount>` or `spotify vol <amount>`");
                    }
                }
            };

            apply_volume(client, target_vol, &mode_desc).await
        }
        None => {
            // Interactive Player-style TUI mode
            run_interactive_volume(client).await
        }
    }
}

async fn apply_volume(client: &mut SpotifyClient, volume: u32, desc: &str) -> Result<()> {
    client.set_volume(volume, None).await?;
    println!("🔊 Volume {} (now at {}%)", desc.cyan(), volume.to_string().bold().green());
    Ok(())
}

async fn run_interactive_volume(client: &mut SpotifyClient) -> Result<()> {
    let state = client.get_playback_state().await?;
    let mut current_vol = state
        .as_ref()
        .and_then(|s| s.device.as_ref())
        .and_then(|d| d.volume_percent)
        .unwrap_or(50);

    let device_name = state
        .as_ref()
        .and_then(|s| s.device.as_ref())
        .map(|d| d.name.as_str())
        .unwrap_or("Active Device");

    println!("\n{}", "── Spotify Interactive Volume Controller ──".green().bold());
    println!("Device: {}\n", device_name.cyan());

    let mut first_render = true;

    loop {
        let step = client.config.volume_settings.step;
        let inc_keys = client.config.volume_settings.keys_for_action("increase");
        let dec_keys = client.config.volume_settings.keys_for_action("decrease");
        let set_keys = client.config.volume_settings.keys_for_action("settings");
        let exit_keys = client.config.volume_settings.keys_for_action("exit");

        // Render bar
        render_volume_bar(current_vol, step, &inc_keys, &dec_keys, &set_keys, &exit_keys, first_render);
        first_render = false;

        // Raw mode keyboard input
        enable_raw_mode()?;
        let key_code = loop {
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key_event) = event::read()? {
                    break Some(key_event.code);
                }
            }
        };
        disable_raw_mode()?;

        let key_str = match key_code {
            Some(KeyCode::Left) => "Left".to_string(),
            Some(KeyCode::Right) => "Right".to_string(),
            Some(KeyCode::Up) => "Up".to_string(),
            Some(KeyCode::Down) => "Down".to_string(),
            Some(KeyCode::Esc) => "Esc".to_string(),
            Some(KeyCode::Char(c)) => c.to_string(),
            _ => continue,
        };

        let action = client
            .config
            .volume_settings
            .find_action(&key_str)
            .map(|s| s.to_string());

        match action.as_deref() {
            Some("increase") => {
                current_vol = (current_vol + step).min(100);
                let _ = client.set_volume(current_vol, None).await;
            }
            Some("decrease") => {
                current_vol = current_vol.saturating_sub(step);
                let _ = client.set_volume(current_vol, None).await;
            }
            Some("settings") => {
                println!();
                manage_volume_settings(client).await?;
                first_render = true;
            }
            Some("exit") => {
                println!("\n\n🔊 Exited volume controller. Final volume: {}%", current_vol.to_string().green().bold());
                break;
            }
            _ => {
                // Unbound key, ignore
            }
        }
    }

    Ok(())
}

fn render_volume_bar(
    volume: u32,
    step: u32,
    inc_keys: &str,
    dec_keys: &str,
    set_keys: &str,
    exit_keys: &str,
    first_render: bool,
) {
    let bar_width: usize = 25;
    let filled_len = ((bar_width as f32) * (volume as f32 / 100.0)).round() as usize;
    let unfilled_len = bar_width.saturating_sub(filled_len);

    let filled = "━".repeat(filled_len).green().bold();
    let unfilled = "─".repeat(unfilled_len).bright_black();

    let mut out = stdout();

    if !first_render {
        // Move up 2 lines and clear
        let _ = crossterm::execute!(out, MoveUp(2), MoveToColumn(0), Clear(ClearType::FromCursorDown));
    }

    println!("  Volume [{}{}] {}%", filled, unfilled, volume.to_string().cyan().bold());
    println!(
        "  {} reduce by {}%   {} increase by {}%   {} settings   {} exit",
        format!("[{}]", dec_keys).bright_black(),
        step,
        format!("[{}]", inc_keys).bright_black(),
        step,
        format!("[{}]", set_keys).bright_black(),
        format!("[{}]", exit_keys).bright_black()
    );
    let _ = out.flush();
}

async fn manage_volume_settings(client: &mut SpotifyClient) -> Result<()> {
    loop {
        println!("\n{}", "── Volume Controller Settings ──".bold().yellow());
        let options = vec![
            format!("1. Change volume step size (Current: {}%)", client.config.volume_settings.step),
            "2. View & Manage keybindings (Add / Delete)".to_string(),
            "3. Return to Volume controller".to_string(),
        ];

        let choice = inquire::Select::new("Select settings option:", options).prompt()?;

        if choice.starts_with("1.") {
            let new_step: u32 = inquire::CustomType::new("Enter new step percentage (1-50):")
                .with_default(client.config.volume_settings.step)
                .with_error_message("Please enter a number between 1 and 50")
                .prompt()?;
            client.config.volume_settings.step = new_step.clamp(1, 50);
            client.config.save()?;
            println!("{}", format!("✓ Step size updated to {}%", client.config.volume_settings.step).green());
        } else if choice.starts_with("2.") {
            manage_keybindings(client)?;
        } else {
            break;
        }
    }
    println!();
    Ok(())
}

fn manage_keybindings(client: &mut SpotifyClient) -> Result<()> {
    loop {
        println!("\n{}", "Current Volume Keybindings:".bold());
        for (i, b) in client.config.volume_settings.bindings.iter().enumerate() {
            println!("  {}. Key: {:<8} -> Action: {}", i + 1, format!("[{}]", b.key).cyan(), b.action.yellow());
        }

        let options = vec![
            "Add new keybinding",
            "Delete an existing keybinding",
            "Reset keybindings to default",
            "Back to settings",
        ];

        let action = inquire::Select::new("Keybinding action:", options).prompt()?;

        match action {
            "Add new keybinding" => {
                let action_choices = vec!["increase", "decrease", "settings", "exit"];
                let selected_action = inquire::Select::new("Bind to which action?", action_choices).prompt()?;

                let key_input: String = inquire::Text::new("Press or type key name (e.g. d, a, Right, Left, Up, Down, q, Esc):")
                    .with_placeholder("e.g. d")
                    .prompt()?;

                let trimmed_key = key_input.trim().to_string();
                if trimmed_key.is_empty() {
                    println!("{}", "Key cannot be empty.".red());
                    continue;
                }

                // Check if already bound
                if client.config.volume_settings.bindings.iter().any(|b| b.key.to_lowercase() == trimmed_key.to_lowercase()) {
                    println!("{}", format!("Key '{}' is already bound to an action.", trimmed_key).yellow());
                    continue;
                }

                client.config.volume_settings.bindings.push(KeyBinding {
                    action: selected_action.to_string(),
                    key: trimmed_key.clone(),
                });
                client.config.save()?;
                println!("{}", format!("✓ Added [{}] for action '{}'", trimmed_key, selected_action).green());
            }
            "Delete an existing keybinding" => {
                let choices: Vec<String> = client
                    .config
                    .volume_settings
                    .bindings
                    .iter()
                    .enumerate()
                    .map(|(i, b)| format!("{}. [{}] for {}", i + 1, b.key, b.action))
                    .collect();

                let to_delete = inquire::Select::new("Select keybinding to delete:", choices).prompt()?;
                let idx = to_delete.split('.').next().unwrap().parse::<usize>()? - 1;

                // Safety Rule: Do not allow deleting if it's the only key for that action!
                if !client.config.volume_settings.can_delete_binding(idx) {
                    let action = &client.config.volume_settings.bindings[idx].action;
                    println!(
                        "{}",
                        format!(
                            "❌ Cannot delete: At least one keybinding must remain for action '{}'!",
                            action
                        )
                        .red()
                        .bold()
                    );
                    continue;
                }

                let removed = client.config.volume_settings.bindings.remove(idx);
                client.config.save()?;
                println!("{}", format!("✓ Deleted [{}] for {}", removed.key, removed.action).green());
            }
            "Reset keybindings to default" => {
                client.config.volume_settings = crate::config::VolumeSettings::default();
                client.config.save()?;
                println!("{}", "✓ Keybindings reset to defaults!".green());
            }
            _ => break,
        }
    }
    Ok(())
}
