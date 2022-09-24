extern crate flexi_logger;

use tokio_util::sync::CancellationToken;

mod config;
mod server;
use crate::config::Config;
use crate::server::spawn_socks5_server;

use flexi_logger::{AdaptiveFormat, Age, Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};

#[tokio::main]
async fn main() {
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

    let cfg = tokio::task::spawn_blocking(Config::get)
        .await
        .expect("get config");
    log::info!("cfg: {:#?}", cfg);

    let token = CancellationToken::new();
    let child_token = token.child_token();

    let (r, _) = tokio::join!(
        spawn_socks5_server(cfg, child_token),
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            token.cancel();
        })
    );

    r.unwrap();
}
