use std::thread;

use anyhow::Error;
use dead_mans_switch::{dead_mans_switch, DeadMansSwitch};
use eloelo::elodisco::EloDisco;
use eloelo::message_bus::{FinishMatch, Message, MessageBus, UiCommand, UiUpdate};
use eloelo::{store, EloElo};
use eloelo_model::player::Player;
use eloelo_model::{GameId, PlayerId, Team, WinScale};
use log::{debug, info};
use tauri::ipc::InvokeError;
use tauri::{AppHandle, Emitter, Manager, State};

mod dead_mans_switch;
mod eloelo;
mod logging;

struct TauriStateInner {
    message_bus: MessageBus,
}

type TauriState<'r> = State<'r, TauriStateInner>;

#[tauri::command]
fn initialize_ui(state: TauriState) {
    debug!("initialize_ui");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::InitializeUi));
}

#[tauri::command]
fn add_new_player(name: String, state: TauriState) {
    debug!("add_new_player({:?})", name);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::AddNewPlayer(Player::from(
            name,
        ))));
}

#[tauri::command]
fn remove_player(name: String, state: TauriState) {
    debug!("remove_player({:?})", name);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::RemovePlayer(PlayerId::from(
            name,
        ))));
}

#[tauri::command]
fn move_player_to_other_team(name: String, state: TauriState) {
    debug!("move_player_to_other_team({:?})", name);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::MovePlayerToOtherTeam(name)));
}

#[tauri::command]
fn remove_player_from_team(name: String, state: TauriState) {
    debug!("remove_player_from_team({:?})", name);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::RemovePlayerFromTeam(name)));
}

#[tauri::command]
fn add_player_to_team(name: String, team: String, state: TauriState) {
    debug!("add_player_to_team({:?})", name);
    let team = Team::from_str(&team).expect("String with valid team value");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::AddPlayerToTeam(name, team)));
}

#[tauri::command]
fn change_game(name: String, state: TauriState) {
    debug!("change_game({:?})", name);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::ChangeGame(GameId::from(
            name,
        ))));
}

#[tauri::command]
fn start_match(state: TauriState) {
    debug!("start_match()");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::StartMatch));
}

#[tauri::command]
fn shuffle_teams(state: TauriState) {
    debug!("shuffle_teams()");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::ShuffleTeams));
}

#[tauri::command]
fn refresh_elo(state: TauriState) {
    debug!("refresh_elo()");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::RefreshElo));
}

fn error(msg: impl std::fmt::Display + std::fmt::Debug + Send + Sync + 'static) -> InvokeError {
    InvokeError::from_anyhow(Error::msg(msg))
}

#[tauri::command]
fn finish_match(
    state: TauriState,
    winner: Option<String>,
    scale: Option<String>,
) -> Result<(), InvokeError> {
    debug!("finish_match({:?})", winner);
    let winner = match winner {
        Some(winner) => {
            Some(Team::from_str(&winner).ok_or_else(|| error("Invalid team designator"))?)
        }
        None => None,
    };
    let scale = scale
        .map(|s| WinScale::try_from(s.as_str()).map_err(InvokeError::from))
        .transpose()?;
    state
        .message_bus
        .send(Message::UiCommand(UiCommand::FinishMatch(FinishMatch {
            winner,
            scale,
        })));
    Ok(())
}

fn start_worker_threads(
    message_bus: MessageBus,
    app_handle: AppHandle,
    mut eloelo: EloElo,
    dead_man_switch: DeadMansSwitch,
) {
    // MessageBus -> UI proxy
    let mut message_bus_receiver = message_bus.subscribe();
    thread::spawn({
        let dead_man_switch = dead_man_switch.clone();
        move || {
            let _h = dead_man_switch;
            info!("Message UI proxy started.");
            loop {
                match message_bus_receiver.blocking_recv() {
                    Some(Message::UiUpdate(UiUpdate::State(ui_state))) => {
                        debug!("< update_ui");
                        app_handle.emit("update_ui", ui_state).unwrap()
                    }
                    Some(Message::UiUpdate(UiUpdate::Avatars(avatars))) => {
                        debug!("< avatars");
                        app_handle.emit("avatars", avatars).unwrap()
                    }
                    Some(Message::UiCommand(UiCommand::CloseApplication)) | None => {
                        info!("Closing MessageBus UI proxy");
                        break;
                    }
                    Some(_) => {}
                }
            }
            info!("MessageBus UI proxy stopped.");
        }
    });

    // UI command dispatcher
    let mut message_bus_receiver = message_bus.subscribe();
    thread::spawn({
        move || {
            let _h = dead_man_switch;
            info!("EloElo worker started.");
            loop {
                match message_bus_receiver.blocking_recv() {
                    Some(Message::UiCommand(UiCommand::CloseApplication)) => {
                        eloelo.dispatch_ui_command(UiCommand::CloseApplication);
                        break;
                    }
                    Some(Message::UiCommand(ui_command)) => {
                        eloelo.dispatch_ui_command(ui_command);
                        let _ =
                            message_bus.send(Message::UiUpdate(UiUpdate::State(eloelo.ui_state())));
                    }
                    Some(_) => {}
                    None => {
                        break;
                    }
                }
            }
            info!("EloElo worker stopped.");
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::init();
    let config = store::load_config().unwrap();
    let state = store::load_state(&config).unwrap();
    let bot_state = store::load_bot_state().unwrap();
    let history = store::load_history().unwrap();
    let message_bus = MessageBus::new();
    let (dead_man_switch, dead_man_observer) = dead_mans_switch();
    let _elodisco = EloDisco::new(config.clone(), bot_state, message_bus.clone());
    let eloelo = EloElo::new(state, history, config, message_bus.clone());
    tauri::Builder::default()
        .setup({
            let message_bus = message_bus.clone();
            move |app| {
                let app_handle = app.handle().clone();
                start_worker_threads(message_bus.clone(), app_handle, eloelo, dead_man_switch);
                Ok(())
            }
        })
        .plugin(tauri_plugin_shell::init())
        .manage(TauriStateInner { message_bus })
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                window
                    .state::<TauriStateInner>()
                    .message_bus
                    .send(Message::UiCommand(UiCommand::CloseApplication));
                debug!("Waiting for workers to stop...");
                dead_man_observer.wait_all_dead();
                debug!("All workers stopped.")
            }
        })
        .invoke_handler(tauri::generate_handler![
            initialize_ui,
            add_new_player,
            remove_player,
            remove_player_from_team,
            move_player_to_other_team,
            add_player_to_team,
            change_game,
            start_match,
            shuffle_teams,
            finish_match,
            refresh_elo,
        ])
        // .plugin(
        //     tauri_plugin_log::Builder::default()
        //         .targets([
        //             tauri_plugin_log::Target::new(TargetKind::LogDir {
        //                 file_name: Some(String::from("eloelo")),
        //             }),
        //             tauri_plugin_log::Target::new(TargetKind::Stderr),
        //             // tauri_plugin_log::Target::new(TargetKind::Webview),
        //         ])
        //         .level(LevelFilter::Debug)
        //         .level_for("serenity", LevelFilter::Warn)
        //         .level_for("h2", LevelFilter::Warn)
        //         .level_for("tracing", LevelFilter::Warn)
        //         .build(),
        // )
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
