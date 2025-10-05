use std::io::Write;
use std::path::{Path, PathBuf};

use crate::utils::ResultExt as _;

use super::config::Config;
use super::message_bus::{Event, ImageFormat, MessageBusSubscription, UiCommand};
use super::{Message, MessageBus};
use anyhow::{bail, Context as _, Result};
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use log::{debug, error, info};
use serde::Deserialize;
use tokio::task::JoinHandle;

#[derive(Deserialize)]
#[serde(tag = "type")]
enum OcrOutput {
    #[serde(alias = "arcade_lobby")]
    #[serde(alias = "regular_lobby")]
    Lobby { players: Vec<String> },
}

pub fn spawn_dota_screenshot_parser(
    config: Config,
    message_bus: MessageBus,
) -> Result<JoinHandle<()>> {
    let parse_screenshot = dota_screenshot_parse_fn(config)?;

    let join_handle = tokio::spawn(async move {
        screenshot_stream(message_bus.subscribe())
            .map(|(data, image_format)| {
                parse_screenshot(&data, image_format)
                    .inspect_err(|e| error!("{e:#}"))
                    .ok()
            })
            .filter_map(|x| async { x })
            .map(|players| Message::UiCommand(UiCommand::AddLobbyScreenshotData(players)))
            .for_each(|message| {
                let message_bus = message_bus.clone();
                async move { message_bus.send(message) }
            })
            .await;
    });
    Ok(join_handle)
}

fn screenshot_stream(
    sub: MessageBusSubscription,
) -> impl Stream<Item = (Bytes, Option<ImageFormat>)> {
    sub.event_stream().filter_map(|e| async {
        match e {
            Ok(Event::DotaScreenshotReceived(data, image_format)) => Some((data, image_format)),
            Err(e) => {
                error!("[ocr] Message bus error: {e}");
                None
            }
            _ => None,
        }
    })
}

fn dota_screenshot_parse_fn(
    config: Config,
) -> Result<impl Fn(&Bytes, Option<ImageFormat>) -> Result<Vec<String>>> {
    let Some(storage_dir) = config.dota_screenshot_dir.clone() else {
        bail!("Dota screenshot dir missing in config");
    };
    let _ = std::fs::create_dir_all(&storage_dir)
        .with_context(|| format!("create dir {}", storage_dir.to_string_lossy()))
        .print_err_info();
    let storage_dir: PathBuf = storage_dir
        .canonicalize()
        .context("storage_dir is invalid")?;

    info!(
        "Storing incoming dota screenshots in {}",
        storage_dir.to_string_lossy()
    );

    let parse_screenshot = move |bytes: &Bytes, image_format: Option<ImageFormat>| {
        let filename = storage_dir.join(make_unique_filename(image_format));
        write(&filename, bytes).context("Screenshot write failed")?;
        let players = execute_ocr(&filename, &config)?;
        Ok(players)
    };
    Ok(parse_screenshot)
}

fn execute_ocr(filename: &Path, config: &Config) -> Result<Vec<String>> {
    let command = make_ocr_command(filename, &config.dota_ocr_engine_command);
    let pwd = config
        .dota_ocr_engine_pwd
        .clone()
        .unwrap_or(PathBuf::from("."));
    debug!("command = {command} pwd = {}", pwd.to_string_lossy());
    let raw_ocr_output = duct::cmd!("bash", "-c", command).dir(pwd).read()?;
    debug!("raw_ocr_output = {raw_ocr_output}");
    let parsed_output: OcrOutput =
        serde_json::from_str(&raw_ocr_output).context("Failed to parse ocr output")?;
    let OcrOutput::Lobby { players } = parsed_output;
    Ok(players)
}

fn make_ocr_command(filename: &Path, template: &str) -> String {
    template.replace("%", &filename.to_string_lossy())
}

fn write(filename: &Path, bytes: &Bytes) -> Result<()> {
    let mut file = std::fs::File::create(filename)?;
    file.write(&bytes)?;
    Ok(())
}

fn make_unique_filename(image_format: Option<ImageFormat>) -> String {
    let basename = chrono::offset::Local::now()
        .format("%Y%m%d_%H%M%S_%f")
        .to_string();
    match image_format.as_ref().map(ImageFormat::to_str) {
        Some(ext) => format!("{basename}.{}", ext),
        None => basename,
    }
}
