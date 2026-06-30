pub mod ping;
pub mod sendpanel;

use std::future::Future;
use std::pin::Pin;

use serenity::all::{CommandInteraction, CreateCommand, EditInteractionResponse};
use serenity::async_trait;
use serenity::builder::CreateInteractionResponseMessage;
use serenity::prelude::Context;

pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

pub type DeferredFuture =
    Pin<Box<dyn Future<Output = Result<EditInteractionResponse, BoxedError>> + Send + 'static>>;

pub enum CommandResult {
    Message(Box<CreateInteractionResponseMessage>),
    Deferred(DeferredFuture), // Esto sera usado proximamente, no eliminar xd
}

#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &'static str;
    fn register(&self) -> CreateCommand;
    async fn execute(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
    ) -> Result<CommandResult, BoxedError>;
}

pub fn all_commands() -> Vec<Box<dyn Command>> {
    vec![Box::new(ping::Ping), Box::new(sendpanel::SendPanel)]
}
