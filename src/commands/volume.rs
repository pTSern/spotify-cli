use crate::api::SpotifyClient;
use anyhow::{bail, Result};
use colored::Colorize;

pub async fn run_volume(
    client: &mut SpotifyClient,
    action_or_amount: Option<String>,
    maybe_amount: Option<u32>,
) -> Result<()> {
    let action_str = match action_or_amount {
        Some(s) => s,
        None => {
            // Interactive mode
            let state = client.get_playback_state().await?;
            let current = state
                .and_then(|s| s.device)
                .and_then(|d| d.volume_percent)
                .unwrap_or(50);

            println!("\n🔊 {}", format!("Current Volume: {}%", current).bold().cyan());

            let options = vec![
                format!("+ 10%  (Set to {}%)", (current + 10).min(100)),
                format!("+  5%  (Set to {}%)", (current + 5).min(100)),
                format!("-  5%  (Set to {}%)", current.saturating_sub(5)),
                format!("- 10%  (Set to {}%)", current.saturating_sub(10)),
                "Set custom percentage (0-100)...".to_string(),
                "Mute (0%)".to_string(),
                "Max (100%)".to_string(),
                "Exit without changing".to_string(),
            ];

            let choice = inquire::Select::new("Select volume adjustment:", options).prompt()?;

            if choice.starts_with("+ 10%") {
                "up".to_string()
            } else if choice.starts_with("+  5%") {
                return apply_volume(client, (current + 5).min(100), "increased by 5%").await;
            } else if choice.starts_with("-  5%") {
                return apply_volume(client, current.saturating_sub(5), "decreased by 5%").await;
            } else if choice.starts_with("- 10%") {
                return apply_volume(client, current.saturating_sub(10), "decreased by 10%").await;
            } else if choice.starts_with("Mute") {
                return apply_volume(client, 0, "muted (0%)").await;
            } else if choice.starts_with("Max") {
                return apply_volume(client, 100, "set to max (100%)").await;
            } else if choice.starts_with("Set custom") {
                let custom_val: u32 = inquire::CustomType::new("Enter desired volume level (0-100):")
                    .with_default(current)
                    .with_error_message("Please enter a valid number between 0 and 100")
                    .prompt()?;
                return apply_volume(client, custom_val.min(100), &format!("set to {}%", custom_val.min(100))).await;
            } else {
                return Ok(());
            }
        }
    };

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
            // Check if user passed a bare number e.g. `spotify vol 75`
            if let Ok(val) = num_str.parse::<u32>() {
                (val.min(100), format!("set to {}%", val.min(100)))
            } else {
                bail!("Invalid volume command. Usage: `spotify vol [up|down|to] <amount>` or `spotify vol <amount>`");
            }
        }
    };

    apply_volume(client, target_vol, &mode_desc).await
}

async fn apply_volume(client: &mut SpotifyClient, volume: u32, desc: &str) -> Result<()> {
    client.set_volume(volume, None).await?;
    println!("🔊 Volume {} (now at {}%)", desc.cyan(), volume.to_string().bold().green());
    Ok(())
}
