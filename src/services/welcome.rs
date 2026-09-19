use serenity::all::{CreateAttachment, CreateEmbed, CreateMessage, Member};
use serenity::http::Http;
use serenity::model::id::{ChannelId, RoleId};
use serenity::prelude::Mentionable;

use crate::utilities::welcome;

const JOIN_ROLE: RoleId = RoleId::new(1370956790854057984);

pub async fn handle(ctx: &Http, member: &Member) {
    if let Err(why) = member.add_role(ctx, JOIN_ROLE).await {
        tracing::error!(error = ?why, user = %member.user.id, "Error al asignar rol de bienvenida");
    }

    let channel_id = match std::env::var("WELCOME_CHANNEL_ID") {
        Ok(value) if value.trim().is_empty() => return,
        Ok(value) => match value.trim().parse::<u64>() {
            Ok(id) if id != 0 => ChannelId::new(id),
            _ => {
                tracing::error!("WELCOME_CHANNEL_ID debe ser un ID numérico mayor que cero");
                return;
            }
        },
        Err(std::env::VarError::NotPresent) => return,
        Err(why) => {
            tracing::error!(error = ?why, "No se pudo leer WELCOME_CHANNEL_ID");
            return;
        }
    };

    let preview = match member.guild_id.get_preview(ctx).await {
        Ok(p) => p,
        Err(why) => {
            tracing::error!(error = ?why, user = %member.user.id, "Error al obtener info del servidor para welcome");
            return;
        }
    };

    let png = match welcome::generate_banner(
        &member.user.name,
        &member.user.face(),
        preview.approximate_member_count,
    )
    .await
    {
        Ok(b) => b,
        Err(why) => {
            tracing::error!(error = ?why, user = %member.user.id, "Error al generar imagen de bienvenida");
            return;
        }
    };

    let attachment = CreateAttachment::bytes(png, "welcome.png");
    let embed = CreateEmbed::new()
        .title(format!("¡Bienvenido a {}!", preview.name))
        .description(format!(
            "Hola {}, bienvenido al servidor oficial.",
            member.user.mention()
        ))
        .color(0xFFFFFF)
        .image("attachment://welcome.png");

    if let Err(why) = channel_id
        .send_message(ctx, CreateMessage::new().embed(embed).add_file(attachment))
        .await
    {
        tracing::error!(error = ?why, user = %member.user.id, "Error al enviar mensaje de bienvenida");
    } else {
        tracing::info!(user = %member.user.name, "Welcome enviado");
    }
}
