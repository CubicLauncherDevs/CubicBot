mod commands;
mod handler;
mod services;
mod utilities;
use std::env;

use serenity::prelude::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("bot=info")),
        )
        .init();

    println!("{}", include_str!("../Assets/splash.txt"));

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    utilities::ensure_directories().await;

    let mut client = Client::builder(token, GatewayIntents::GUILD_MEMBERS)
        .event_handler(handler::Handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        tracing::error!(error = ?why, "Client error");
    }
}
