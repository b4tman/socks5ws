extern crate flexi_logger;

mod config;
mod server;
mod service;

use flexi_logger::{AdaptiveFormat, Age, Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};

use clap::{Parser, Subcommand};

#[derive(Subcommand, Debug)]
enum Command {
    /// install service
    Install,
    /// uninstall service
    Uninstall,
    /// start service
    Start,
    /// stop service
    Stop,
    /// run service (by Windows)
    Run,
}

/// SOCKS5 proxy windows service
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

macro_rules! handle_error {
    ($name:expr, $res:expr) => {
        match $res {
            Err(e) => {
                log::error!("{} error: {:#?}", $name, e)
            }
            _ => (),
        }
    };
}

fn main() {
    let args = Cli::parse();

    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(
            FileSpec::default().directory(std::env::current_exe().unwrap().parent().unwrap()),
        )
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(4),
        )
        .adaptive_format_for_stderr(AdaptiveFormat::Detailed)
        .print_message()
        .duplicate_to_stderr(Duplicate::Warn)
        .start_with_specfile(
            std::env::current_exe()
                .unwrap()
                .with_file_name("logspec.toml"),
        )
        .unwrap();

    match args.command {
        Command::Install => handle_error!("install", service::install()),
        Command::Uninstall => handle_error!("uninstall", service::uninstall()),
        Command::Run => handle_error!("run", service::run()),
        Command::Start => handle_error!("start", service::start()),
        Command::Stop => handle_error!("stop", service::stop()),
    }
}
