use fast_socks5::{
    server::{SimpleUserPassword, Socks5Server, Socks5Socket},
    Result,
};
use std::future::Future;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::select;
use tokio::task;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::config::PasswordAuth;

pub async fn spawn_socks5_server(cfg: Config, token: CancellationToken) -> Result<()> {
    let mut server_config = fast_socks5::server::Config::default();
    server_config.set_request_timeout(cfg.request_timeout);
    server_config.set_skip_auth(cfg.skip_auth);
    server_config.set_dns_resolve(cfg.dns_resolve);
    server_config.set_execute_command(cfg.execute_command);
    server_config.set_udp_support(cfg.allow_udp);

    if let Some(PasswordAuth { username, password }) = cfg.auth {
        server_config.set_authentication(SimpleUserPassword { username, password });
        log::info!("Simple auth system has been set.");
    } else {
        log::warn!("No authentication has been set!");
    }

    let mut listener = Socks5Server::bind(&cfg.listen_addr).await?;
    listener.set_config(server_config);

    let mut incoming = listener.incoming();

    log::info!("Listen for socks connections @ {}", &cfg.listen_addr);

    // Standard TCP loop
    while let Some(socket_res) = or_chancel(incoming.next(), token.child_token()).await {
        match socket_res {
            Ok(socket) => {
                let child_token = token.child_token();
                spawn_and_log_error(socket.upgrade_to_socks5(), child_token);
            }
            Err(err) => {
                log::error!("accept error = {:?}", err);
            }
        }
    }

    Ok(())
}

async fn or_chancel<F, R>(future: F, token: CancellationToken) -> Option<R>
where
    F: Future<Output = Option<R>>,
{
    select! {
        _ = token.cancelled() => {
            log::error!("canceled");
            None
        }
        res = future => {
            res
        }
    }
}

fn spawn_and_log_error<F, T>(future: F, token: CancellationToken) -> task::JoinHandle<()>
where
    F: Future<Output = Result<Socks5Socket<T>>> + Send + 'static,
    T: AsyncRead + AsyncWrite + Unpin,
{
    tokio::spawn(async move {
        // Wait for either cancellation or a very long time
        let result = select! {
            _ = token.cancelled() => {
                Err("Client connection canceled".to_string())
            }
            res = future => {
                res.map_err(|e| format!("{:#}", &e))
            }
        };
        if let Err(e) = result {
            log::error!("{}", &e);
        }
    })
}
