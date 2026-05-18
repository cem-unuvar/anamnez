//! TLS accept loop + per-connection device_id injection.
//!
//! axum 0.7 + hyper 1 + tokio-rustls 0.26: there is no first-class peer-cert
//! pipeline, so we run a manual accept loop that parses the client cert's CN
//! after handshake and stashes the resulting `WorkstationId` in each request's
//! extensions before dispatching to the router.

use crate::serve::app_state::AppState;
use crate::serve::boot::tls_paths;
use crate::serve::mtls;
use anamnez_core::error::{Error, Result};
use axum::Router;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

pub async fn run(addr: SocketAddr, state: AppState) -> Result<()> {
    let data_dir = state
        .config
        .db_path
        .parent()
        .ok_or(Error::Invariant("db_path has no parent"))?;
    let paths = tls_paths(data_dir);

    let server_cert_pem = std::fs::read_to_string(&paths.server_cert)?;
    let server_key_pem = std::fs::read_to_string(&paths.server_key)?;
    let ca_pem = std::fs::read_to_string(&paths.ca_cert)?;

    let server_cert: Vec<CertificateDer<'static>> = {
        let mut c = std::io::Cursor::new(server_cert_pem.as_bytes());
        rustls_pemfile::certs(&mut c)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)?
    };
    let server_key: PrivateKeyDer<'static> = {
        let mut c = std::io::Cursor::new(server_key_pem.as_bytes());
        rustls_pemfile::private_key(&mut c)
            .map_err(Error::from)?
            .ok_or(Error::Invariant("server_key.pem: no private key found"))?
    };

    let verifier = mtls::build_verifier(&ca_pem, state.revoked_devices.clone())?;

    let tls_cfg = ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(server_cert, server_key)
        .map_err(|e| {
            let leaked: &'static str =
                Box::leak(format!("rustls server config: {e}").into_boxed_str());
            Error::Invariant(leaked)
        })?;

    let acceptor = TlsAcceptor::from(Arc::new(tls_cfg));
    let listener = TcpListener::bind(addr).await?;

    let app = crate::serve::routes::build(state.clone());

    tracing::info!("anamnez accept loop ready");

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "accept error");
                continue;
            }
        };
        let acceptor = acceptor.clone();
        let app = app.clone();
        tokio::spawn(async move {
            if let Err(e) = serve_connection(stream, peer_addr, acceptor, app).await {
                tracing::debug!(?peer_addr, error = %e, "connection closed");
            }
        });
    }
}

async fn serve_connection(
    stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    acceptor: TlsAcceptor,
    app: Router,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tls = acceptor.accept(stream).await?;
    let (_io, conn) = tls.get_ref();
    // mTLS is optional at the TLS layer (see `mtls::AnamnezClientVerifier::client_auth_mandatory`):
    // workstations doing enrollment exchange have no client cert yet. When a cert is
    // present, stamp the resulting `WorkstationId` onto the request extensions so the
    // `require_device_id` middleware sees it; otherwise leave the extension off and
    // every authed/non-enrollment route will reject with `Forbidden`.
    let device_id = conn
        .peer_certificates()
        .and_then(|certs| certs.first())
        .and_then(|c| mtls::workstation_id_from_cert(c).ok());

    let app = match device_id {
        Some(id) => app.layer(axum::Extension(id)),
        None => app,
    };
    let svc = TowerToHyperService::new(app);

    let io = TokioIo::new(tls);
    Builder::new(TokioExecutor::new())
        .serve_connection(io, svc)
        .await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            format!("hyper: {e}").into()
        })?;
    let _ = peer_addr;
    Ok(())
}

/// Adapter that lets a tower `Service` be used as a hyper `Service`. axum 0.7's
/// `Router` is a tower service; hyper 1 wants a hyper service. This adapter is
/// `Clone`-able and `Service::call` returns a future, matching hyper's contract.
#[derive(Clone)]
struct TowerToHyperService<S> {
    inner: S,
}

impl<S> TowerToHyperService<S> {
    fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, B> hyper::service::Service<hyper::Request<hyper::body::Incoming>> for TowerToHyperService<S>
where
    S: tower::Service<hyper::Request<hyper::body::Incoming>, Response = hyper::Response<B>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send,
{
    type Response = hyper::Response<B>;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<
            dyn std::future::Future<Output = std::result::Result<Self::Response, Self::Error>>
                + Send,
        >,
    >;

    fn call(&self, req: hyper::Request<hyper::body::Incoming>) -> Self::Future {
        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(req).await })
    }
}
