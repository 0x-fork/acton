use teloxide::{
    Bot,
    repls::CommandReplExt,
    requests::{Requester, ResponseResult},
    types::Message,
    utils::command::BotCommands,
};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
enum Command {
    #[command(description = "say hello")]
    Start,
    #[command(description = "show help")]
    Help,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = std::env::var("TELOXIDE_TOKEN")
        .map_err(|_| "Set TELOXIDE_TOKEN to the Telegram bot token")?;
    let bot = Bot::new(token);

    Command::repl(bot, answer).await;

    Ok(())
}

async fn answer(bot: Bot, message: Message, command: Command) -> ResponseResult<()> {
    let text = match command {
        Command::Start => "Hello, world!".to_owned(),
        Command::Help => Command::descriptions().to_string(),
    };

    bot.send_message(message.chat.id, text).await?;

    Ok(())
}
