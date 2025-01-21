use log::error;
use serenity::all::{CacheHttp, CreateMessage, User};

pub async fn send_direct_message(ctx: impl CacheHttp, user: User, message: CreateMessage) {
    let _ = user
        .dm(&ctx, message)
        .await
        .inspect_err(|e| error!("{e:#}"));
}
