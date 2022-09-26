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
    /// save default config
    SaveConfig,
}

/// SOCKS5 proxy windows service
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

fn main() {
    let args = Cli::parse();

    let logger = Logger::try_with_str("info")
        .unwrap()
        .log_to_file(
            FileSpec::default().directory(std::env::current_exe().unwrap().parent().unwrap()),
        )
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(4),
        )
        .format(flexi_logger::detailed_format)
        .adaptive_format_for_stderr(AdaptiveFormat::Detailed)
        .print_message()
        .duplicate_to_stderr(Duplicate::Warn)
        .write_mode(flexi_logger::WriteMode::Async)
        .start_with_specfile(
            std::env::current_exe()
                .unwrap()
                .with_file_name("logspec.toml"),
        )
        .unwrap();

    let res = match args.command {
        Command::Install => service::install(),
        Command::Uninstall => service::uninstall(),
        Command::Run => service::run(),
        Command::Start => service::start(),
        Command::Stop => service::stop(),
        Command::SaveConfig => {
            config::Config::default().save();
            Ok(())
        }
    };

    if let Err(e) = res {
        log::error!("{:?} error: {:#?}", args.command, e);
    }

    drop(logger);
}
