use flexi_logger::{Duplicate, FileSpec, Logger, WriteMode};

use super::store::data_dir;

pub fn init() {
    Logger::try_with_str("warn,eloelo_lib=debug,eloelo_model=debug,spawelo=debug")
        .expect("log config text")
        .log_to_file(FileSpec::default().directory(data_dir().join("logs")))
        .write_mode(WriteMode::BufferAndFlush)
        .duplicate_to_stderr(Duplicate::Debug) // print warnings and errors also to the console
        .start()
        .expect("log init");
}
