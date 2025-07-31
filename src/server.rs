use anyhow::{Context, Result, anyhow};
use fast_socks5::server::{DnsResolveHelper, Socks5ServerProtocol, run_tcp_proxy, run_udp_proxy};
use fast_socks5::{ReplyError, Socks5Command, SocksError};
use std::future::Future;
use tokio::net::TcpListener;
use tokio::select;
use tokio::task;
use tokio_stream::{Stream, StreamExt, wrappers::TcpListenerStream};
use tokio_util::sync::CancellationToken;

use async_stream::stream;

use crate::config::Config;
use crate::config::PasswordAuth;

pub fn server_executor(cfg: Config, token: CancellationToken) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async { spawn_socks5_server(cfg, token).await })
}

pub async fn spawn_socks5_server(cfg: Config, token: CancellationToken) -> Result<()> {
    cfg.validate()?;

    let listener = TcpListener::bind(&cfg.listen_addr).await?;

    let incoming = stream_with_cancellation(TcpListenerStream::new(listener), &token);
    tokio::pin!(incoming);

    log::info!("Listen for socks connections @ {}", &cfg.listen_addr);

    while let Some(socket_res) = incoming.next().await {
        match socket_res {
            Ok(socket) => {
                let child_token = token.child_token();
                spawn_and_log_error(serve_socks5(cfg.clone(), socket), child_token);
            }
            Err(err) => {
                log::error!("accept error: {err}");
            }
        }
    }

    Ok(())
}

async fn serve_socks5(cfg: Config, socket: tokio::net::TcpStream) -> Result<(), SocksError> {
    let (proto, cmd, target_addr) = match &cfg.auth {
        None if cfg.skip_auth => Socks5ServerProtocol::skip_auth_this_is_not_rfc_compliant(socket),
        None => Socks5ServerProtocol::accept_no_auth(socket).await?,
        Some(PasswordAuth { username, password }) => {
            Socks5ServerProtocol::accept_password_auth(socket, |user, pass| {
                user == *username && pass == *password
            })
            .await?
            .0
        }
    }
    .read_command()
    .await?
    .resolve_dns()
    .await?;

    match cmd {
        Socks5Command::TCPConnect => {
            run_tcp_proxy(proto, &target_addr, cfg.request_timeout, false).await?;
        }
        Socks5Command::UDPAssociate if cfg.allow_udp => {
            let reply_ip = cfg.public_addr.context("invalid reply ip")?;
            run_udp_proxy(proto, &target_addr, None, reply_ip, None).await?;
        }
        _ => {
            proto.reply_error(&ReplyError::CommandNotSupported).await?;
            return Err(ReplyError::CommandNotSupported.into());
        }
    };
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

fn spawn_and_log_error<F>(future: F, token: CancellationToken) -> task::JoinHandle<()>
where
    F: Future<Output = Result<(), SocksError>> + Send + 'static,
{
    tokio::spawn(async move {
        let result = select! {
            biased;

            _ = token.cancelled() => {
                Err(anyhow!("Client connection canceled"))
            }
            res = future => {
                res.map_err(anyhow::Error::new)
            }
        };
        if let Err(e) = result {
            log::error!("{}", &e);
        }
    })
}
