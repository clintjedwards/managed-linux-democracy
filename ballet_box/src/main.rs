use anyhow::{bail, Context, Result};
use axum::{
    extract::{Extension, Json, State},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    str::FromStr,
    sync::atomic::AtomicU64,
};
use tracing::{error, info, warn};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

struct AppContext {
    vim_votes: AtomicU64,
    emacs_votes: AtomicU64,
    vscode_votes: AtomicU64,
}

impl AppContext {
    fn new() -> Self {
        Self {
            vim_votes: AtomicU64::new(0),
            emacs_votes: AtomicU64::new(0),
            vscode_votes: AtomicU64::new(0),
        }
    }
}

enum PossibleVote {
    Vim,
    Emacs,
    Vscode,
}

#[derive(Debug)]
struct AppError {
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
        .route("/", get(static_handler))
        .route("/vote", post(vote_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(bind_address).await.unwrap();

    info!(addr = %listener.local_addr().unwrap(), "started server");
    axum::serve(listener, app).await.unwrap();
}

async fn static_handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

async fn vote_handler(
    State(state): State<std::sync::Arc<AppContext>>,
    Json(input): Json<VoteRequest>,
) -> Result<Json<VoteResponse>, AppError> {
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
