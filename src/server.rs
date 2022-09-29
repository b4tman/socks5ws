use fast_socks5::{
    server::{SimpleUserPassword, Socks5Server, Socks5Socket},
    Result,
};
use std::future::Future;
use tokio::io::{self, AsyncRead, AsyncWrite};
use tokio::select;
use tokio::task;
use tokio_stream::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use async_stream::stream;

use crate::config::Config;
use crate::config::PasswordAuth;

pub fn server_executor(cfg: Config, token: CancellationToken) -> io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async { spawn_socks5_server(cfg, token).await })
}

pub async fn spawn_socks5_server(cfg: Config, token: CancellationToken) -> io::Result<()> {
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

    let incoming = stream_with_cancellation(listener.incoming(), &token);
    tokio::pin!(incoming);

    log::info!("Listen for socks connections @ {}", &cfg.listen_addr);

    while let Some(socket_res) = incoming.next().await {
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

fn stream_with_cancellation<'a, S>(
    mut inner: S,
    token: &'a CancellationToken,
) -> impl Stream<Item = <S as Stream>::Item> + 'a
where
    S: StreamExt + Unpin + 'a,
{
    stream! {
        while let Some(res) = check_cancelled(inner.next(), token, None).await  {
            yield res;
        }
    }
}

async fn check_cancelled<F, R>(future: F, token: &CancellationToken, default: R) -> R
where
    F: Future<Output = R>,
{
    select! {
        biased;

        _ = token.cancelled() => {
            log::error!("accept canceled");

            default
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
        let result = select! {
            biased;

            _ = token.cancelled() => {
                Err("Client connection canceled".to_string())
            }
            res = future => {
                res.map_err(|e| e.to_string())
            }
        };
        if let Err(e) = result {
            log::error!("{}", &e);
        }
    })
}
