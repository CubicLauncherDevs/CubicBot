use std::sync::LazyLock;

use serde::Deserialize;
use serenity::all::{
    CommandInteraction, CommandOptionType, CreateActionRow, CreateAllowedMentions,
    CreateAutocompleteResponse, CreateButton, CreateCommand, CreateCommandOption, CreateEmbed,
    CreateEmbedFooter, CreateInteractionResponseMessage,
};
use serenity::async_trait;
use serenity::prelude::Context;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::commands::{BoxedError, Command, CommandResult};

#[derive(Clone, Deserialize)]
struct DocTag {
    name: String,
    title: String,
    aliases: Vec<String>,
    summary: String,
    url: String,
}

static TAGS: LazyLock<Vec<DocTag>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../Assets/tags.json"))
        .expect("Assets/tags.json debe contener un catálogo de tags válido")
});

// También normaliza acentos descompuestos y separadores como espacios o guiones.
fn normalize(value: &str) -> String {
    value
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

impl DocTag {
    fn keys(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.name.as_str())
            .chain(std::iter::once(self.title.as_str()))
            .chain(self.aliases.iter().map(String::as_str))
    }
}

fn find_tag<'a>(tags: &'a [DocTag], query: &str) -> Option<&'a DocTag> {
    let query = normalize(query);
    if query.is_empty() {
        return None;
    }
    tags.iter()
        .find(|tag| tag.keys().any(|key| normalize(key) == query))
}

fn suggestions<'a>(tags: &'a [DocTag], query: &str, limit: usize) -> Vec<&'a DocTag> {
    let query = normalize(query);
    let mut matches: Vec<_> = tags
        .iter()
        .filter_map(|tag| {
            let rank = tag
                .keys()
                .filter_map(|key| {
                    let key = normalize(key);
                    if query.is_empty() || key == query {
                        Some(0)
                    } else if key.starts_with(&query) {
                        Some(1)
                    } else if query.split('-').all(|word| key.contains(word)) {
                        Some(2)
                    } else {
                        None
                    }
                })
                .min()?;
            Some((rank, tag))
        })
        .collect();
    matches.sort_by_key(|(rank, _)| *rank);
    matches
        .into_iter()
        .take(limit)
        .map(|(_, tag)| tag)
        .collect()
}

fn autocomplete_response(tags: &[DocTag], query: &str) -> CreateAutocompleteResponse {
    suggestions(tags, query, 25).into_iter().fold(
        CreateAutocompleteResponse::new(),
        |response, tag| {
            let label: String = format!("{} — {}", tag.name, tag.title)
                .chars()
                .take(100)
                .collect();
            response.add_string_choice(label, &tag.name)
        },
    )
}

fn tag_response(query: &str) -> CreateInteractionResponseMessage {
    let response = CreateInteractionResponseMessage::new().allowed_mentions(
        CreateAllowedMentions::new()
            .all_users(false)
            .all_roles(false)
            .everyone(false),
    );
    if let Some(tag) = find_tag(&TAGS, query) {
        return response
            .embed(
                CreateEmbed::new()
                    .title(&tag.title)
                    .description(&tag.summary)
                    .url(&tag.url)
                    .color(0x5865F2)
                    .footer(CreateEmbedFooter::new("Documentación de CubicLauncher")),
            )
            .components(vec![CreateActionRow::Buttons(vec![
                CreateButton::new_link(&tag.url).label("Ver documentación"),
            ])]);
    }

    let mut matches = suggestions(&TAGS, query, 5);
    if matches.is_empty() {
        matches = suggestions(&TAGS, "", 5);
    }
    let choices = matches
        .iter()
        .map(|tag| format!("`{}`", tag.name))
        .collect::<Vec<_>>()
        .join(", ");
    response
        .content(format!(
            "No encontré ese tag. Escribe `/tag` y elige un tema del autocompletado.\nSugerencias: {choices}."
        ))
        .ephemeral(true)
}

pub struct Tag;

