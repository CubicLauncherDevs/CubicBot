# CubicBot

Bot de Discord para CubicLauncher, escrito en Rust con Serenity.

## Configuración

Completa el archivo `.env` de la raíz del proyecto. Si no existe, copia
`.env.example` como `.env`. El bot lo carga automáticamente al arrancar.
Las variables exportadas en la terminal tienen prioridad sobre `.env`.

```dotenv
DISCORD_TOKEN=token_del_bot
GUILD_ID=id_del_servidor
WELCOME_CHANNEL_ID=id_del_canal_de_bienvenida
THEMES_CHANNEL_ID=id_del_canal_de_temas
THEMES_POLL_SECONDS=300
```

Ejecuta desde la raíz:

```bash
cargo run
```

Reinicia el bot después de modificar `.env`. El archivo está excluido de Git.
Para obtener IDs, activa el modo desarrollador de Discord y utiliza «Copiar ID».
Instala el bot con los scopes `bot` y `applications.commands`. Activa también
**Server Members Intent** en el Developer Portal, usado por las bienvenidas.

Los comandos se registran en el servidor indicado por `GUILD_ID`.

## Respuestas rápidas: `/tag`

Todos los miembros pueden usar `/tag tema:…` para publicar una respuesta breve
de la documentación en el mismo canal, con un botón **Ver documentación**.

```text
/tag tema:java
/tag tema:instalar-tema
/tag tema:mods
/tag tema:crear-temas
```

El argumento `tema` tiene autocompletado nativo: al escribir `ja`, Discord sugiere
Java; sin escribir nada, muestra los tags disponibles (máximo 25 sugerencias).
Busca por nombre, título y alias, sin distinguir mayúsculas ni tildes; espacios,
guiones y guiones bajos funcionan como separadores equivalentes. Por ejemplo,
`theme` y `temas` resuelven a `instalar-tema`.

Al enviar el comando, se requiere un nombre, título o alias completo. Un valor
desconocido muestra un aviso con sugerencias visible solo para quien lo ejecutó.

Los resúmenes están en `Assets/tags.json`, con los campos `name`, `title`,
`aliases`, `summary` (Markdown) y `url`. Para añadir o actualizar una respuesta,
edita el catálogo, ejecuta `cargo test` y recompila/reinicia el bot con `cargo run`.
Los nombres y alias deben identificar un único tag. El catálogo se incluye en
el ejecutable: no necesita consultar la web para responder ni se actualiza
automáticamente al cambiar la documentación.

Después de actualizar el bot, reinícialo para registrar `/tag` en `GUILD_ID`.

## Avisos de temas

El bot consulta el [catálogo oficial de GitHub](https://github.com/CubicLauncherDevs/Themes/blob/master/themes.json)
cada 5 minutos (configurable con `THEMES_POLL_SECONDS`, mínimo 30).
Cuando aparece un ID de tema nuevo, envía un embed a `THEMES_CHANNEL_ID`
con nombre, autor, versión, descripción, imagen disponible y enlaces a GitHub
y al catálogo web. Las nuevas versiones de un tema conocido no generan avisos.

El bot necesita **Ver canal**, **Enviar mensajes** e **Insertar enlaces** en el
canal de destino. Si `THEMES_CHANNEL_ID` está vacío, el seguimiento se desactiva.

- La primera consulta correcta guarda los temas existentes sin anunciarlos.
- El estado se guarda por canal en `data/themes-<ID>.json`. Conserva la carpeta
  `data/` entre reinicios y despliegues para mantener el historial.
- Al reiniciar, anuncia los temas nuevos que aparecieron mientras estaba apagado.
- Los errores de red o de envío se registran y se reintentan en la siguiente consulta.
- El estado se guarda tras cada envío mediante un archivo temporal y renombrado.
  Un cierre abrupto entre el envío y su guardado puede ocasionar un anuncio repetido.
- Ejecuta una sola instancia del bot con este estado. No borres el archivo para
  reintentar errores: borrarlo establece una nueva referencia sin anuncios antiguos.
- Si el estado está dañado o no se puede leer, el seguimiento se detiene y lo indica
  en la consola; restaura el archivo y reinicia el bot.

### `/ultimotema`

Envía manualmente el tema publicado más recientemente al canal configurado,
aunque ya se haya anunciado. Requiere **Administrar servidor**. La confirmación
y los errores son visibles solo para quien ejecutó el comando.

Se compara la fecha más antigua de las versiones de cada tema, interpretando
su zona horaria; no se considera nueva publicación la actualización de un tema antiguo.
El envío manual no modifica el historial de los avisos automáticos.

## Bienvenidas

Configura `WELCOME_CHANNEL_ID` en `.env` con el ID del canal donde se enviará
la imagen de bienvenida al entrar un miembro. Si falta o está vacío, no se envían
mensajes de bienvenida. Reinicia el bot después de cambiarlo.

El bot necesita **Ver canal**, **Enviar mensajes**, **Insertar enlaces** y
**Adjuntar archivos** en ese canal, además de **Server Members Intent** activado.

## Tickets

Para utilizar los tickets, completa también `TICKET_CATEGORY_ID`, `STAFF_ROLE_ID`,
`TICKET_LOG_CHANNEL_ID` y `TICKET_PANEL_CHANNEL_ID` en `.env`.

## Verificación

```bash
cargo check
cargo test
```
