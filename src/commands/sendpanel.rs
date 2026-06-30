use serenity::all::{CommandInteraction, CreateCommand, UserId};
use serenity::async_trait;
use serenity::builder::CreateInteractionResponseMessage;
use serenity::prelude::Context;

use crate::commands::{BoxedError, Command, CommandResult};
use crate::services;
// santiago y motstaff
const ALLOWED: [UserId; 2] = [
    UserId::new(1224181541127717034),
    UserId::new(962589013921918996),
];

pub struct SendPanel;

#[async_trait]
impl Command for SendPanel {
    fn name(&self) -> &'static str {
        "sendpanel"
    }

    fn register(&self) -> CreateCommand {
        CreateCommand::new("sendpanel")
            .description("Envía el panel de tickets al canal configurado")
    }

    async fn execute(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
    ) -> Result<CommandResult, BoxedError> {
        if !ALLOWED.contains(&command.user.id) {
            return Ok(CommandResult::Message(Box::new(
                CreateInteractionResponseMessage::new()
                    .content("No tienes permiso para usar este comando.")
                    .ephemeral(true),
            )));
        }

        let guild_id = command
            .guild_id
            .ok_or("Este comando solo funciona en un servidor.")?;
        services::tickets::send_panel(&ctx.http, guild_id).await;

        Ok(CommandResult::Message(Box::new(
            CreateInteractionResponseMessage::new()
                .content("✅ Panel de tickets enviado.")
                .ephemeral(true),
        )))
    }
}
