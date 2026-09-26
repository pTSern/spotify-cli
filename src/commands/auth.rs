use crate::api::SpotifyClient;
use crate::auth::login as do_login;
use crate::config::Config;
use anyhow::{Context, Result};
use colored::Colorize;

pub async fn run_login(config: &mut Config) -> Result<()> {
    do_login(config).await
}

pub async fn run_logout(config: &mut Config) -> Result<()> {
    config.clear_tokens()?;
    println!("{}", "Logged out successfully. Local credentials cleared.".green());
    Ok(())
}

pub async fn run_status(client: &mut SpotifyClient) -> Result<()> {
    match client.get_me().await {
        Ok(user) => {
            let name = user.display_name.unwrap_or(user.id);
            let product = user.product.unwrap_or_else(|| "free".to_string());
            println!("\n{}", "Spotify Account Status:".bold());
            println!("  User:    {}", name.cyan().bold());
            println!("  Plan:    {}", product.green());
            if let Some(email) = user.email {
                println!("  Email:   {}", email.bright_black());
            }
            if let Ok(path) = Config::config_path() {
                println!("  Config:  {}", path.display().to_string().bright_black());
            }
            println!();
        }
        Err(e) => {
            eprintln!("{}: {}", "Error retrieving user profile".red(), e);
        }
    }
    Ok(())
}

pub async fn run_setup(config: &mut Config) -> Result<()> {
    println!("{}", "Configure Spotify Developer App Client ID".cyan().bold());
    println!("Get your Client ID from: https://developer.spotify.com/dashboard\n");

    let current = config.client_id.as_deref().unwrap_or("none");
    println!("Current Client ID: {}", current.bright_black());

    let input: String = inquire::Text::new("Enter new Spotify Client ID: ")
        .prompt()
        .context("Prompt cancelled")?;

    let trimmed = input.trim().to_string();
    if !trimmed.is_empty() {
        config.client_id = Some(trimmed);
        config.save()?;
        println!("{}", "Client ID saved! Now run `spotify auth login` to authorize.".green());
    }
    Ok(())
}
