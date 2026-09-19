use std::env;

use serenity::all::{Channel, CreateAutocompleteResponse, EditInteractionResponse, Member};
use serenity::async_trait;
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::model::application::Interaction;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::prelude::*;

use crate::commands::{self, CommandResult};
use crate::services;

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Autocomplete(command) => {
                let all_cmds = commands::all_commands();
                let response = all_cmds
                    .iter()
                    .find(|cmd| cmd.name() == command.data.name)
                    .map(|cmd| cmd.autocomplete(&command))
                    .unwrap_or_else(CreateAutocompleteResponse::new);
                if let Err(why) = command
                    .create_response(&ctx.http, CreateInteractionResponse::Autocomplete(response))
                    .await
                {
                    tracing::error!(error = ?why, command = %command.data.name, "Error al responder autocompletado");
                }
            }
            Interaction::Command(command) => {
                let cmd_name = command.data.name.clone();
                let all_cmds = commands::all_commands();
                let cmd = all_cmds.iter().find(|c| c.name() == cmd_name);

                let Some(cmd) = cmd else {
                    let _ = command
                        .create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Comando no encontrado"),
                            ),
                        )
                        .await;
                    return;
                };

                match cmd.execute(&ctx, &command).await {
                    Ok(CommandResult::Message(data)) => {
                        let builder = CreateInteractionResponse::Message(*data);
                        if let Err(why) = command.create_response(&ctx.http, builder).await {
                            tracing::error!(error = ?why, "Error al responder comando");
                        }
                    }
                    Ok(CommandResult::Deferred { future, ephemeral }) => {
                        if let Err(why) = command
                            .create_response(
                                &ctx.http,
                                CreateInteractionResponse::Defer(
                                    CreateInteractionResponseMessage::new().ephemeral(ephemeral),
                                ),
                            )
                            .await
                        {
                            tracing::error!(error = ?why, "Cannot defer on work");
                            return;
                        }
                        match future.await {
                            Ok(edit) => {
                                if let Err(why) = command.edit_response(&ctx.http, edit).await {
                                    tracing::error!(error = ?why, "Error al editar mensaje");
                                }
                            }
                            Err(why) => {
                                tracing::error!(error = %why, "Error ejecutando comando diferido");
                                let _ = command
                                    .edit_response(
                                        &ctx.http,
                                        EditInteractionResponse::new().content(format!("{why}")),
                                    )
                                    .await;
                            }
                        }
                    }
                    Err(why) => {
                        tracing::error!(error = ?why, "Error ejecutando comando");
                        let _ = command
                            .create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content(format!("Error: {why}")),
                                ),
                            )
                            .await;
                    }
                }
            }
            Interaction::Component(component) => {
                let custom_id = component.data.custom_id.clone();
                let guild_id = match component.guild_id {
                    Some(g) => g,
                    None => return,
                };
                let http = &ctx.http;

                match custom_id.as_str() {
                    "ticket_create" => {
                        tracing::info!(user = %component.user.id, "Creando ticket");

                        let _ = component
                            .create_response(
                                http,
                                CreateInteractionResponse::Defer(
                                    CreateInteractionResponseMessage::new().ephemeral(true),
                                ),
                            )
                            .await;

                        services::tickets::create_ticket(http, guild_id, &component.user).await;

                        let _ = component
                            .edit_response(
                                http,
                                EditInteractionResponse::new()
                                    .content("¡Ticket creado! Revisa el canal creado."),
                            )
                            .await;
                    }
                    "ticket_close" => {
                        let roles: Vec<_> = component
                            .member
                            .as_ref()
                            .map(|m| m.roles.clone())
                            .unwrap_or_default();

                        if !services::tickets::is_staff(&roles)
                            && !services::tickets::is_ticket_owner(
                                http,
                                component.user.id,
                                component.channel_id,
                            )
                            .await
                        {
                            let _ = component
                                .create_response(
                                    http,
                                    CreateInteractionResponse::Message(
                                        CreateInteractionResponseMessage::new()
                                            .content("No tienes permiso para cerrar este ticket.")
                                            .ephemeral(true),
                                    ),
                                )
                                .await;
                            return;
                        }

                        tracing::info!(
                            channel = %component.channel_id,
                            closer = %component.user.id,
                            "Cerrando ticket"
                        );

                        let _ = component
                            .create_response(
                                http,
                                CreateInteractionResponse::Defer(
                                    CreateInteractionResponseMessage::new(),
                                ),
                            )
                            .await;

                        match component.channel_id.to_channel(http).await {
                            Ok(Channel::Guild(channel)) => {
                                services::tickets::close_ticket(http, &channel, component.user.id)
                                    .await;
                            }
                            _ => {
                                let _ = component
                                    .edit_response(
                                        http,
                                        EditInteractionResponse::new()
                                            .content("Error al obtener el canal."),
                                    )
                                    .await;
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    async fn guild_member_addition(&self, ctx: Context, new_member: Member) {
        services::welcome::handle(&ctx.http, &new_member).await;
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!(user = %ready.user.name, "Bot conectado");

        let guild_id = match env::var("GUILD_ID") {
            Ok(value) => match value.parse::<u64>() {
                Ok(id) if id != 0 => GuildId::new(id),
                _ => {
                    tracing::error!(
                        "GUILD_ID debe ser el ID numérico del servidor, mayor que cero. No se registraron los comandos slash."
                    );
                    return;
                }
            },
            Err(why) => {
                tracing::error!(error = ?why, "No se pudo leer GUILD_ID. Configura el ID del servidor y reinicia el bot para registrar los comandos slash.");
                return;
            }
        };

        let all_cmds = commands::all_commands();
        let commands: Vec<_> = all_cmds.iter().map(|cmd| cmd.register()).collect();

        match guild_id.set_commands(&ctx.http, commands).await {
            Ok(registered) => {
                tracing::info!(guild = %guild_id, count = registered.len(), "Comandos slash registrados");
            }
            Err(why) => {
                tracing::error!(error = ?why, guild = %guild_id, "Error al registrar comandos slash. Comprueba GUILD_ID y que el bot esté instalado en ese servidor con el scope applications.commands.");
            }
        }
    }
}
