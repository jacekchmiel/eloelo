use anyhow::Result;
use axum::extract::ws::{self, WebSocket};
use axum::extract::{Json, State, WebSocketUpgrade};
use axum::response::{ErrorResponse, Response};
use axum::routing::{any, get, post};
use axum::Router;
use dead_mans_switch::{dead_mans_switch, DeadMansSwitch};
use eloelo::elodisco::EloDisco;
use eloelo::message_bus::{FinishMatch, Message, MessageBus, UiCommand, UiUpdate};
use eloelo::{store, unwrap_or_def_verbose, EloElo};
use eloelo_model::player::{DiscordUsername, Player};
use eloelo_model::{GameId, PlayerId, Team, WinScale};
use futures_util::stream::{StreamExt as _, TryStreamExt as _};
use http::StatusCode;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serenity::futures::SinkExt;
use std::fmt::Display;
use std::sync::Arc;
use std::thread;
use tokio::signal;
use tower_http::services::ServeDir;
mod dead_mans_switch;
mod eloelo;
mod logging;

pub fn print_err(e: &impl Display) {
    error!("{e}")
}

struct AppState {
    message_bus: MessageBus,
}

type AppStateArg = State<Arc<AppState>>;

async fn initialize_ui(State(state): AppStateArg) {
    debug!("initialize_ui");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::InitializeUi));
}

#[derive(Debug, Deserialize)]
struct AddNewPlayer {
    name: String,
    discord_username: Option<DiscordUsername>,
}
async fn add_new_player(State(state): AppStateArg, Json(body): Json<AddNewPlayer>) {
    debug!("add_new_player({:?}", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::AddNewPlayer(Player {
            id: PlayerId::from(body.name),
            display_name: None,
            discord_username: body.discord_username,
            fosiaudio_name: None,
            elo: Default::default(),
        })));
}

#[derive(Debug, Deserialize)]
struct RemovePlayer {
    id: PlayerId,
}
async fn remove_player(State(state): AppStateArg, Json(body): Json<RemovePlayer>) {
    debug!("remove_player({:?})", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::RemovePlayer(body.id)));
}

#[derive(Debug, Deserialize)]
struct AddPlayerToOtherTeam {
    id: PlayerId,
}
async fn move_player_to_other_team(
    State(state): AppStateArg,
    Json(body): Json<AddPlayerToOtherTeam>,
) {
    debug!("move_player_to_other_team({:?})", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::MovePlayerToOtherTeam(
            body.id,
        )));
}

#[derive(Debug, Deserialize)]
struct RemovePlayerFromTeam {
    id: PlayerId,
}
async fn remove_player_from_team(
    State(state): AppStateArg,
    Json(body): Json<RemovePlayerFromTeam>,
) {
    debug!("remove_player_from_team({:?})", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::RemovePlayerFromTeam(body.id)));
}

#[derive(Debug, Deserialize)]
struct AddPlayerToTeam {
    id: PlayerId,
    team: Team,
}
async fn add_player_to_team(State(state): AppStateArg, Json(body): Json<AddPlayerToTeam>) {
    debug!("add_player_to_team({:?})", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::AddPlayerToTeam(
            body.id, body.team,
        )));
}

#[derive(Debug, Deserialize)]
struct ChangeGame {
    id: GameId,
}
async fn change_game(State(state): AppStateArg, Json(body): Json<ChangeGame>) {
    debug!("change_game({:?})", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::ChangeGame(body.id)));
}

async fn start_match(State(state): AppStateArg) {
    debug!("start_match()");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::StartMatch));
}

async fn shuffle_teams(State(state): AppStateArg) {
    debug!("shuffle_teams()");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::ShuffleTeams));
}

async fn refresh_elo(State(state): AppStateArg) {
    debug!("refresh_elo()");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::RefreshElo));
}

