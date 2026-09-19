use std::collections::BTreeSet;
use std::env;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serenity::all::{
    Channel, ChannelId, CreateActionRow, CreateAllowedMentions, CreateButton, CreateEmbed,
    CreateEmbedFooter, CreateMessage, GuildId, Http,
};
use serenity::async_trait;
use tokio::fs;
use tokio::task::JoinHandle;

use crate::commands::BoxedError;

const CATALOG_URL: &str =
    "https://raw.githubusercontent.com/CubicLauncherDevs/Themes/master/themes.json";
const THEMES_URL: &str = "https://www.cubiclauncher.org/themes";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Theme {
    id: String,
    name: String,
    author: String,
    dir_path: String,
    description: Option<String>,
    preview_url: Option<String>,
    latest_version: String,
    date: Option<String>,
    versions: Vec<Version>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Version {
    version: String,
    date: String,
    showcase_url: Option<String>,
    preview_url: Option<String>,
}

impl Theme {
    fn published_at(&self) -> Result<DateTime<FixedOffset>, BoxedError> {
        // La fecha del catálogo puede cambiar al actualizar el tema.
        // Su primera versión identifica la publicación original.
        let dates = self
            .versions
            .iter()
            .map(|version| DateTime::parse_from_rfc3339(&version.date))
            .collect::<Result<Vec<_>, _>>()?;
        match dates.into_iter().min() {
            Some(date) => Ok(date),
            None => Ok(DateTime::parse_from_rfc3339(
                self.date
                    .as_deref()
                    .ok_or("Tema sin fecha de publicación")?,
            )?),
        }
    }

    fn message(&self) -> CreateMessage {
        let mut url =
            reqwest::Url::parse("https://github.com/CubicLauncherDevs/Themes/tree/master/")
                .expect("URL del repositorio válida");
        url.path_segments_mut()
            .expect("URL con segmentos")
            .pop_if_empty()
            .extend(self.dir_path.split('/'));

        let mut embed = CreateEmbed::new()
            .title(shorten(&format!("Nuevo tema: {}", self.name), 256))
            .url(url.as_str())
            .color(0x5865F2)
            .field("Autor", shorten(&self.author, 256), true)
            .field("Versión", shorten(&self.latest_version, 128), true)
            .footer(CreateEmbedFooter::new("Temas de CubicLauncher"));

        if let Some(description) = self.description.as_deref().filter(|s| !s.trim().is_empty()) {
            embed = embed.description(shorten(description, 2000));
        }

        let latest = self
            .versions
            .iter()
            .find(|v| v.version == self.latest_version);
        let image = latest
            .and_then(|v| v.showcase_url.as_deref().or(v.preview_url.as_deref()))
            .or(self.preview_url.as_deref());
        if let Some(image) = image.and_then(|value| reqwest::Url::parse(value).ok())
            && matches!(image.scheme(), "https" | "http")
        {
            embed = embed.image(image.as_str());
        }

        CreateMessage::new()
            .embed(embed)
            .allowed_mentions(
                CreateAllowedMentions::new()
                    .all_users(false)
                    .all_roles(false)
                    .everyone(false),
            )
            .components(vec![CreateActionRow::Buttons(vec![
                CreateButton::new_link(url.as_str()).label("Ver tema en GitHub"),
                CreateButton::new_link(THEMES_URL).label("Catálogo de temas"),
            ])])
    }
}

// Discord limita el texto por unidades UTF-16, no por bytes UTF-8.
fn shorten(text: &str, limit: usize) -> String {
    let mut length = 0;
    text.chars()
        .take_while(|c| {
            length += c.len_utf16();
            length <= limit
        })
        .collect()
}

fn configured_channel() -> Result<Option<ChannelId>, BoxedError> {
    match env::var("THEMES_CHANNEL_ID") {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => {
            let id = value.trim().parse::<u64>()?;
            if id == 0 {
                return Err("THEMES_CHANNEL_ID debe ser mayor que cero".into());
            }
            Ok(Some(ChannelId::new(id)))
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn http_client() -> Result<reqwest::Client, BoxedError> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("CubicBot/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()?)
}

async fn fetch_catalog(client: &reqwest::Client) -> Result<Vec<Theme>, BoxedError> {
    let bytes = client
        .get(CATALOG_URL)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    parse_catalog(&bytes)
}

fn parse_catalog(bytes: &[u8]) -> Result<Vec<Theme>, BoxedError> {
    let themes: Vec<Theme> = serde_json::from_slice(bytes)?;
    let mut ids = BTreeSet::new();
    for theme in &themes {
        if theme.id.trim().is_empty() || !ids.insert(&theme.id) {
            return Err("El catálogo contiene IDs vacíos o duplicados".into());
        }
    }
    Ok(themes)
}

fn latest_theme(themes: &[Theme]) -> Result<&Theme, BoxedError> {
    let mut dated = themes
        .iter()
        .map(|theme| Ok((theme.published_at()?, theme)))
        .collect::<Result<Vec<_>, BoxedError>>()?;
    dated.sort_by(|(a_date, a), (b_date, b)| a_date.cmp(b_date).then(a.id.cmp(&b.id)));
    dated
        .last()
        .map(|(_, theme)| *theme)
        .ok_or_else(|| "El catálogo está vacío".into())
}

pub(crate) async fn send_latest(http: &Http, guild: GuildId) -> Result<ChannelId, BoxedError> {
    let channel =
        configured_channel()?.ok_or("Configura THEMES_CHANNEL_ID en .env y reinicia el bot")?;
    match channel.to_channel(http).await? {
        Channel::Guild(destination) if destination.guild_id == guild => {}
        _ => return Err("El canal de temas debe pertenecer a este servidor".into()),
    }
    let themes = fetch_catalog(&http_client()?).await?;
    channel
        .send_message(http, latest_theme(&themes)?.message())
        .await?;
    Ok(channel)
}

#[derive(Deserialize, Serialize)]
struct State {
    seen_ids: BTreeSet<String>,
}

struct Monitor {
    path: PathBuf,
    state: Option<State>,
    dirty: bool,
}

#[async_trait]
trait ThemeSender: Sync {
    async fn send(&self, theme: &Theme) -> Result<(), BoxedError>;
}

struct DiscordSender {
    http: Arc<Http>,
    channel: ChannelId,
}

#[async_trait]
impl ThemeSender for DiscordSender {
    async fn send(&self, theme: &Theme) -> Result<(), BoxedError> {
        self.channel
            .send_message(&self.http, theme.message())
            .await?;
        tracing::info!(theme = %theme.id, channel = %self.channel, "Tema anunciado");
        Ok(())
    }
}

impl Monitor {
    async fn load(path: PathBuf) -> Result<Self, BoxedError> {
        let state = match fs::read(&path).await {
            Ok(bytes) => Some(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            path,
            state,
            dirty: false,
        })
    }

    async fn save(&mut self) -> Result<(), BoxedError> {
        if !self.dirty {
            return Ok(());
        }
        let state = self
            .state
            .as_ref()
            .ok_or("Estado de temas no inicializado")?;
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).await?;
        }
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(state)?).await?;
        fs::rename(temporary, &self.path).await?;
        self.dirty = false;
        Ok(())
    }

    async fn process(
        &mut self,
        themes: &[Theme],
        sender: &impl ThemeSender,
    ) -> Result<(), BoxedError> {
        // Reintentar primero una escritura fallida, sin repetir el envío en esta ejecución.
        self.save().await?;
        if self.state.is_none() {
            self.state = Some(State {
                seen_ids: themes.iter().map(|t| t.id.clone()).collect(),
            });
            self.dirty = true;
            self.save().await?;
            tracing::info!(
                count = themes.len(),
                "Seguimiento de temas inicializado; temas existentes guardados sin anunciarlos"
            );
            return Ok(());
        }

        for theme in themes {
            if self
                .state
                .as_ref()
                .is_some_and(|s| s.seen_ids.contains(&theme.id))
            {
                continue;
            }
            if let Err(error) = sender.send(theme).await {
                tracing::error!(error = %error, theme = %theme.id, "No se pudo anunciar el tema; se reintentará en la próxima consulta");
                continue;
            }
            self.state
                .as_mut()
                .expect("Estado inicializado")
                .seen_ids
                .insert(theme.id.clone());
            self.dirty = true;
            self.save().await?;
        }
        Ok(())
    }
}

pub(crate) fn start(http: Arc<Http>) -> Result<Option<JoinHandle<()>>, BoxedError> {
    let Some(channel) = configured_channel()? else {
        tracing::info!("Avisos de temas desactivados: configura THEMES_CHANNEL_ID en .env");
        return Ok(None);
    };
    let seconds = match env::var("THEMES_POLL_SECONDS") {
        Ok(value) => value.parse::<u64>()?,
        Err(env::VarError::NotPresent) => 300,
        Err(error) => return Err(error.into()),
    };
    if seconds < 30 {
        return Err("THEMES_POLL_SECONDS debe ser al menos 30".into());
    }
    let client = http_client()?;
    let path = Path::new("data").join(format!("themes-{channel}.json"));
    Ok(Some(tokio::spawn(async move {
        let mut monitor = match Monitor::load(path).await {
            Ok(monitor) => monitor,
            Err(error) => {
                tracing::error!(error = %error, "No se pudo cargar el estado de temas; seguimiento detenido. Corrige el archivo y reinicia el bot");
                return;
            }
        };
        let sender = DiscordSender { http, channel };
        tracing::info!(channel = %channel, seconds, "Seguimiento de temas activado");
        loop {
            let result = async {
                monitor.save().await?;
                let themes = fetch_catalog(&client).await?;
                monitor.process(&themes, &sender).await
            }
            .await;
            if let Err(error) = result {
                tracing::error!(error = %error, "Error al comprobar temas; se reintentará en la próxima consulta");
            }
            tokio::time::sleep(Duration::from_secs(seconds)).await;
        }
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    fn theme(id: &str, date: &str) -> Theme {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "name": id,
            "author": "CubicLauncher",
            "dirPath": format!("src/CubicLauncher/{id}"),
            "latestVersion": "V1",
            "versions": [{"version": "V1", "date": date}]
        }))
        .unwrap()
    }

    #[derive(Default)]
    struct RecordingSender {
        sent: Mutex<Vec<String>>,
        fail: AtomicBool,
    }

    #[async_trait]
    impl ThemeSender for RecordingSender {
        async fn send(&self, theme: &Theme) -> Result<(), BoxedError> {
            if self.fail.load(Ordering::Relaxed) {
                return Err("Fallo simulado de Discord".into());
            }
            self.sent.lock().unwrap().push(theme.id.clone());
            Ok(())
        }
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "cubicbot-themes-{}-{stamp}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn state_path(&self) -> PathBuf {
            self.0.join("data/themes.json")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn latest_uses_first_version_and_compares_timezones() {
        let mut old = theme("old-updated", "2026-01-01T00:00:00Z");
        old.versions.push(Version {
            version: "V2".into(),
            date: "2026-12-01T00:00:00Z".into(),
            showcase_url: None,
            preview_url: None,
        });
        let themes = vec![
            old,
            theme("newest", "2026-09-19T01:00:00-06:00"),
            theme("earlier", "2026-09-19T06:00:00Z"),
        ];
        assert_eq!(latest_theme(&themes).unwrap().id, "newest");
        assert!(latest_theme(&[]).is_err());
        assert!(latest_theme(&[theme("bad", "not-a-date")]).is_err());
    }

    #[test]
    fn rejects_invalid_catalog_without_treating_it_as_empty() {
        assert!(parse_catalog(b"<html>server error</html>").is_err());
        assert!(parse_catalog(br#"[{"id":"incomplete"}]"#).is_err());
        let entry = serde_json::json!({
            "id": "same", "name": "Theme", "author": "Author", "dirPath": "src/A/B",
            "latestVersion": "V1", "versions": []
        });
        let bytes = serde_json::to_vec(&vec![entry.clone(), entry]).unwrap();
        assert!(parse_catalog(&bytes).is_err());
    }

    #[test]
    fn embed_supports_null_preview_spaces_and_discord_text_limits() {
        let mut theme = theme("Signature Amber", "2026-09-19T01:00:00Z");
        theme.description = Some("🎨".repeat(3000));
        theme.versions[0].showcase_url =
            Some("https://themes.cubiclauncher.org/Show case.png".into());
        let message = serde_json::to_value(theme.message()).unwrap();
        let embed = &message["embeds"][0];
        assert!(
            embed["url"]
                .as_str()
                .unwrap()
                .ends_with("Signature%20Amber")
        );
        assert_eq!(
            embed["description"]
                .as_str()
                .unwrap()
                .encode_utf16()
                .count(),
            2000
        );
        assert!(
            embed["image"]["url"]
                .as_str()
                .unwrap()
                .ends_with("Show%20case.png")
        );
        assert_eq!(message["allowed_mentions"]["parse"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn baseline_restart_updates_and_reappearing_themes() {
        let directory = TestDirectory::new();
        let sender = RecordingSender::default();
        let mut themes = vec![theme("existing", "2026-01-01T00:00:00Z")];
        let mut monitor = Monitor::load(directory.state_path()).await.unwrap();
        monitor.process(&themes, &sender).await.unwrap();
        assert!(sender.sent.lock().unwrap().is_empty());

        // Simula un reinicio y un tema publicado mientras el bot estaba apagado.
        let mut monitor = Monitor::load(directory.state_path()).await.unwrap();
        themes[0].latest_version = "V2".into();
        themes.push(theme("new", "2026-09-19T00:00:00Z"));
        monitor.process(&themes, &sender).await.unwrap();
        monitor.process(&[], &sender).await.unwrap();
        let mut monitor = Monitor::load(directory.state_path()).await.unwrap();
        monitor.process(&themes, &sender).await.unwrap();
        assert_eq!(*sender.sent.lock().unwrap(), ["new"]);
    }

    #[tokio::test]
    async fn failed_delivery_is_retried_after_restart() {
        let directory = TestDirectory::new();
        let sender = RecordingSender::default();
        let mut monitor = Monitor::load(directory.state_path()).await.unwrap();
        monitor.process(&[], &sender).await.unwrap();
        let themes = vec![theme("new", "2026-09-19T00:00:00Z")];
        sender.fail.store(true, Ordering::Relaxed);
        monitor.process(&themes, &sender).await.unwrap();
        let mut monitor = Monitor::load(directory.state_path()).await.unwrap();
        assert!(!monitor.state.as_ref().unwrap().seen_ids.contains("new"));

        sender.fail.store(false, Ordering::Relaxed);
        monitor.process(&themes, &sender).await.unwrap();
        monitor.process(&themes, &sender).await.unwrap();
        assert_eq!(*sender.sent.lock().unwrap(), ["new"]);
    }

    #[tokio::test]
    async fn retries_state_write_without_resending_in_same_process() {
        let directory = TestDirectory::new();
        let sender = RecordingSender::default();
        let mut monitor = Monitor::load(directory.state_path()).await.unwrap();
        monitor.process(&[], &sender).await.unwrap();
        let temporary = monitor.path.with_extension("json.tmp");
        fs::create_dir(&temporary).await.unwrap();
        let themes = vec![theme("new", "2026-09-19T00:00:00Z")];
        assert!(monitor.process(&themes, &sender).await.is_err());
        assert!(monitor.dirty);
        fs::remove_dir(&temporary).await.unwrap();
        monitor.process(&themes, &sender).await.unwrap();
        let monitor = Monitor::load(directory.state_path()).await.unwrap();
        assert!(monitor.state.as_ref().unwrap().seen_ids.contains("new"));
        assert_eq!(*sender.sent.lock().unwrap(), ["new"]);
    }

    #[tokio::test]
    async fn corrupt_state_is_not_silently_reset() {
        let directory = TestDirectory::new();
        let path = directory.0.join("broken.json");
        fs::write(&path, b"{broken").await.unwrap();
        assert!(Monitor::load(path.clone()).await.is_err());
        assert_eq!(fs::read(&path).await.unwrap(), b"{broken");
    }
}
