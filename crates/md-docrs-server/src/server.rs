use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use md_docrs_core::{Error, ItemSpec, RustdocFetcher, cache::CrateCache, render_spec};
use std::sync::Arc;

pub struct AppState {
    pub fetcher: Arc<dyn RustdocFetcher>,
    pub cache: Arc<dyn CrateCache>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/healthz", get(|| async { "ok" }))
        .route("/{crate_name}", get(crate_root))
        .route("/{crate_name}/", get(crate_root))
        .route("/{crate_name}/{version}", get(version_root))
        .route("/{crate_name}/{version}/", get(version_root))
        .route("/{crate_name}/{version}/{*rest}", get(deep))
        .with_state(state)
}

async fn root() -> &'static str {
    "md-docrs-server - GET /<crate>[/<version>][/<path>] for Markdown docs\n"
}

#[axum::debug_handler]
async fn crate_root(
    State(state): State<Arc<AppState>>,
    Path(crate_name): Path<String>,
) -> Response {
    serve(&state, &crate_name, "latest", &[]).await
}

#[axum::debug_handler]
async fn version_root(
    State(state): State<Arc<AppState>>,
    Path((crate_name, version)): Path<(String, String)>,
) -> Response {
    serve(&state, &crate_name, &version, &[]).await
}

#[axum::debug_handler]
async fn deep(
    State(state): State<Arc<AppState>>,
    Path((crate_name, version, rest)): Path<(String, String, String)>,
) -> Response {
    let path_segs = parse_rest(&rest);
    serve(&state, &crate_name, &version, &path_segs).await
}

/// Parse the tail of a docs.rs-style URL into item path segments.
///
/// Examples:
/// - empty string -> `[]`
/// - `serde/` -> `[serde]` (will be stripped)
/// - `serde/de/trait.Deserialize.html` -> `[serde, de, Deserialize]`
/// - `anyhow/struct.Error.html` -> `[anyhow, Error]`
fn parse_rest(rest: &str) -> Vec<String> {
    let rest = rest.trim_end_matches('/');
    if rest.is_empty() {
        return vec![];
    }

    let parts: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return vec![];
    }

    let last_idx = parts.len() - 1;
    let mut out = Vec::with_capacity(parts.len());

    for (i, seg) in parts.iter().enumerate() {
        if i == last_idx {
            if let Some(name) = strip_kind_prefix(seg) {
                out.push(name);
            } else {
                out.push((*seg).to_string());
            }
        } else {
            out.push((*seg).to_string());
        }
    }

    out
}

fn strip_kind_prefix(seg: &str) -> Option<String> {
    let seg = seg.strip_suffix(".html").unwrap_or(seg);

    for prefix in [
        "struct.",
        "enum.",
        "trait.",
        "fn.",
        "type.",
        "constant.",
        "static.",
        "macro.",
        "union.",
        "primitive.",
        "derive.",
        "attr.",
    ] {
        if let Some(rest) = seg.strip_prefix(prefix) {
            return Some(rest.to_string());
        }
    }

    None
}

async fn serve(
    state: &AppState,
    crate_name: &str,
    version: &str,
    path_segs: &[String],
) -> Response {
    let path: Vec<String> = match path_segs.split_first() {
        Some((head, tail)) if head == crate_name => tail.to_vec(),
        _ => path_segs.to_vec(),
    };

    let spec = ItemSpec {
        crate_name: crate_name.to_string(),
        version: version.to_string(),
        target: None,
        path,
    };

    match render_spec(&spec, state.fetcher.as_ref(), state.cache.as_ref()).await {
        Ok(body) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                "text/markdown; charset=utf-8".parse().unwrap(),
            );
            headers.insert(header::VARY, "Accept".parse().unwrap());
            headers.insert(
                "x-markdown-tokens",
                (body.len() / 4).to_string().parse().unwrap(),
            );
            (StatusCode::OK, headers, body).into_response()
        }
        Err(err) => error_to_response(&err),
    }
}

fn error_to_response(err: &Error) -> Response {
    let status = match err {
        Error::NotFound(_) => StatusCode::NOT_FOUND,
        Error::InvalidSpec(_) => StatusCode::BAD_REQUEST,
        Error::FormatVersionMismatch { .. } | Error::Fetch(_) | Error::Json(_) | Error::Io(_) => {
            StatusCode::BAD_GATEWAY
        }
    };
    (status, err.to_string()).into_response()
}