#[derive(Debug, Deserialize)]
struct PresentInLobbyChange {
    id: PlayerId,
    present: bool,
}
async fn present_in_lobby_change(
    State(state): AppStateArg,
    Json(body): Json<PresentInLobbyChange>,
) {
    debug!("present_in_lobby_change({body:?}");
    let message = if body.present {
        Message::UiCommand(UiCommand::AddPlayerToLobby(body.id))
    } else {
        Message::UiCommand(UiCommand::RemovePlayerFromLobby(body.id))
    };
    let _ = state.message_bus.send(message);
}

fn bad_request(msg: impl Display) -> ErrorResponse {
    (StatusCode::BAD_REQUEST, msg.to_string()).into()
}

#[derive(Debug, Deserialize)]
struct FinishMatchBody {
    winner: Option<Team>,
    scale: Option<WinScale>,
    duration: Option<std::time::Duration>, //TODO: check if we can send Duration
    fake: Option<bool>,
}
async fn finish_match(
    State(state): AppStateArg,
    Json(body): Json<FinishMatchBody>,
) -> axum::response::Result<()> {
    debug!("finish_match({body:?})");
    let cmd = match body.winner {
        None => UiCommand::FinishMatch(FinishMatch::Cancelled),
        Some(winner) => {
            let duration = body
                .duration
                .ok_or_else(|| bad_request("Missing match duration"))?;
            let scale = body.scale.ok_or_else(|| bad_request("Missing win scale"))?;
            let fake = body.fake.unwrap_or(false);
            UiCommand::FinishMatch(FinishMatch::Finished {
                winner,
                scale,
                duration,
                fake,
            })
        }
    };
    state.message_bus.send(Message::UiCommand(cmd));
    Ok(())
}

async fn call_to_lobby(State(state): AppStateArg) {
    state
        .message_bus
        .send(Message::UiCommand(UiCommand::CallToLobby));
}

// fn start_worker_threads(
//     message_bus: MessageBus,
//     app_handle: AppHandle,
//     mut eloelo: EloElo,
//     dead_man_switch: DeadMansSwitch,
// ) {
//     MessageBus -> UI proxy
//     let mut message_bus_receiver = message_bus.subscribe();
//     thread::spawn({
//         let dead_man_switch = dead_man_switch.clone();
//         move || {
//             let _h = dead_man_switch;
//             info!("Message UI proxy started.");
//             loop {
//                 match message_bus_receiver.blocking_recv() {
//                     Some(Message::UiUpdate(UiUpdate::State(ui_state))) => {
//                         debug!("< update_ui");
//                         app_handle.emit("update_ui", ui_state).unwrap()
//                     }
//                     Some(Message::UiUpdate(UiUpdate::DiscordInfo(discord_info))) => {
//                         debug!("< avatars");
//                         app_handle.emit("discord_info", discord_info).unwrap()
//                     }
//                     Some(Message::UiCommand(UiCommand::CloseApplication)) | None => {
//                         info!("Closing MessageBus UI proxy");
//                         break;
//                     }
//                     Some(_) => {}
//                 }
//             }
//             info!("MessageBus UI proxy stopped.");
//         }
//     });

//     UI command dispatcher
//     let mut message_bus_receiver = message_bus.subscribe();
//     thread::spawn({
//         move || {
//             let _h = dead_man_switch;
//             info!("EloElo worker started.");
//             loop {
//                 match message_bus_receiver.blocking_recv() {
//                     Some(Message::UiCommand(UiCommand::CloseApplication)) => {
//                         eloelo.dispatch_ui_command(UiCommand::CloseApplication);
//                         break;
//                     }
//                     Some(Message::UiCommand(ui_command)) => {
//                         eloelo.dispatch_ui_command(ui_command);
//                         let _ =
//                             message_bus.send(Message::UiUpdate(UiUpdate::State(eloelo.ui_state())));
//                     }
//                     Some(_) => {}
//                     None => {
//                         break;
//                     }
//                 }
//             }
//             info!("EloElo worker stopped.");
//         }
//     });
// }

/// Stub mimicking tauri infrastructure
// struct AppHandle;

