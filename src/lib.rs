use anyhow::Result;
use dead_mans_switch::{dead_mans_switch, DeadMansSwitch};
use eloelo::elodisco::EloDisco;
use eloelo::message_bus::{Message, MessageBus, UiCommand, UiUpdate};
use eloelo::{store, unwrap_or_def_verbose, EloElo};
use log::{debug, error, info};
use serde::Serialize;
use std::fmt::Display;
use std::thread;
use std::time::Duration;

mod dead_mans_switch;
mod eloelo;
mod logging;

pub fn print_err(e: &impl Display) {
    error!("{e}")
}

// struct TauriStateInner {
//     message_bus: MessageBus,
// }

// type TauriState<'r> = State<'r, TauriStateInner>;

// #[tauri::command]
// fn initialize_ui(state: TauriState) {
//     debug!("initialize_ui");
//     let _ = state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::InitializeUi));
// }

// #[tauri::command]
// fn add_new_player(name: String, discord_username: Option<String>, state: TauriState) {
//     debug!("add_new_player({:?}, {:?})", name, discord_username);
//     let _ = state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::AddNewPlayer(Player {
//             id: PlayerId::from(name),
//             display_name: None,
//             discord_username: discord_username.map(DiscordUsername::from),
//             fosiaudio_name: None,
//             elo: Default::default(),
//         })));
// }

// #[tauri::command]
// fn remove_player(id: String, state: TauriState) {
//     debug!("remove_player({:?})", id);
//     let _ = state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::RemovePlayer(PlayerId::from(
//             id,
//         ))));
// }

// #[tauri::command]
// fn move_player_to_other_team(id: String, state: TauriState) {
//     debug!("move_player_to_other_team({:?})", id);
//     let _ = state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::MovePlayerToOtherTeam(
//             PlayerId::from(id),
//         )));
// }

// #[tauri::command]
// fn remove_player_from_team(id: String, state: TauriState) {
//     debug!("remove_player_from_team({:?})", id);
//     let _ = state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::RemovePlayerFromTeam(
//             PlayerId::from(id),
//         )));
// }

// #[tauri::command]
// fn add_player_to_team(id: String, team: String, state: TauriState) {
//     debug!("add_player_to_team({:?})", id);
//     let team = Team::from_str(&team).expect("String with valid team value");
//     let _ = state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::AddPlayerToTeam(
//             PlayerId::from(id),
//             team,
//         )));
// }

// #[tauri::command]
// fn change_game(name: String, state: TauriState) {
//     debug!("change_game({:?})", name);
//     let _ = state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::ChangeGame(GameId::from(
//             name,
//         ))));
// }

// #[tauri::command]
// fn start_match(state: TauriState) {
//     debug!("start_match()");
//     let _ = state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::StartMatch));
// }

// #[tauri::command]
// fn shuffle_teams(state: TauriState) {
//     debug!("shuffle_teams()");
//     let _ = state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::ShuffleTeams));
// }

// #[tauri::command]
// fn refresh_elo(state: TauriState) {
//     debug!("refresh_elo()");
//     let _ = state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::RefreshElo));
// }

// #[tauri::command]
// fn present_in_lobby_change(state: TauriState, id: PlayerId, present: bool) {
//     debug!("present_in_lobby_change({id}, {present})");
//     let message = if present {
//         Message::UiCommand(UiCommand::AddPlayerToLobby(id))
//     } else {
//         Message::UiCommand(UiCommand::RemovePlayerFromLobby(id))
//     };
//     let _ = state.message_bus.send(message);
// }

// fn invoke_error(
//     msg: impl std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
// ) -> InvokeError {
//     InvokeError::from_anyhow(Error::msg(msg))
// }

// #[tauri::command]
// fn finish_match(
//     state: TauriState,
//     winner: Option<String>,
//     scale: Option<String>,
//     duration: Option<std::time::Duration>, //TODO: check if we can send Duration
//     fake: Option<bool>,
// ) -> Result<(), InvokeError> {
//     debug!("finish_match({winner:?}, {scale:?}, {duration:?})");
//     let cmd = match winner {
//         None => UiCommand::FinishMatch(FinishMatch::Cancelled),
//         Some(winner) => {
//             let winner =
//                 Team::from_str(&winner).ok_or_else(|| invoke_error("Invalid team designator"))?;
//             let scale = WinScale::try_from(scale.ok_or_else(|| invoke_error("Missing win scale"))?)
//                 .map_err(InvokeError::from_error)?;
//             let duration = duration.ok_or_else(|| invoke_error("Missing match duration"))?;
//             let fake = fake.unwrap_or(false);
//             UiCommand::FinishMatch(FinishMatch::Finished {
//                 winner,
//                 scale,
//                 duration,
//                 fake,
//             })
//         }
//     };
//     state.message_bus.send(Message::UiCommand(cmd));
//     Ok(())
// }

