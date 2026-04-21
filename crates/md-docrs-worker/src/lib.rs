#![warn(clippy::pedantic)]

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use md_docrs_core::{
    Error, ItemSpec, Result, RustdocFetcher,
    cache::{CrateCache, InMemoryCache},
    fetch::{DOCS_RS_BASE, build_url, validate_format_version},
    render_spec,
};
use rustdoc_types::{Crate, FORMAT_VERSION};
use std::{future::Future, io::Cursor, pin::Pin, sync::Arc};
use tower_service::Service;
use worker::*;

#[derive(Clone)]
struct WorkerFetcher {
    base: String,
}

impl WorkerFetcher {
    fn new() -> Self {
        Self {
            base: DOCS_RS_BASE.to_string(),
        }
    }

    async fn fetch_bytes(&self, url: &str, method: Method) -> Result<(u16, Vec<u8>)> {
        let mut init = RequestInit::new();
        init.with_method(method);

        let request = Request::new_with_init(url, &init)
            .map_err(|err| Error::Fetch(format!("failed to build request for {url}: {err}")))?;

        let mut response = Fetch::Request(request)
            .send()
            .await
            .map_err(|err| Error::Fetch(format!("request failed for {url}: {err}")))?;

        let status = response.status_code();
        let bytes = response.bytes().await.map_err(|err| {
            Error::Fetch(format!("failed to read response body for {url}: {err}"))
        })?;

        Ok((status, bytes))
    }

    async fn head_status(&self, url: &str) -> Result<u16> {
        let mut init = RequestInit::new();
        init.with_method(Method::Head);

        let request = Request::new_with_init(url, &init)
            .map_err(|err| Error::Fetch(format!("failed to build request for {url}: {err}")))?;

        let response = Fetch::Request(request)
            .send()
            .await
            .map_err(|err| Error::Fetch(format!("request failed for {url}: {err}")))?;

        Ok(response.status_code())
    }
}

impl RustdocFetcher for WorkerFetcher {
    fn fetch<'a>(
        &'a self,
        crate_name: &'a str,
        version: &'a str,
        target: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Crate>> + 'a>> {
        Box::pin(async move {
            let url = build_url(
                &self.base,
                crate_name,
                version,
                target,
                Some(FORMAT_VERSION),
            );

            let (status, bytes) = self.fetch_bytes(&url, Method::Get).await?;

            if status == 404 {
                let probe_url = build_url(&self.base, crate_name, version, target, None);
                let probe_status = self.head_status(&probe_url).await?;
                if (200..300).contains(&probe_status) {
                    return Err(Error::Fetch(format!(
                        "{crate_name}@{version} has no rustdoc JSON for format version {FORMAT_VERSION}; waiting on docs.rs rebuild"
                    )));
                }
                return Err(Error::Fetch(format!(
                    "{crate_name}@{version} not found on docs.rs"
                )));
            }

            if !(200..300).contains(&status) {
                return Err(Error::Fetch(format!(
                    "{status} response for {crate_name}@{version}"
                )));
            }

            let decoded = zstd::decode_all(Cursor::new(bytes))?;
            let krate: Crate = serde_json::from_slice(&decoded)?;
            validate_format_version(&krate)?;
            Ok(krate)
        })
    }
}

#[derive(Clone)]
struct AppState {
    fetcher: Arc<dyn RustdocFetcher>,
    cache: Arc<dyn CrateCache>,
}

fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/healthz", get(healthz))
        .route("/{crate_name}", get(crate_root))
        .route("/{crate_name}/", get(crate_root))
        .route("/{crate_name}/{version}", get(version_root))
        .route("/{crate_name}/{version}/", get(version_root))
        .route("/{crate_name}/{version}/{*rest}", get(deep))
        .with_state(state)
}

async fn root() -> &'static str {
    "md-docrs-worker - GET /<crate>[/<version>][/<path>] for Markdown docs\n"
}

async fn healthz() -> &'static str {
    "ok"
}

#[axum_macros::debug_handler]
async fn crate_root(
    State(state): State<Arc<AppState>>,
    Path(crate_name): Path<String>,
) -> Response {
    serve(&state, &crate_name, "latest", &[]).await
}

#[axum_macros::debug_handler]
async fn version_root(
    State(state): State<Arc<AppState>>,
    Path((crate_name, version)): Path<(String, String)>,
) -> Response {
    serve(&state, &crate_name, &version, &[]).await
}

#[axum_macros::debug_handler]
async fn deep(
    State(state): State<Arc<AppState>>,
    Path((crate_name, version, rest)): Path<(String, String, String)>,
) -> Response {
    let path_segs = parse_rest(&rest);
    serve(&state, &crate_name, &version, &path_segs).await
}

fn parse_rest(rest: &str) -> Vec<String> {
    let rest = rest.trim_end_matches('/');
    if rest.is_empty() {
        return vec![];
    }

    let parts: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    let mut out = Vec::with_capacity(parts.len());
    if parts.is_empty() {
        return out;
    }

    let last_idx = parts.len() - 1;
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
    let path = match path_segs.split_first() {
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

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    _env: Env,
    _ctx: Context,
) -> worker::Result<axum::http::Response<axum::body::Body>> {
    let state = Arc::new(AppState {
        fetcher: Arc::new(WorkerFetcher::new()),
        cache: Arc::new(InMemoryCache::default()),
    });

    router(state)
        .call(req)
        .await
        .map_err(|err| worker::Error::RustError(err.to_string()))
}