// impl AppHandle {
//     pub fn emit(&self, _message: &str, _payload: impl Serialize) -> Result<()> {
//         Ok(())
//     }
// }

async fn create_ui_event_stream(ws: WebSocketUpgrade, State(state): AppStateArg) -> Response {
    ws.on_upgrade(move |socket| ui_event_stream(socket, state.message_bus.clone()))
}

fn wrap_result<T: Serialize, E: Display>(
    r: std::result::Result<T, E>,
) -> std::result::Result<ws::Message, axum::Error> {
    #[derive(Serialize)]
    enum WrappedResult<T> {
        Success { data: T },
        Error { message: String },
    }
    let wrapped_result = match r {
        Ok(data) => WrappedResult::Success { data },
        Err(e) => WrappedResult::Error {
            message: e.to_string(),
        },
    };
    let json_text = serde_json::to_string_pretty(&wrapped_result)
        .unwrap_or_else(|e| format!("{{ \"error\": \"JSON serialization failed: {e}\" }}"));
    Ok(ws::Message::text(json_text))
}

async fn ui_event_stream(socket: WebSocket, message_bus: MessageBus) {
    let stream = message_bus.subscribe().ui_update_stream().map(wrap_result);
    let _ = stream.forward(socket).await.inspect_err(print_err);
}

#[tokio::main]
async fn main() {
    logging::init();
    let config = unwrap_or_def_verbose(store::load_config());
    let state = unwrap_or_def_verbose(store::load_state());
    let bot_state = unwrap_or_def_verbose(store::load_bot_state());
    let message_bus = MessageBus::new();
    // let _elodisco = EloDisco::new(config.clone(), bot_state, message_bus.clone());
    let mut eloelo = EloElo::new(state, config, message_bus.clone());
    let eloelo_task = tokio::spawn({
        let message_bus = message_bus.clone();
        async move {
            let mut ui_command_stream = message_bus.subscribe().ui_command_stream().boxed();
            loop {
                match ui_command_stream.try_next().await {
                    Ok(Some(command @ UiCommand::CloseApplication)) => {
                        eloelo.dispatch_ui_command(command);
                        break;
                    }
                    Ok(Some(command)) => {
                        eloelo.dispatch_ui_command(command);
                    }
                    Ok(None) => {
                        break;
                    }
                    Err(e) => {
                        print_err(&e);
                        break;
                    }
                }
            }
        }
    });

    let shared_state = Arc::new(AppState {
        message_bus: message_bus.clone(),
    });
    let app = Router::new()
        .route("/v1/ui_stream", any(create_ui_event_stream))
        .route("/v1/initialize_ui", get(initialize_ui))
        .route("/v1/add_new_player", post(add_new_player))
        .route("/v1/remove_player", post(remove_player))
        .route(
            "/v1/move_player_to_other_team",
            post(move_player_to_other_team),
        )
        .route("/v1/remove_player_from_team", post(remove_player_from_team))
        .route("/v1/add_player_to_team", post(add_player_to_team))
        .route("/v1/change_game", post(change_game))
        .route("/v1/start_match", post(start_match))
        .route("/v1/finish_match", post(finish_match))
        .route("/v1/shuffle_teams", post(shuffle_teams))
        .route("/v1/refresh_elo", post(refresh_elo))
        .route("/v1/call_to_lobby", post(call_to_lobby))
        .route("/v1/present_in_lobby_change", post(present_in_lobby_change))
        .fallback_service(ServeDir::new("ui/dist")) // FIXME: configurable assets directory?
        .with_state(shared_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tokio::spawn(async {
        axum::serve(listener, app).await // .expect("axum::serve exited with error"),
    });

    info!("Running");
    signal::ctrl_c().await.unwrap();
    info!("ctrl-c received");
    message_bus.send(Message::UiCommand(UiCommand::CloseApplication));

    debug!("Waiting for workers to stop...");
    let _ = eloelo_task.await.inspect_err(print_err);
    debug!("All workers stopped.")
}
