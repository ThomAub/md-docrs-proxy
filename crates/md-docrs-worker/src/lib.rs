#![warn(clippy::pedantic)]

use md_docrs_core::{
    Error, ItemSpec,
    cache::CacheKey,
    fetch::{DOCS_RS_BASE, build_url, validate_format_version},
    render_loaded_crate,
};
use rustdoc_types::{Crate, FORMAT_VERSION};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufReader, Cursor, Read},
    sync::Arc,
};
use worker::kv::{KvError, KvStore};
use worker::{Context, Env, Fetch, Headers, Method, Request, RequestInit, Response, Result, event};

#[derive(Clone)]
struct AppState {
    fetcher: Arc<WorkerFetcher>,
    cache: Arc<KvCrateCache>,
}

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

    async fn fetch_bytes(
        &self,
        url: &str,
        method: Method,
    ) -> md_docrs_core::Result<(u16, Vec<u8>)> {
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

    async fn head_status(&self, url: &str) -> md_docrs_core::Result<u16> {
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

impl WorkerFetcher {
    async fn fetch(
        &self,
        crate_name: &str,
        version: &str,
        target: Option<&str>,
    ) -> md_docrs_core::Result<Crate> {
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
                    "{crate_name}@{version} has no rustdoc JSON for format version \
                     {FORMAT_VERSION}; waiting on docs.rs rebuild"
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

        let mut decoder = ruzstd::decoding::StreamingDecoder::new(BufReader::new(Cursor::new(
            bytes,
        )))
        .map_err(|err| {
            Error::Io(std::io::Error::other(format!(
                "zstd decode init failed: {err}"
            )))
        })?;
        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded)?;
        let krate: Crate = serde_json::from_slice(&decoded)?;
        validate_format_version(&krate)?;
        Ok(krate)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedCrate {
    krate: Crate,
}

#[derive(Clone)]
struct KvCrateCache {
    kv: KvStore,
    ttl_seconds: u64,
}

impl KvCrateCache {
    fn new(kv: KvStore) -> Self {
        Self {
            kv,
            ttl_seconds: 60 * 60,
        }
    }

    fn key_string(key: &CacheKey) -> String {
        match &key.target {
            Some(target) => format!("crate:{}:{}:{}", key.crate_name, key.version, target),
            None => format!("crate:{}:{}", key.crate_name, key.version),
        }
    }
}

impl KvCrateCache {
    async fn get(&self, key: &CacheKey) -> Option<Arc<Crate>> {
        let cache_key = Self::key_string(key);

        match self.kv.get(&cache_key).json::<CachedCrate>().await {
            Ok(Some(cached)) => Some(Arc::new(cached.krate)),
            Ok(None) | Err(_) => None,
        }
    }

    async fn put(&self, key: CacheKey, value: Arc<Crate>) {
        let cache_key = Self::key_string(&key);
        let cached = CachedCrate {
            krate: (*value).clone(),
        };

        let Ok(payload) = serde_json::to_string(&cached) else {
            return;
        };

        let builder = match self.kv.put(&cache_key, payload) {
            Ok(builder) => builder.expiration_ttl(self.ttl_seconds),
            Err(err) => {
                if matches!(err, KvError::InvalidKvStore(_)) {
                    panic!("invalid kv store");
                }
                return;
            }
        };

        if let Err(err) = builder.execute().await
            && matches!(err, KvError::InvalidKvStore(_))
        {
            panic!("invalid kv store");
        }
    }
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let kv = env.kv("KRATE_KV")?;
    let state = AppState {
        fetcher: Arc::new(WorkerFetcher::new()),
        cache: Arc::new(KvCrateCache::new(kv)),
    };

    route(req, state).await
}

async fn route(req: Request, state: AppState) -> Result<Response> {
    let path = req.path();
    let url = req.url()?;

    if let Some(spec) = url
        .query_pairs()
        .find(|(key, _)| key == "spec")
        .map(|(_, value)| value.into_owned())
    {
        let target = url
            .query_pairs()
            .find(|(key, _)| key == "target")
            .map(|(_, value)| value.into_owned());
        return serve_spec(&state, &spec, target).await;
    }

    if path == "/" {
        return text_response(
            200,
            "md-docrs-worker - GET /<crate>[/<version>][/<path>] for Markdown docs\n",
            "text/plain; charset=utf-8",
        );
    }

    if path == "/healthz" {
        return text_response(200, "ok", "text/plain; charset=utf-8");
    }

    if path == "/kv" {
        return kv_list(&state).await;
    }

    let target = url
        .query_pairs()
        .find(|(key, _)| key == "target")
        .map(|(_, value)| value.into_owned());

    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if segments.is_empty() {
        return text_response(
            200,
            "md-docrs-worker - GET /<crate>[/<version>][/<path>] for Markdown docs\n",
            "text/plain; charset=utf-8",
        );
    }

    let crate_name = segments[0].to_string();
    let version = if segments.len() >= 2 {
        segments[1].to_string()
    } else {
        "latest".to_string()
    };

    let path_segs = if segments.len() > 2 {
        parse_rest_segments(&segments[2..])
    } else {
        Vec::new()
    };

    serve(&state, &crate_name, &version, target, &path_segs).await
}

fn parse_rest_segments(segments: &[&str]) -> Vec<String> {
    if segments.is_empty() {
        return vec![];
    }

    let last_idx = segments.len() - 1;
    let mut out = Vec::with_capacity(segments.len());

    for (idx, segment) in segments.iter().enumerate() {
        if idx == last_idx {
            if let Some(name) = strip_kind_prefix(segment) {
                out.push(name);
            } else {
                out.push((*segment).to_string());
            }
        } else {
            out.push((*segment).to_string());
        }
    }

    out
}

fn strip_kind_prefix(segment: &str) -> Option<String> {
    let segment = segment.strip_suffix(".html").unwrap_or(segment);

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
        if let Some(rest) = segment.strip_prefix(prefix) {
            return Some(rest.to_string());
        }
    }

    None
}

async fn kv_list(state: &AppState) -> Result<Response> {
    let list_response = state
        .cache
        .kv
        .list()
        .limit(100)
        .execute()
        .await
        .map_err(|e| {
            if matches!(e, KvError::InvalidKvStore(_)) {
                panic!("invalid kv store");
            }
            e
        })?;

    let body = serde_json::to_string_pretty(&list_response)
        .map_err(|err| worker::Error::RustError(err.to_string()))?;

    text_response(200, &body, "application/json; charset=utf-8")
}

async fn serve_spec(state: &AppState, raw_spec: &str, target: Option<String>) -> Result<Response> {
    let spec = match ItemSpec::parse(raw_spec) {
        Ok(spec) => spec.with_target(target),
        Err(err) => return error_response(&err),
    };

    render_spec_response(state, spec).await
}

async fn serve(
    state: &AppState,
    crate_name: &str,
    version: &str,
    target: Option<String>,
    path_segs: &[String],
) -> Result<Response> {
    let path: Vec<String> = match path_segs.split_first() {
        Some((head, tail)) if head == crate_name => tail.to_vec(),
        _ => path_segs.to_vec(),
    };

    let spec = ItemSpec {
        crate_name: crate_name.to_string(),
        version: version.to_string(),
        target,
        path,
    };

    render_spec_response(state, spec).await
}

async fn render_spec_response(state: &AppState, spec: ItemSpec) -> Result<Response> {
    let key = CacheKey {
        crate_name: spec.crate_name.clone(),
        version: spec.version.clone(),
        target: spec.target.clone(),
    };

    let krate = if let Some(hit) = state.cache.get(&key).await {
        hit
    } else {
        let fetched = match state
            .fetcher
            .fetch(&spec.crate_name, &spec.version, spec.target.as_deref())
            .await
        {
            Ok(fetched) => fetched,
            Err(err) => return error_response(&err),
        };
        let krate = Arc::new(fetched);
        state.cache.put(key, Arc::clone(&krate)).await;
        krate
    };

    match render_loaded_crate(&krate, &spec) {
        Ok(body) => markdown_response(&body),
        Err(err) => error_response(&err),
    }
}

fn markdown_response(body: &str) -> Result<Response> {
    let headers = Headers::new();
    headers.set("content-type", "text/markdown; charset=utf-8")?;
    headers.set("vary", "Accept")?;
    headers.set("x-markdown-tokens", &(body.len() / 4).to_string())?;

    Ok(Response::ok(body.to_string())?.with_headers(headers))
}

fn error_response(err: &Error) -> Result<Response> {
    let status = match err {
        Error::NotFound(_) => 404,
        Error::InvalidSpec(_) => 400,
        Error::FormatVersionMismatch { .. } | Error::Fetch(_) | Error::Json(_) | Error::Io(_) => {
            502
        }
    };

    text_response(status, &err.to_string(), "text/plain; charset=utf-8")
}

fn text_response(status: u16, body: &str, content_type: &str) -> Result<Response> {
    let headers = Headers::new();
    headers.set("content-type", content_type)?;

    Ok(Response::ok(body.to_string())?
        .with_headers(headers)
        .with_status(status))
}
