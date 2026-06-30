use std::collections::HashMap;
use std::sync::LazyLock;

use serenity::all::{ChannelType, GuildChannel, Timestamp, User, UserId};
use serenity::builder::{
    CreateActionRow, CreateButton, CreateChannel, CreateEmbed, CreateEmbedFooter, CreateMessage,
    GetMessages,
};
use serenity::http::Http;
use serenity::model::channel::{PermissionOverwrite, PermissionOverwriteType};
use serenity::model::id::{ChannelId, GuildId, RoleId};
use serenity::model::permissions::Permissions;
use serenity::prelude::Mentionable;
use tokio::sync::RwLock;

static ACTIVE_TICKETS: LazyLock<RwLock<HashMap<UserId, ChannelId>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

struct TicketConfig {
    category: ChannelId,
    staff_role: RoleId,
    log_channel: ChannelId,
    panel_channel: ChannelId,
}

static CONFIG: LazyLock<TicketConfig> = LazyLock::new(|| {
    fn ev(var: &str) -> u64 {
        std::env::var(var)
            .unwrap_or_else(|_| panic!("falta la variable de entorno {var}"))
            .parse()
            .unwrap_or_else(|_| panic!("{var} no es un entero válido"))
    }
    TicketConfig {
        category: ChannelId::new(ev("TICKET_CATEGORY_ID")),
        staff_role: RoleId::new(ev("STAFF_ROLE_ID")),
        log_channel: ChannelId::new(ev("TICKET_LOG_CHANNEL_ID")),
        panel_channel: ChannelId::new(ev("TICKET_PANEL_CHANNEL_ID")),
    }
});

pub fn is_staff(roles: &[RoleId]) -> bool {
    roles.contains(&CONFIG.staff_role)
}

pub async fn is_ticket_owner(_http: &Http, user_id: UserId, channel_id: ChannelId) -> bool {
    ACTIVE_TICKETS
        .read()
        .await
        .get(&user_id)
        .is_some_and(|cid| *cid == channel_id)
}

pub async fn send_panel(http: &Http, _guild_id: GuildId) {
    let row = CreateActionRow::Buttons(vec![
        CreateButton::new("ticket_create")
            .label("🎫 Crear Ticket")
            .style(serenity::all::ButtonStyle::Primary),
    ]);

    let embed = CreateEmbed::new()
        .title("🎫 Sistema de Tickets")
        .description(
            "¿Necesitas ayuda o tienes algún problema?\n\n\
             Haz clic en el botón de abajo para crear un ticket y \
             un miembro del staff te atenderá lo antes posible.",
        )
        .color(0x5865F2)
        .footer(CreateEmbedFooter::new(
            "Sistema de Tickets • Responde rápido",
        ))
        .timestamp(Timestamp::now());

    if let Err(why) = CONFIG
        .panel_channel
        .send_message(
            http,
            CreateMessage::new().embed(embed).components(vec![row]),
        )
        .await
    {
        tracing::error!(error = ?why, "Error al enviar panel de tickets");
    } else {
        tracing::info!("Panel de tickets enviado");
    }
}

