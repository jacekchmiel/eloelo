use log::{error, info};
use serenity::all::{CacheHttp, CreateMessage, User};

pub struct DirectMessenger<C> {
    ctx: C,
    user: User,
}

impl<C: CacheHttp> DirectMessenger<C> {
    pub fn new(ctx: C, user: User) -> Self {
        Self { ctx, user }
    }

    pub async fn send_dm(self, message: CreateMessage, message_log_label: &str) {
        send_direct_message(self.ctx, self.user, message, message_log_label).await
    }
}

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
