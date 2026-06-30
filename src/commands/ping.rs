use serenity::all::{CommandInteraction, CreateCommand};
use serenity::async_trait;
use serenity::builder::CreateInteractionResponseMessage;
use serenity::prelude::Context;

use crate::commands::{BoxedError, Command, CommandResult};
use crate::utilities::format_duration;

pub struct Ping;

#[async_trait]
impl Command for Ping {
    fn name(&self) -> &'static str {
        "ping"
    }

    fn register(&self) -> CreateCommand {
        CreateCommand::new("ping").description("A ping command")
    }

    async fn execute(
        &self,
        _ctx: &Context,
        command: &CommandInteraction,
    ) -> Result<CommandResult, BoxedError> {
        let timestamp = command.id.created_at();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let elapsed_ms = now_ms - timestamp.timestamp_millis();
        let elapsed = std::time::Duration::from_millis(elapsed_ms as u64);

        Ok(CommandResult::Message(Box::new(
            CreateInteractionResponseMessage::new().content(format_duration(elapsed)),
        )))
    }
}
