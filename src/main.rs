mod commands;
mod handler;
mod services;
mod utilities;
use std::env;

use serenity::prelude::*;

#[tokio::main]
async fn main() {
    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(dotenvy::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            eprintln!("No se pudo cargar .env: {error}");
            return;
        }
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("cubicbot=info")),
        )
        .init();

    println!("{}", include_str!("../Assets/splash.txt"));

    let token = match env::var("DISCORD_TOKEN") {
        Ok(token) if !token.trim().is_empty() => token,
        _ => {
            tracing::error!("Configura DISCORD_TOKEN en el archivo .env antes de iniciar el bot");
            return;
        }
    };

    let mut client = Client::builder(token, GatewayIntents::GUILD_MEMBERS)
        .event_handler(handler::Handler)
        .await
        .expect("Error creating client");

    let themes_task = match services::themes::start(client.http.clone()) {
        Ok(task) => task,
        Err(error) => {
            tracing::error!(error = %error, "Configuración de temas inválida; revisa .env y reinicia el bot");
            None
        }
    };

    if let Err(why) = client.start().await {
        tracing::error!(error = ?why, "Client error");
    }
    if let Some(task) = themes_task {
        task.abort();
    }
}
