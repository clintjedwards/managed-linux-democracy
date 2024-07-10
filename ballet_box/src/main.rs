use anyhow::{bail, Result};
use axum::{
    body::Body,
    extract::{ConnectInfo, Json, Path, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use dashmap::DashMap;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    str::FromStr,
    sync::atomic::AtomicU64,
};
use tracing::{error, info, warn};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

#[derive(RustEmbed)]
#[folder = "public"]
pub struct EmbeddedFrontendFS;

struct AppContext {
    vim_votes: AtomicU64,
    emacs_votes: AtomicU64,
    vscode_votes: AtomicU64,
    rate_limiter: DashMap<IpAddr, u64>,
}

impl AppContext {
    fn new() -> Self {
        Self {
            vim_votes: AtomicU64::new(0),
            emacs_votes: AtomicU64::new(0),
            vscode_votes: AtomicU64::new(0),
            rate_limiter: DashMap::new(),
        }
    }
}

#[derive(Debug)]
enum PossibleVote {
    Vim,
    Emacs,
    Vscode,
}

#[derive(Debug)]
pub struct AppError {
    status: axum::http::StatusCode,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let body = serde_json::json!({ "error": self.message });
        (self.status, axum::Json(body)).into_response()
    }
}

fn check_vote(user_vote: &str) -> Result<PossibleVote> {
    let user_vote = user_vote.to_lowercase();

    match user_vote.as_str() {
        "vim" => Ok(PossibleVote::Vim),
        "emacs" => Ok(PossibleVote::Emacs),
        "vscode" => Ok(PossibleVote::Vscode),
        _ => bail!("Not a valid vote"),
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct VoteRequest {
    vote: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct VoteResponse {
    current_tally: Vec<(String, u64)>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CurrentWinnerResponse {
    winner: String,
}

#[tokio::main]
async fn main() {
    let app_state = std::sync::Arc::new(AppContext::new());

    init_logger().unwrap();

    let args: Vec<String> = std::env::args().collect();

    let bind_address = if args.len() > 1 {
        std::net::SocketAddrV4::from_str(&args[1]).unwrap()
    } else {
        SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 8080)
    };

    let app = Router::new()
        .route("/api/vote", post(vote_handler))
        .route("/api/current_winner", get(current_winner_handler))
        .route(
            "/",
            get(|| async { static_handler(Path("".to_string())).await }),
        )
        .route("/*path", get(static_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(bind_address).await.unwrap();

    info!(addr = %listener.local_addr().unwrap(), "started server");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

pub async fn static_handler(Path(path): Path<String>) -> Result<Response<Body>, AppError> {
    let path = if path.is_empty() {
        "index.html".to_string()
    } else {
        path
    };

    match EmbeddedFrontendFS::get(&path) {
        Some(content) => {
            let ext = std::path::Path::new(&path)
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("txt");

            let mime_type = mime_guess::from_ext(ext).first_or_text_plain();

            Ok(Response::builder()
                .header(axum::http::header::CONTENT_TYPE, mime_type.as_ref())
                .body(Body::from(content.data.clone()))
                .unwrap())
        }
        None => Ok(Response::builder()
            .status(axum::http::StatusCode::NOT_FOUND)
            .header(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from("<h1>404</h1><p>Not Found</p>"))
            .unwrap()),
    }
}

async fn current_winner_handler(
    State(state): State<std::sync::Arc<AppContext>>,
) -> Result<Json<CurrentWinnerResponse>, AppError> {
    let mut winner = ("", 0);

    if state.emacs_votes.load(std::sync::atomic::Ordering::Relaxed) > winner.1 {
        winner = (
            "emacs",
            state.emacs_votes.load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    if state
        .vscode_votes
        .load(std::sync::atomic::Ordering::Relaxed)
        > winner.1
    {
        winner = (
            "vscode",
            state
                .vscode_votes
                .load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    if state.vim_votes.load(std::sync::atomic::Ordering::Relaxed) > winner.1 {
        winner = (
            "vim",
            state.vim_votes.load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    Ok(Json(CurrentWinnerResponse {
        winner: winner.0.into(),
    }))
}

async fn vote_handler(
    State(state): State<std::sync::Arc<AppContext>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(input): Json<VoteRequest>,
) -> Result<Json<VoteResponse>, AppError> {
    let now = chrono::Utc::now();
    let epoch_seconds = now.timestamp() as u64;

    let matched_ip = state.rate_limiter.get(&addr.ip());

    if let Some(matched_ip) = matched_ip {
        let last_request_time = *matched_ip.value();

        if epoch_seconds - last_request_time < 1 {
            return Err(AppError {
                status: axum::http::StatusCode::TOO_MANY_REQUESTS,
                message: "Okay, listen. Democracy has limits. You're doing that too much; try again in a second."
                    .into(),
            });
        }
    }

    state
        .rate_limiter
        .entry(addr.ip())
        .and_modify(|seconds| *seconds = epoch_seconds)
        .or_insert(epoch_seconds);

    let vote = match check_vote(&input.vote) {
        Ok(vote) => vote,
        Err(_) => {
            return Err(AppError {
                status: axum::http::StatusCode::BAD_REQUEST,
                message: "Not a valid vote; Must be one of 'vim', 'emacs', 'vscode';\
                 If your preferred choice of text editor isn't here...it sucks to suck."
                    .into(),
            });
        }
    };

    match vote {
        PossibleVote::Vim => state
            .vim_votes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        PossibleVote::Emacs => state
            .emacs_votes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        PossibleVote::Vscode => state
            .vscode_votes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    };

    let response = vec![
        (
            "vim_votes".into(),
            state.vim_votes.load(std::sync::atomic::Ordering::Relaxed),
        ),
        (
            "emacs_votes".into(),
            state.emacs_votes.load(std::sync::atomic::Ordering::Relaxed),
        ),
        (
            "vscode_votes".into(),
            state
                .vscode_votes
                .load(std::sync::atomic::Ordering::Relaxed),
        ),
    ];

    info!(choice = ?vote, "vote cast!");

    Ok(Json(VoteResponse {
        current_tally: response,
    }))
}

fn init_logger() -> Result<()> {
    let filter = EnvFilter::from_default_env()
        // These directives filter out debug information that is too numerous and we generally don't need during
        // development.
        .add_directive("sqlx=off".parse().expect("Invalid directive"))
        .add_directive("h2=off".parse().expect("Invalid directive"))
        .add_directive("hyper=off".parse().expect("Invalid directive"))
        .add_directive("rustls=off".parse().expect("Invalid directive"))
        .add_directive("bollard=off".parse().expect("Invalid directive"))
        .add_directive("reqwest=off".parse().expect("Invalid directive"))
        .add_directive("tungstenite=off".parse().expect("Invalid directive"))
        .add_directive(LevelFilter::DEBUG.into()); // Accept debug level logs and above for everything else

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();

    Ok(())
}