#[async_trait]
impl Command for Tag {
    fn name(&self) -> &'static str {
        "tag"
    }

    fn register(&self) -> CreateCommand {
        CreateCommand::new(self.name())
            .description("Comparte una respuesta rápida de la documentación de CubicLauncher")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "tema",
                    "Tema de la documentación",
                )
                .required(true)
                .max_length(100)
                .set_autocomplete(true),
            )
    }

    fn autocomplete(&self, command: &CommandInteraction) -> CreateAutocompleteResponse {
        match command.data.autocomplete() {
            Some(option) if option.name == "tema" => autocomplete_response(&TAGS, option.value),
            _ => CreateAutocompleteResponse::new(),
        }
    }

    async fn execute(
        &self,
        _ctx: &Context,
        command: &CommandInteraction,
    ) -> Result<CommandResult, BoxedError> {
        let query = command
            .data
            .options
            .iter()
            .find(|option| option.name == "tema")
            .and_then(|option| option.value.as_str())
            .unwrap_or_default();
        Ok(CommandResult::Message(Box::new(tag_response(query))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn catalog_has_unambiguous_keys_and_valid_discord_content() {
        assert!(!TAGS.is_empty());
        let mut keys = HashMap::new();
        let mut names = HashSet::new();
        for tag in TAGS.iter() {
            assert!(names.insert(&tag.name), "Nombre duplicado: {}", tag.name);
            assert_eq!(normalize(&tag.name), tag.name);
            assert!((1..=100).contains(&tag.name.encode_utf16().count()));
            assert!((1..=256).contains(&tag.title.encode_utf16().count()));
            assert!((1..=4096).contains(&tag.summary.encode_utf16().count()));
            for key in tag.keys() {
                let key = normalize(key);
                assert!(!key.is_empty());
                if let Some(previous) = keys.insert(key.clone(), &tag.name) {
                    assert_eq!(previous, &tag.name, "Alias ambiguo: {key}");
                }
            }
            let url = reqwest::Url::parse(&tag.url).unwrap();
            assert_eq!(url.scheme(), "https");
            assert_eq!(url.host_str(), Some("dev.cubiclauncher.org"));
            assert!(url.path() == "/docs" || url.path().starts_with("/docs/"));
        }
    }

    #[test]
    fn lookup_accepts_aliases_accents_case_and_separators() {
        for query in [
            "theme",
            "TEMAS",
            " Instalar_theme ",
            "Cómo instalar un tema",
        ] {
            assert_eq!(find_tag(&TAGS, query).unwrap().name, "instalar-tema");
        }
        for query in ["CONFIGURACIÓN", "configuracio\u{301}n"] {
            assert_eq!(find_tag(&TAGS, query).unwrap().name, "config");
        }
        assert_eq!(
            find_tag(&TAGS, "crear   theme").unwrap().name,
            "crear-temas"
        );
        assert!(find_tag(&TAGS, "ja").is_none());
        assert!(find_tag(&TAGS, "").is_none());
        assert!(find_tag(&TAGS, "---").is_none());
        assert!(find_tag(&TAGS, "no-existe").is_none());
    }

    #[test]
    fn autocomplete_matches_partial_names_titles_and_aliases_once() {
        assert_eq!(suggestions(&TAGS, "ja", 25)[0].name, "java");
        assert_eq!(suggestions(&TAGS, "JRE", 25)[0].name, "java");
        assert_eq!(suggestions(&TAGS, "configuración", 25)[0].name, "config");
        assert_eq!(
            suggestions(&TAGS, "personalizados", 25)[0].name,
            "crear-temas"
        );
        let matches = suggestions(&TAGS, "theme", 25);
        assert_eq!(matches[0].name, "instalar-tema");
        assert!(matches.iter().any(|tag| tag.name == "crear-temas"));
        let names: HashSet<_> = matches.iter().map(|tag| &tag.name).collect();
        assert_eq!(matches.len(), names.len());
        assert!(suggestions(&TAGS, "no-existe", 25).is_empty());
    }

    #[test]
    fn autocomplete_obeys_discord_limit_and_returns_canonical_values() {
        let tags: Vec<_> = (0..40)
            .map(|index| {
                let mut tag = TAGS[0].clone();
                tag.name = format!("tag-{index}");
                tag
            })
            .collect();
        let response = serde_json::to_value(autocomplete_response(&tags, "")).unwrap();
        let choices = response["choices"].as_array().unwrap();
        assert_eq!(choices.len(), 25);
        assert_eq!(choices[0]["value"], "tag-0");
        for choice in choices {
            assert!(choice["name"].as_str().unwrap().chars().count() <= 100);
        }
        let response = serde_json::to_value(autocomplete_response(&TAGS, "jre")).unwrap();
        assert_eq!(response["choices"][0]["value"], "java");
        let empty = serde_json::to_value(autocomplete_response(&TAGS, "no-existe")).unwrap();
        assert_eq!(empty["choices"], serde_json::json!([]));
    }

    #[test]
    fn successful_response_is_public_with_documentation_button() {
        let response = serde_json::to_value(tag_response("theme")).unwrap();
        let tag = find_tag(&TAGS, "instalar-tema").unwrap();
        assert_eq!(response["embeds"][0]["description"], tag.summary);
        assert_eq!(response["components"][0]["components"][0]["url"], tag.url);
        assert_eq!(response["flags"].as_u64().unwrap_or_default() & 64, 0);
        assert_eq!(response["allowed_mentions"]["parse"], serde_json::json!([]));
    }

    #[test]
    fn unknown_or_missing_tag_is_private_and_suggests_valid_tags() {
        for query in ["no-existe", "", "@everyone"] {
            let response = serde_json::to_value(tag_response(query)).unwrap();
            assert_eq!(response["flags"].as_u64().unwrap() & 64, 64);
            let content = response["content"].as_str().unwrap();
            assert!(content.contains("Sugerencias:"));
            assert!(!content.contains("@everyone"));
        }
        let response = serde_json::to_value(tag_response("ja")).unwrap();
        assert!(response["content"].as_str().unwrap().contains("`java`"));
    }

    #[test]
    fn slash_registration_enables_native_autocomplete() {
        let command = serde_json::to_value(Tag.register()).unwrap();
        assert_eq!(command["name"], "tag");
        assert_eq!(command["options"][0]["name"], "tema");
        assert_eq!(command["options"][0]["type"], 3);
        assert_eq!(command["options"][0]["required"], true);
        assert_eq!(command["options"][0]["autocomplete"], true);
        assert!(command["default_member_permissions"].is_null());
    }
}
