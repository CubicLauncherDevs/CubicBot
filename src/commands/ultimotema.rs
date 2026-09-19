use serenity::all::{
    CommandInteraction, CreateCommand, CreateInteractionResponseMessage, EditInteractionResponse,
    Permissions,
};
use serenity::async_trait;
use serenity::prelude::Context;

use crate::commands::{BoxedError, Command, CommandResult};
use crate::services::themes;

pub struct UltimoTema;

#[async_trait]
impl Command for UltimoTema {
    fn name(&self) -> &'static str {
        "ultimotema"
    }

    fn register(&self) -> CreateCommand {
        CreateCommand::new(self.name())
            .description("Envía el último tema publicado al canal de temas configurado")
            .default_member_permissions(Permissions::MANAGE_GUILD)
            .dm_permission(false)
    }

    async fn execute(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
    ) -> Result<CommandResult, BoxedError> {
        let permissions = command
            .member
            .as_ref()
            .and_then(|member| member.permissions);
        if command.guild_id.is_none()
            || !permissions.is_some_and(|p| {
                p.intersects(Permissions::MANAGE_GUILD | Permissions::ADMINISTRATOR)
            })
        {
            return Ok(CommandResult::Message(Box::new(
                CreateInteractionResponseMessage::new()
                    .content("Necesitas el permiso Administrar servidor para usar este comando en un servidor.")
                    .ephemeral(true),
            )));
        }

        let guild = command
            .guild_id
            .ok_or("Este comando solo funciona en un servidor")?;
        let http = ctx.http.clone();
        Ok(CommandResult::Deferred {
            ephemeral: true,
            future: Box::pin(async move {
                let channel = themes::send_latest(&http, guild).await?;
                Ok(EditInteractionResponse::new()
                    .content(format!("✅ Último tema enviado a <#{channel}>.")))
            }),
        })
    }
}
