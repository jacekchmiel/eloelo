use anyhow::Result;

pub trait CommandHandler {
    fn supported_commands(&self) -> Vec<CommandDescription>;
    fn dispatch_command(
        &mut self,
        username: &str,
        command: &str,
        args: &[&str],
    ) -> Option<Result<String>>;
}

pub struct CommandDescription {
    pub keyword: String,
    pub description: String,
}

pub fn parse_command(command: &str) -> (&str, Vec<&str>) {
    let mut tokens = command.trim_start_matches('/').split(" ");
    let command_token = tokens.next().unwrap_or_default();
    let arg_tokens = tokens.collect();
    (command_token, arg_tokens)
}
