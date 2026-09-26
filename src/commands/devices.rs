use crate::api::SpotifyClient;
use crate::models::Device;
use crate::ui::print_device_list;
use anyhow::{bail, Context, Result};
use colored::Colorize;

pub async fn run_devices(
    client: &mut SpotifyClient,
    switch_target: Option<String>,
    interactive: bool,
) -> Result<()> {
    let devices = client.get_devices().await?;

    if devices.is_empty() {
        println!("{}", "No active Spotify devices found. Make sure Spotify is open on your PC/phone.".yellow());
        return Ok(());
    }

    if switch_target.is_none() && !interactive {
        print_device_list(&devices);
        return Ok(());
    }

    // Handle device switching
    let selected_device = if let Some(target) = switch_target {
        let target_lower = target.to_lowercase();
        let matches: Vec<&Device> = devices
            .iter()
            .filter(|d| d.name.to_lowercase().contains(&target_lower) || d.device_type.to_lowercase().contains(&target_lower))
            .collect();

        match matches.len() {
            0 => {
                println!("No device found matching '{}'. Select from the list below:", target.yellow());
                prompt_select_device(&devices)?
            }
            1 => matches[0].clone(),
            _ => {
                println!("Multiple devices matched '{}'. Select one:", target.cyan());
                let options: Vec<Device> = matches.into_iter().cloned().collect();
                prompt_select_device(&options)?
            }
        }
    } else {
        prompt_select_device(&devices)?
    };

    if selected_device.is_active {
        println!("Device '{}' is already active.", selected_device.name.cyan());
        return Ok(());
    }

    if let Some(ref id) = selected_device.id {
        println!("Switching playback to '{}' ({})...", selected_device.name.bold(), selected_device.device_type);
        client.transfer_playback(id, true).await?;
        println!("{}", "✓ Playback transferred successfully!".green());
    } else {
        bail!("Selected device does not have a valid ID");
    }

    Ok(())
}

fn prompt_select_device(devices: &[Device]) -> Result<Device> {
    let labels: Vec<String> = devices
        .iter()
        .map(|d| {
            let status = if d.is_active { " (active)" } else { "" };
            format!("{} - {}{}", d.name, d.device_type, status)
        })
        .collect();

    let ans = inquire::Select::new("Select Spotify device to activate:", labels)
        .prompt()
        .context("Device selection cancelled")?;

    let idx = devices
        .iter()
        .position(|d| {
            let status = if d.is_active { " (active)" } else { "" };
            format!("{} - {}{}", d.name, d.device_type, status) == ans
        })
        .context("Device not found")?;

    Ok(devices[idx].clone())
}
