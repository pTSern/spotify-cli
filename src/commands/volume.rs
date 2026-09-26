use crate::api::SpotifyClient;
use anyhow::{bail, Result};
use colored::Colorize;

pub async fn run_volume(
    client: &mut SpotifyClient,
    action_or_amount: &str,
    maybe_amount: Option<u32>,
) -> Result<()> {
    let (target_vol, mode_desc) = match action_or_amount {
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

    client.set_volume(target_vol, None).await?;
    println!("🔊 Volume {} (now at {}%)", mode_desc.cyan(), target_vol.to_string().bold().green());
    Ok(())
}
