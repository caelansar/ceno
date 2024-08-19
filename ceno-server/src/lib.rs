mod config;
mod engine;
mod error;
mod pool;
mod router;

use anyhow::Result;
use axum::{
    body::Bytes,
    extract::{Host, Query, State},
    http::{request::Parts, Response},
    response::IntoResponse,
    routing::any,
    Router,
};
use dashmap::DashMap;
use matchit::Match;
use std::{collections::HashMap, sync::Arc};
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{info, info_span, instrument, Instrument};

pub use config::*;
pub use engine::{Req, Res};
pub use error::*;
pub use pool::*;
pub use router::*;

#[derive(Clone)]
pub struct AppState {
    pools: DashMap<String, Arc<ThreadPool>>,
    routers: DashMap<String, SwappableAppRouter>,
}

#[derive(Clone)]
pub struct TenentRouter {
    host: String,
    router: SwappableAppRouter,
}

pub async fn start_server(port: u16, routers: Vec<TenentRouter>) -> Result<()> {
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(addr).await?;

    info!("listening on {}", listener.local_addr()?);

    let map = DashMap::new();
    for TenentRouter { host, router } in routers {
        map.insert(host, router);
    }
    let state = AppState::new(map);
    let app = Router::new()
        .route("/*path", any(handler))
        .with_state(state);

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {info!("shutdown server gracefully")},
        _ = terminate => {},
    }
}

#[instrument(skip(state))]
async fn handler(
    State(state): State<AppState>,
    parts: Parts,
    Host(host): Host,
    Query(query): Query<HashMap<String, String>>,
    body: Option<Bytes>,
) -> Result<impl IntoResponse, AppError> {
    let (router, pool) = get_router_by_host(host, state)?;
    let matched = router.match_it(parts.method.clone(), parts.uri.path())?;
    info!(%matched.value, "router matched");

    let req = assemble_req(&matched, &parts, query, body)?;
    let handler = matched.value;
    // let worker = JsWorker::try_new(&router.code)?;
    //
    // let res = worker.run(handler, req)?;
    // info!(?res, "run JsWorker");

    let res = pool
        .execute(handler, req)
        .instrument(info_span!("wait response asynchronously").or_current())
        .await
        .map_err(|_| AppError::HostNotFound("".to_string()))?;
    info!(?res, "pool execute");

    Ok(Response::from(res))
}

impl AppState {
    pub fn new(routers: DashMap<String, SwappableAppRouter>) -> Self {
        let pools = DashMap::new();
        for data in routers.iter() {
            let (host, router) = data.pair();
            let inner = ThreadPool::new(4, &router.load().code);
            pools.insert(host.to_string(), Arc::new(inner));
        }
        Self { routers, pools }
    }
}

impl TenentRouter {
    pub fn new(host: impl Into<String>, router: SwappableAppRouter) -> Self {
        Self {
            host: host.into(),
            router,
        }
    }
}

#[instrument(skip(state))]
fn get_router_by_host(
    mut host: String,
    state: AppState,
) -> Result<(AppRouter, Arc<ThreadPool>), AppError> {
    let _ = host.split_off(host.find(':').unwrap_or(host.len()));

    info!(%host, "split host");

    let router: AppRouter = state
        .routers
        .get(&host)
        .ok_or(AppError::HostNotFound(host.clone()))?
        .load();

    let pool = state
        .pools
        .get(&host)
        .ok_or(AppError::HostNotFound(host))?
        .clone();

    Ok((router, pool))
}

fn assemble_req(
    matched: &Match<&str>,
    parts: &Parts,
    query: HashMap<String, String>,
    body: Option<Bytes>,
) -> Result<Req, AppError> {
    let params: HashMap<String, String> = matched
        .params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    // convert request data into Req
    let headers = parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
        .collect();
    let body = body.and_then(|v| String::from_utf8(v.into()).ok());

    let req = Req::builder()
        .method(parts.method.to_string())
        .url(parts.uri.to_string())
        .query(query)
        .params(params)
        .headers(headers)
        .body(body)
        .build();

    Ok(req)
}
