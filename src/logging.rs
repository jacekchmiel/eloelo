use flexi_logger::{Duplicate, FileSpec, Logger, LoggerHandle, WriteMode};
use log::error;

use super::store::data_dir;

pub fn init() -> LoggerHandle {
    let logger = Logger::try_with_str(
        "warn,eloelo=debug,eloelo_lib=debug,eloelo_model=debug,spawelo=debug,history=debug",
    )
    .expect("log config text")
    .log_to_file(FileSpec::default().directory(data_dir().join("logs")))
    .write_mode(WriteMode::BufferAndFlush)
    .duplicate_to_stderr(Duplicate::Debug) // print warnings and errors also to the console
    .start()
    .expect("log init");

    let orig_hook = std::panic::take_hook();
    let logger_for_panic = logger.clone();
    std::panic::set_hook(Box::new(move |panic_info| {
        // log, invoke default handler and exit the process
        // we don't want ui hanging without working backend thread
        error!("Panic: {panic_info}");
        logger_for_panic.flush();
        orig_hook(panic_info);
        std::process::exit(1);
    }));
    logger
}
