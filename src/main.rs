extern crate flexi_logger;

use tokio_util::sync::CancellationToken;

mod config;
mod server;
use crate::config::Config;
use crate::server::server_executor;

use flexi_logger::{AdaptiveFormat, Age, Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};

fn main() {
    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default())
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(4),
        )
        .adaptive_format_for_stderr(AdaptiveFormat::Detailed)
        .print_message()
        .duplicate_to_stderr(Duplicate::Warn)
        .start_with_specfile("logspec.toml")
        .unwrap();

    let cfg = Config::get();
    log::info!("cfg: {:#?}", cfg);

    let token = CancellationToken::new();
    let child_token = token.child_token();
    let handle = std::thread::spawn(move || server_executor(cfg, child_token));

    std::thread::sleep(std::time::Duration::from_secs(10));
    token.cancel();

    handle.join().unwrap();
}
