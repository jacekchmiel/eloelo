use anyhow::{format_err, Context, Result};
use axum::extract::ws::{self, WebSocket};
use axum::extract::{Json, State, WebSocketUpgrade};
use axum::response::{ErrorResponse, IntoResponse, Response};
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
use serenity::futures;
use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
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

#[derive(Serialize)]
struct EmptyResponse;

impl IntoResponse for EmptyResponse {
    fn into_response(self) -> Response {
        serde_json::to_string(&EmptyResponse)
            .unwrap()
            .into_response()
    }
}

async fn initialize_ui(State(state): AppStateArg) -> impl IntoResponse {
    debug!("initialize_ui");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::InitializeUi));
    EmptyResponse
}

#[derive(Debug, Deserialize)]
struct AddNewPlayer {
    name: String,
    discord_username: Option<DiscordUsername>,
}
async fn add_new_player(
    State(state): AppStateArg,
    Json(body): Json<AddNewPlayer>,
) -> impl IntoResponse {
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
    EmptyResponse
}

#[derive(Debug, Deserialize)]
struct RemovePlayer {
    id: PlayerId,
}
async fn remove_player(
    State(state): AppStateArg,
    Json(body): Json<RemovePlayer>,
) -> impl IntoResponse {
    debug!("remove_player({:?})", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::RemovePlayer(body.id)));
    EmptyResponse
}

#[derive(Debug, Deserialize)]
struct AddPlayerToOtherTeam {
    id: PlayerId,
}
async fn move_player_to_other_team(
    State(state): AppStateArg,
    Json(body): Json<AddPlayerToOtherTeam>,
) -> impl IntoResponse {
    debug!("move_player_to_other_team({:?})", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::MovePlayerToOtherTeam(
            body.id,
        )));
    EmptyResponse
}

#[derive(Debug, Deserialize)]
struct RemovePlayerFromTeam {
    id: PlayerId,
}
async fn remove_player_from_team(
    State(state): AppStateArg,
    Json(body): Json<RemovePlayerFromTeam>,
) -> impl IntoResponse {
    debug!("remove_player_from_team({:?})", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::RemovePlayerFromTeam(body.id)));
    EmptyResponse
}

#[derive(Debug, Deserialize)]
struct AddPlayerToTeam {
    id: PlayerId,
    team: Team,
}
async fn add_player_to_team(
    State(state): AppStateArg,
    Json(body): Json<AddPlayerToTeam>,
) -> impl IntoResponse {
    debug!("add_player_to_team({:?})", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::AddPlayerToTeam(
            body.id, body.team,
        )));
    EmptyResponse
}

#[derive(Debug, Deserialize)]
struct ChangeGame {
    id: GameId,
}
async fn change_game(State(state): AppStateArg, Json(body): Json<ChangeGame>) -> impl IntoResponse {
    debug!("change_game({:?})", body);
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::ChangeGame(body.id)));
    EmptyResponse
}

async fn start_match(State(state): AppStateArg) -> impl IntoResponse {
    debug!("start_match()");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::StartMatch));
    EmptyResponse
}

async fn shuffle_teams(State(state): AppStateArg) -> impl IntoResponse {
    debug!("shuffle_teams()");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::ShuffleTeams));
    EmptyResponse
}

async fn refresh_elo(State(state): AppStateArg) -> impl IntoResponse {
    debug!("refresh_elo()");
    let _ = state
        .message_bus
        .send(Message::UiCommand(UiCommand::RefreshElo));
    EmptyResponse
}

#[derive(Debug, Deserialize)]
struct PresentInLobbyChange {
    id: PlayerId,
    present: bool,
}
async fn present_in_lobby_change(
    State(state): AppStateArg,
    Json(body): Json<PresentInLobbyChange>,
) -> impl IntoResponse {
    debug!("present_in_lobby_change({body:?}");
    let message = if body.present {
        Message::UiCommand(UiCommand::AddPlayerToLobby(body.id))
    } else {
        Message::UiCommand(UiCommand::RemovePlayerFromLobby(body.id))
    };
    let _ = state.message_bus.send(message);
    EmptyResponse
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
) -> axum::response::Result<impl IntoResponse> {
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
    Ok(EmptyResponse)
}

async fn call_to_lobby(State(state): AppStateArg) -> impl IntoResponse {
    state
        .message_bus
        .send(Message::UiCommand(UiCommand::CallToLobby));
    EmptyResponse
}

async fn create_ui_event_stream(ws: WebSocketUpgrade, State(state): AppStateArg) -> Response {
    ws.on_upgrade(move |socket| ui_event_stream(socket, state.message_bus.clone()))
}

fn wrap_result<T: Serialize, E: Display>(
    r: std::result::Result<T, E>,
) -> std::result::Result<ws::Message, axum::Error> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    enum WrappedResult<T> {
        Success(T),
        Error(String),
    }
    let wrapped_result = match r {
        Ok(data) => WrappedResult::Success(data),
        Err(e) => WrappedResult::Error(e.to_string()),
    };
    let json_text = serde_json::to_string_pretty(&wrapped_result)
        .unwrap_or_else(|e| format!("{{ \"error\": \"JSON serialization failed: {e}\" }}"));
    Ok(ws::Message::text(json_text))
}

async fn ui_event_stream(socket: WebSocket, message_bus: MessageBus) {
    debug!("ui_event_stream");
    let stream = message_bus.subscribe().ui_update_stream().map(wrap_result);
    let _ = stream.forward(socket).await.inspect_err(print_err);
}

async fn terminate_on_signal() -> Result<()> {
    let interrupt_signal: Pin<Box<dyn Future<Output = _>>> = Box::pin(async {
        signal::unix::signal(signal::unix::SignalKind::interrupt())
            .context("Failed to register terminate signal handlers!")?
            .recv()
            .await;
        Ok(())
    });
    let terminate_signal = Box::pin(async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .context("Failed to register terminate signal handlers!")?
            .recv()
            .await;
        Ok(())
    });
    futures::future::select_all([interrupt_signal, terminate_signal])
        .await
        .0
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
    let eloelo_task = tokio::spawn(eloelo.dispatch_ui_commands(message_bus.clone()));

    let shared_state = Arc::new(AppState {
        message_bus: message_bus.clone(),
    });
    let app = Router::new()
        .nest(
            "/ui/api/v1",
            Router::new()
                .route("/ui_stream", any(create_ui_event_stream))
                .route("/initialize_ui", post(initialize_ui))
                .route("/remove_player", post(remove_player))
                .route("/add_new_player", post(add_new_player))
                .route(
                    "/move_player_to_other_team",
                    post(move_player_to_other_team),
                )
                .route("/remove_player_from_team", post(remove_player_from_team))
                .route("/add_player_to_team", post(add_player_to_team))
                .route("/change_game", post(change_game))
                .route("/start_match", post(start_match))
                .route("/finish_match", post(finish_match))
                .route("/shuffle_teams", post(shuffle_teams))
                .route("/refresh_elo", post(refresh_elo))
                .route("/call_to_lobby", post(call_to_lobby))
                .route("/present_in_lobby_change", post(present_in_lobby_change))
                .with_state(shared_state),
        )
        .fallback_service(ServeDir::new("ui/dist"));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tokio::spawn(async { axum::serve(listener, app).await });

    info!("Running");
    let _ = terminate_on_signal().await.inspect_err(print_err);
    info!("Terminating.");
    message_bus.send(Message::UiCommand(UiCommand::CloseApplication));

    debug!("Waiting for workers to stop...");
    let _ = eloelo_task.await.inspect_err(print_err);
    debug!("All workers stopped.")
}