pub async fn create_ticket(http: &Http, guild_id: GuildId, user: &User) {
    {
        let active = ACTIVE_TICKETS.read().await;
        if active.contains_key(&user.id) {
            let _ = user
                .direct_message(
                    http,
                    CreateMessage::new().content("Ya tienes un ticket abierto."),
                )
                .await;
            return;
        }
    }

    let ch_name = format!("ticket-{}", user.name.to_lowercase().replace(' ', "-"));

    let everyone_overwrite = PermissionOverwrite {
        allow: Permissions::empty(),
        deny: Permissions::VIEW_CHANNEL,
        kind: PermissionOverwriteType::Role(guild_id.everyone_role()),
    };

    let user_overwrite = PermissionOverwrite {
        allow: Permissions::VIEW_CHANNEL
            | Permissions::SEND_MESSAGES
            | Permissions::READ_MESSAGE_HISTORY,
        deny: Permissions::empty(),
        kind: PermissionOverwriteType::Member(user.id),
    };

    let staff_overwrite = PermissionOverwrite {
        allow: Permissions::VIEW_CHANNEL
            | Permissions::SEND_MESSAGES
            | Permissions::READ_MESSAGE_HISTORY,
        deny: Permissions::empty(),
        kind: PermissionOverwriteType::Role(CONFIG.staff_role),
    };

    let channel = match guild_id
        .create_channel(
            http,
            CreateChannel::new(&ch_name)
                .kind(ChannelType::Text)
                .category(CONFIG.category)
                .permissions(vec![everyone_overwrite, user_overwrite, staff_overwrite]),
        )
        .await
    {
        Ok(c) => c,
        Err(why) => {
            tracing::error!(error = ?why, user = %user.id, "Error al crear canal de ticket");
            return;
        }
    };

    tracing::info!(user = %user.id, channel = %channel.id, "Ticket creado");
    ACTIVE_TICKETS.write().await.insert(user.id, channel.id);

    let close_row = CreateActionRow::Buttons(vec![
        CreateButton::new("ticket_close")
            .label("🔒 Cerrar Ticket")
            .style(serenity::all::ButtonStyle::Danger),
    ]);

    let embed = CreateEmbed::new()
        .title("🎫 Ticket Creado")
        .description(format!(
            "Bienvenido {}. El staff te atenderá pronto.\nUsa el botón para cerrar el ticket.",
            user.mention()
        ))
        .color(0x00FF00);

    let _ = channel
        .id
        .send_message(
            http,
            CreateMessage::new()
                .embed(embed)
                .components(vec![close_row]),
        )
        .await;
}

pub async fn close_ticket(http: &Http, channel: &GuildChannel, closer_id: UserId) {
    let channel_id = channel.id;
    let ticket_user_id = ACTIVE_TICKETS
        .read()
        .await
        .iter()
        .find(|(_, cid)| **cid == channel_id)
        .map(|(uid, _)| *uid);

    let messages = channel_id
        .messages(http, GetMessages::new().limit(100))
        .await
        .unwrap_or_default();

    let mut log = String::new();
    log.push_str("=== TICKET CERRADO ===\n\n");

    if let Some(author_id) = ticket_user_id {
        log.push_str(&format!("Usuario: <@{}> (ID: {})\n", author_id, author_id));
    }
    log.push_str(&format!("Canal: #{}\n", channel.name));
    log.push_str(&format!(
        "Cerrado por: <@{}> (ID: {})\n\n",
        closer_id, closer_id
    ));
    log.push_str("=== MENSAJES ===\n");

    for msg in messages.iter().rev() {
        if msg.author.bot {
            continue;
        }
        let time = msg.timestamp.to_rfc3339().unwrap_or_default();
        log.push_str(&format!(
            "[{}] {}: {}\n",
            time, msg.author.name, msg.content
        ));
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let uid_str = ticket_user_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "desconocido".to_string());
    let filename = format!("ticket-{uid_str}-{ts}.txt");

    let attachment = serenity::builder::CreateAttachment::bytes(log.into_bytes(), &filename);
    let embed = CreateEmbed::new()
        .title("Ticket Cerrado")
        .description(format!(
            "Canal: #{}\nTicket de: {}",
            channel.name,
            ticket_user_id
                .map(|id| format!("<@{}>", id))
                .unwrap_or_else(|| "desconocido".to_string())
        ))
        .color(0xFF0000);

    if let Err(why) = CONFIG
        .log_channel
        .send_message(http, CreateMessage::new().embed(embed).add_file(attachment))
        .await
    {
        tracing::error!(error = ?why, channel = %channel.name, "Error al enviar log de ticket");
    }

    if let Err(why) = channel_id.delete(http).await {
        tracing::error!(error = ?why, channel = %channel.name, "Error al eliminar canal de ticket");
    }

    if let Some(uid) = ticket_user_id {
        tracing::info!(user = %uid, channel = %channel.name, closer = %closer_id, "Ticket cerrado");
        ACTIVE_TICKETS.write().await.remove(&uid);
    }
}
