extern crate flexi_logger;

mod config;
mod server;
mod service;

use config::Config;
use flexi_logger::{
    AdaptiveFormat, Age, Cleanup, Criterion, Duplicate, FileSpec, Logger, LoggerHandle, Naming,
};

use clap::{Parser, Subcommand};
use tokio_util::sync::CancellationToken;

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
    /// save default config
    SaveConfig,
    /// run server as foreground proccess
    Serve,
}

/// SOCKS5 proxy windows service
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

fn create_logger() -> LoggerHandle {
    Logger::try_with_str("info")
        .expect("default logging level invalid")
        .log_to_file(
            FileSpec::default().directory(
                std::env::current_exe()
                    .expect("can't get current exe path")
                    .parent()
                    .expect("can't get parent folder"),
            ),
        )
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(4),
        )
        .format(flexi_logger::detailed_format)
        .adaptive_format_for_stdout(AdaptiveFormat::Detailed)
        .print_message()
        .duplicate_to_stdout(Duplicate::Info)
        .write_mode(flexi_logger::WriteMode::Async)
        .start_with_specfile(
            std::env::current_exe()
                .unwrap()
                .with_file_name("logspec.toml"),
        )
        .expect("can't start logger")
}

fn server_foreground() {
    let control_token = CancellationToken::new();
    let server_token = control_token.child_token();

    let res = ctrlc::set_handler(move || {
        log::info!("recieved Ctrl-C");
        control_token.cancel();
    });

    if res.is_ok() {
        log::info!("Press Ctrl-C to stop server");
    }

    server::server_executor(Config::get(), server_token).unwrap();
}

fn main() {
    let args = Cli::parse();
    let logger = create_logger();

    let res = match args.command {
        Command::Install => service::install(),
        Command::Uninstall => service::uninstall(),
        Command::Run => service::run(),
        Command::Start => service::start(),
        Command::Stop => service::stop(),
        Command::SaveConfig => {
            Config::default().save();
            Ok(())
        }
        Command::Serve => {
            server_foreground();
            Ok(())
        }
    };

    if let Err(e) = res {
        log::error!("{:?} error: {:#?}", args.command, e);
    }

    drop(logger);
}
