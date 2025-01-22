use log::{error, info};
use serenity::all::{CacheHttp, CreateMessage, User};

pub async fn send_direct_message(
    ctx: impl CacheHttp,
    user: User,
    message: CreateMessage,
    message_log_label: &str,
) {
    info!(
        "DISCORD DM: {} -> {}",
        message_log_label,
        user.display_name()
    );
    let _ = user
        .dm(&ctx, message)
        .await
        .inspect_err(|e| error!("{e:#}"));
}