// #[tauri::command]
// fn call_to_lobby(state: TauriState) -> Result<(), InvokeError> {
//     state
//         .message_bus
//         .send(Message::UiCommand(UiCommand::CallToLobby));
//     Ok(())
// }

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
                    Some(Message::UiUpdate(UiUpdate::DiscordInfo(discord_info))) => {
                        debug!("< avatars");
                        app_handle.emit("discord_info", discord_info).unwrap()
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

/// Stub mimicking tauri infrastructure
struct AppHandle;

impl AppHandle {
    pub fn emit(&self, _message: &str, _payload: impl Serialize) -> Result<()> {
        todo!()
    }
}

pub fn run() {
    logging::init();
    let config = unwrap_or_def_verbose(store::load_config());
    let state = unwrap_or_def_verbose(store::load_state());
    let bot_state = unwrap_or_def_verbose(store::load_bot_state());
    let message_bus = MessageBus::new();
    let (dead_man_switch, _dead_man_observer) = dead_mans_switch();
    let _elodisco = EloDisco::new(config.clone(), bot_state, message_bus.clone());
    let eloelo = EloElo::new(state, config, message_bus.clone());
    let app_handle = AppHandle;

    start_worker_threads(message_bus.clone(), app_handle, eloelo, dead_man_switch);

    loop {
        thread::sleep(Duration::from_millis(10));
    }
    // TODO: Handle graceful exit (SIGTERM?)
    // if let tauri::WindowEvent::CloseRequested { .. } = event {
    //     window
    //         .state::<TauriStateInner>()
    //         .message_bus
    //         .send(Message::UiCommand(UiCommand::CloseApplication));
    //     debug!("Waiting for workers to stop...");
    //     dead_man_observer.wait_all_dead();
    //     debug!("All workers stopped.")
    // }

    // tauri::Builder::default()
    //     .setup({
    //         let message_bus = message_bus.clone();
    //         move |app| {
    //             let app_handle = app.handle().clone();
    //             start_worker_threads(message_bus.clone(), app_handle, eloelo, dead_man_switch);
    //             Ok(())
    //         }
    //     })
    //     .plugin(tauri_plugin_shell::init())
    //     .manage(TauriStateInner { message_bus })
    //     .on_window_event(move |window, event| {
    //         if let tauri::WindowEvent::CloseRequested { .. } = event {
    //             window
    //                 .state::<TauriStateInner>()
    //                 .message_bus
    //                 .send(Message::UiCommand(UiCommand::CloseApplication));
    //             debug!("Waiting for workers to stop...");
    //             dead_man_observer.wait_all_dead();
    //             debug!("All workers stopped.")
    //         }
    //     })
    //     .invoke_handler(tauri::generate_handler![
    //         initialize_ui,
    //         add_new_player,
    //         remove_player,
    //         remove_player_from_team,
    //         move_player_to_other_team,
    //         add_player_to_team,
    //         change_game,
    //         call_to_lobby,
    //         start_match,
    //         shuffle_teams,
    //         finish_match,
    //         refresh_elo,
    //         present_in_lobby_change,
    //     ])
    //     // .plugin(
    //     //     tauri_plugin_log::Builder::default()
    //     //         .targets([
    //     //             tauri_plugin_log::Target::new(TargetKind::LogDir {
    //     //                 file_name: Some(String::from("eloelo")),
    //     //             }),
    //     //             tauri_plugin_log::Target::new(TargetKind::Stderr),
    //     //             // tauri_plugin_log::Target::new(TargetKind::Webview),
    //     //         ])
    //     //         .level(LevelFilter::Debug)
    //     //         .level_for("serenity", LevelFilter::Warn)
    //     //         .level_for("h2", LevelFilter::Warn)
    //     //         .level_for("tracing", LevelFilter::Warn)
    //     //         .build(),
    //     // )
    //     .run(tauri::generate_context!())
    //     .expect("error while running tauri application");
}
