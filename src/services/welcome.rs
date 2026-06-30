use serenity::all::{CreateAttachment, CreateEmbed, CreateMessage, Member};
use serenity::http::Http;
use serenity::model::id::{ChannelId, RoleId};
use serenity::prelude::Mentionable;

use crate::utilities::welcome;

const JOIN_ROLE: RoleId = RoleId::new(1370956790854057984);
const WELCOME_CHANNEL: ChannelId = ChannelId::new(1368648155553468508);

pub async fn handle(ctx: &Http, member: &Member) {
    if let Err(why) = member.add_role(ctx, JOIN_ROLE).await {
        tracing::error!(error = ?why, user = %member.user.id, "Error al asignar rol de bienvenida");
    }

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

    if let Err(why) = WELCOME_CHANNEL
        .send_message(ctx, CreateMessage::new().embed(embed).add_file(attachment))
        .await
    {
        tracing::error!(error = ?why, user = %member.user.id, "Error al enviar mensaje de bienvenida");
    } else {
        tracing::info!(user = %member.user.name, "Welcome enviado");
    }
}
