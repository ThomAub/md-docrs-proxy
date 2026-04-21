//! Side-by-side comparison harness for the Zig and Rust WASM builds.
//!
//! Exercises two code paths on every artifact that exports them:
//!
//! - `resolve_url` (minimal column) — spec -> docs.rs URL, size/latency only.
//! - `render_spec` (full column) — spec -> fetched rustdoc JSON -> Markdown.
//!   The host provides `env.fetch_bytes` (blocking reqwest), then the guest
//!   owns the zstd decode + JSON parse + render.
//!
//! Artifacts expected under `artifacts/` (populated by `build.sh`):
//!   zig-minimal.wasm, zig-full.wasm, rust-minimal.wasm, rust-full.wasm
//!
//! Missing artifacts are skipped. `--offline` disables the render column so
//! the harness runs without network.
//!
//! Parity: for each spec, the first full-pipeline output is hashed (blake3)
//! and compared across artifacts.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};

const OUT_CAP: u32 = 512;
const DEFAULT_ITERATIONS: usize = 200;
const DEFAULT_RENDER_ITERATIONS: usize = 3;

#[derive(Clone, Debug)]
struct Spec {
    spec: &'static str,
    target: Option<&'static str>,
}

const DEFAULT_SPECS: &[Spec] = &[
    Spec {
        spec: "serde",
        target: None,
    },
    Spec {
        spec: "tokio@1.52.1::sync::Mutex",
        target: None,
    },
    Spec {
        spec: "anyhow::Error",
        target: Some("x86_64-unknown-linux-gnu"),
    },
    Spec {
        spec: "rustdoc-types@0.57::Crate",
        target: None,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Runtime {
    Wasmtime,
    #[cfg(feature = "wasmer")]
    Wasmer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flavor {
    Minimal,
    Full,
}

struct Artifact {
    label: &'static str,
    path: PathBuf,
    flavor: Flavor,
}

struct Args {
    runtime: Runtime,
    iterations: usize,
    render_iterations: usize,
    artifacts_dir: PathBuf,
    offline: bool,
}

fn parse_args() -> Result<Args> {
    let mut runtime = Runtime::Wasmtime;
    let mut iterations = DEFAULT_ITERATIONS;
    let mut render_iterations = DEFAULT_RENDER_ITERATIONS;
    let mut artifacts_dir = default_artifacts_dir();
    let mut offline = false;

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--runtime" => {
                let v = iter.next().context("--runtime expects a value")?;
                runtime = match v.as_str() {
                    "wasmtime" => Runtime::Wasmtime,
                    #[cfg(feature = "wasmer")]
                    "wasmer" => Runtime::Wasmer,
                    #[cfg(not(feature = "wasmer"))]
                    "wasmer" => {
                        bail!("wasmer runtime not compiled in; rebuild with `--features wasmer`")
                    }
                    other => bail!("unknown runtime: {other}"),
                };
            }
            "--iterations" => {
                iterations = iter
                    .next()
                    .context("--iterations expects a value")?
                    .parse()
                    .context("--iterations must be a positive integer")?;
            }
            "--render-iterations" => {
                render_iterations = iter
                    .next()
                    .context("--render-iterations expects a value")?
                    .parse()
                    .context("--render-iterations must be a positive integer")?;
            }
            "--artifacts-dir" => {
                artifacts_dir =
                    PathBuf::from(iter.next().context("--artifacts-dir expects a value")?);
            }
            "--offline" => offline = true,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}"),
        }
    }

    Ok(Args {
        runtime,
        iterations,
        render_iterations,
        artifacts_dir,
        offline,
    })
}

fn print_help() {
    println!(
        "usage: wasm-compare [--runtime wasmtime|wasmer]\n\
         \t[--iterations N] [--render-iterations N]\n\
         \t[--artifacts-dir PATH] [--offline]\n\
         \n\
         Reports size + median/p95 latency for each WASM artifact under\n\
         --artifacts-dir (default: wasm/artifacts). Runs resolve_url for all\n\
         artifacts; runs the full render_spec pipeline for artifacts that\n\
         export it. --offline skips the render column (no network).\n\
         \n\
         Expected artifacts:\n\
         \tzig-minimal.wasm      Zig ReleaseSmall, resolve_url only\n\
         \tzig-full.wasm         Zig ReleaseSmall, full pipeline\n\
         \trust-minimal.wasm     Rust wasm-release, --no-default-features\n\
         \trust-minimal-opt.wasm Rust wasm-release + wasm-opt -Oz, --no-default-features\n\
         \trust-full.wasm        Rust wasm-release, --features full\n\
         \trust-full-opt.wasm    Rust wasm-release + wasm-opt -Oz, --features full\n\
         Any subset is fine; missing artifacts are skipped."
    );
}

fn default_artifacts_dir() -> PathBuf {
    if let Some(dir) = option_env!("CARGO_MANIFEST_DIR") {
        return Path::new(dir)
            .join("..")
            .join("..")
            .join("wasm")
            .join("artifacts");
    }
    PathBuf::from("wasm/artifacts")
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let all = artifacts(&args);
    let present: Vec<_> = all
        .iter()
        .filter(|artifact| artifact.path.exists())
        .collect();

    if present.is_empty() {
        bail!(
            "no .wasm artifacts found under {}\n\
             run `./wasm/build.sh` first, or pass --artifacts-dir",
            args.artifacts_dir.display(),
        );
    }

    print_run_header(&args);
    print_artifact_sizes(&present)?;

    for spec in DEFAULT_SPECS {
        print_spec_header(spec);
        print_resolve_results(&present, &args, spec)?;
        print_render_results(&present, &args, spec)?;
        println!();
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

fn artifacts(args: &Args) -> [Artifact; 6] {
    [
        Artifact {
            label: "zig-minimal",
            path: args.artifacts_dir.join("zig-minimal.wasm"),
            flavor: Flavor::Minimal,
        },
        Artifact {
            label: "zig-full",
            path: args.artifacts_dir.join("zig-full.wasm"),
            flavor: Flavor::Full,
        },
        Artifact {
            label: "rust-minimal",
            path: args.artifacts_dir.join("rust-minimal.wasm"),
            flavor: Flavor::Minimal,
        },
        Artifact {
            label: "rust-minimal-opt",
            path: args.artifacts_dir.join("rust-minimal-opt.wasm"),
            flavor: Flavor::Minimal,
        },
        Artifact {
            label: "rust-full",
            path: args.artifacts_dir.join("rust-full.wasm"),
            flavor: Flavor::Full,
        },
        Artifact {
            label: "rust-full-opt",
            path: args.artifacts_dir.join("rust-full-opt.wasm"),
            flavor: Flavor::Full,
        },
    ]
}

fn print_run_header(args: &Args) {
    println!("runtime:    {:?}", args.runtime);
    println!(
        "iterations: resolve_url={}, render_spec={}",
        args.iterations, args.render_iterations
    );
    println!("artifacts:  {}", args.artifacts_dir.display());
    println!(
        "mode:       {}",
        if args.offline { "offline" } else { "online" }
    );
    println!();
}

fn print_artifact_sizes(present: &[&Artifact]) -> Result<()> {
    println!("{:<14} {:>10} {:>8}", "artifact", "bytes", "flavor");
    println!("{:-<14} {:->10} {:->8}", "", "", "");

    for artifact in present {
        let meta = fs::metadata(&artifact.path)?;
        let flavor = match artifact.flavor {
            Flavor::Minimal => "minimal",
            Flavor::Full => "full",
        };
        println!("{:<14} {:>10} {:>8}", artifact.label, meta.len(), flavor);
    }

    println!();
    Ok(())
}

fn print_spec_header(spec: &Spec) {
    println!(
        "spec: {}{}",
        spec.spec,
        spec.target
            .map(|target| format!(" (target={target})"))
            .unwrap_or_default(),
    );
    println!(
        "{:<14}  {:<60}  {:>10}  {:>10}",
        "artifact", "resolve_url output", "median us", "p95 us"
    );
    println!("{:-<14}  {:-<60}  {:->10}  {:->10}", "", "", "", "");
}

fn print_resolve_results(present: &[&Artifact], args: &Args, spec: &Spec) -> Result<()> {
    for artifact in present {
        let bytes = fs::read(&artifact.path)?;
        match run_resolve(args.runtime, &bytes, spec, args.iterations) {
            Ok(result) => {
                let output = result
                    .output
                    .as_deref()
                    .unwrap_or("<resolve_url returned 0>");
                let shown = truncate(output, 60);
                println!(
                    "{:<14}  {:<60}  {:>10}  {:>10}",
                    artifact.label,
                    shown,
                    result.median.as_micros(),
                    result.p95.as_micros(),
                );
            }
            Err(error) => println!("{:<14}  resolve_url error: {}", artifact.label, error),
        }
    }

    Ok(())
}

fn print_render_results(present: &[&Artifact], args: &Args, spec: &Spec) -> Result<()> {
    if args.offline
        || !present
            .iter()
            .any(|artifact| artifact.flavor == Flavor::Full)
    {
        return Ok(());
    }

    println!();
    println!(
        "{:<14}  {:>8}  {:>8}  {:>10}  {:>10}  {:<16}",
        "artifact", "md bytes", "fetch ms", "render ms", "total ms", "parity"
    );
    println!(
        "{:-<14}  {:->8}  {:->8}  {:->10}  {:->10}  {:-<16}",
        "", "", "", "", "", ""
    );

    let mut parity: HashMap<String, Vec<&str>> = HashMap::new();
    for artifact in present {
        if artifact.flavor != Flavor::Full {
            continue;
        }

        let bytes = fs::read(&artifact.path)?;
        match run_render(args.runtime, &bytes, spec, args.render_iterations) {
            Ok(result) => {
                let hash = blake3::hash(result.output.as_bytes());
                let short = short_hash(hash.to_hex().as_str());
                parity
                    .entry(short.clone())
                    .or_default()
                    .push(artifact.label);
                println!(
                    "{:<14}  {:>8}  {:>8}  {:>10}  {:>10}  {:<16}",
                    artifact.label,
                    result.output.len(),
                    result.fetch_median.as_millis(),
                    result.render_median.as_millis(),
                    result.total_median.as_millis(),
                    short,
                );
            }
            Err(error) => println!("{:<14}  render_spec error: {}", artifact.label, error),
        }
    }

    print_parity_summary(&parity);
    Ok(())
}

fn print_parity_summary(parity: &HashMap<String, Vec<&str>>) {
    if parity.len() > 1 {
        println!("parity:  outputs differ across artifacts");
        for (hash, labels) in parity {
            println!("   {}: {}", hash, labels.join(", "));
        }
    } else if let Some((hash, labels)) = parity.iter().next()
        && labels.len() > 1
    {
        println!(
            "parity:  all {} full artifacts agree ({})",
            labels.len(),
            hash
        );
    }
}

fn short_hash(hex: &str) -> String {
    hex.chars().take(12).collect()
}

struct ResolveResult {
    output: Option<String>,
    median: Duration,
    p95: Duration,
}

struct RenderResult {
    output: String,
    fetch_median: Duration,
    render_median: Duration,
    total_median: Duration,
}

fn run_resolve(
    runtime: Runtime,
    wasm_bytes: &[u8],
    spec: &Spec,
    iterations: usize,
) -> Result<ResolveResult> {
    match runtime {
        Runtime::Wasmtime => wasmtime_runner::run_resolve(wasm_bytes, spec, iterations),
        #[cfg(feature = "wasmer")]
        Runtime::Wasmer => wasmer_runner::run_resolve(wasm_bytes, spec, iterations),
    }
}

fn run_render(
    runtime: Runtime,
    wasm_bytes: &[u8],
    spec: &Spec,
    iterations: usize,
) -> Result<RenderResult> {
    match runtime {
        Runtime::Wasmtime => wasmtime_runner::run_render(wasm_bytes, spec, iterations),
        #[cfg(feature = "wasmer")]
        Runtime::Wasmer => wasmer_runner::run_render(wasm_bytes, spec, iterations),
    }
}

fn stats(mut samples: Vec<Duration>) -> (Duration, Duration) {
    samples.sort();
    let median = samples[samples.len() / 2];
    let p95_idx = samples.len().saturating_mul(95) / 100;
    let p95 = samples[p95_idx.min(samples.len() - 1)];
    (median, p95)
}

fn median_duration(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn blocking_http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("md-docrs-wasm-compare/", env!("CARGO_PKG_VERSION"),))
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")
}

mod wasmtime_runner {
    use super::{
        Context, Duration, HashMap, OUT_CAP, RenderResult, ResolveResult, Result, Spec, bail,
        blocking_http_client, median_duration, stats,
    };
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    use wasmtime::{Caller, Engine, Linker, Memory, Module, Store, TypedFunc};

    pub fn run_resolve(wasm_bytes: &[u8], spec: &Spec, iterations: usize) -> Result<ResolveResult> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm_bytes)?;
        let linker = build_linker::<()>(&engine)?;
        let mut store = Store::new(&engine, ());
        let instance = linker.instantiate(&mut store, &module)?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .context("wasm module is missing the `memory` export")?;
        let alloc: TypedFunc<u32, u32> = instance.get_typed_func(&mut store, "alloc")?;
        let free: TypedFunc<(u32, u32), ()> = instance.get_typed_func(&mut store, "free")?;
        let resolve_url: TypedFunc<(u32, u32, u32, u32, u32, u32), u32> =
            instance.get_typed_func(&mut store, "resolve_url")?;

        let first = call_resolve(&mut store, memory, &alloc, &free, &resolve_url, spec)?;

        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            let _ = call_resolve(&mut store, memory, &alloc, &free, &resolve_url, spec)?;
            samples.push(start.elapsed());
        }
        let (median, p95) = stats(samples);
        Ok(ResolveResult {
            output: first,
            median,
            p95,
        })
    }

    pub fn run_render(wasm_bytes: &[u8], spec: &Spec, iterations: usize) -> Result<RenderResult> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm_bytes)?;
        let state = HostState::new()?;
        let linker = build_linker::<HostState>(&engine)?;
        let mut store = Store::new(&engine, state);
        let instance = linker.instantiate(&mut store, &module)?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .context("wasm module is missing the `memory` export")?;
        let alloc: TypedFunc<u32, u32> = instance.get_typed_func(&mut store, "alloc")?;
        let free: TypedFunc<(u32, u32), ()> = instance.get_typed_func(&mut store, "free")?;
        let render_spec: TypedFunc<(u32, u32, u32, u32, u32, u32), i32> = instance
            .get_typed_func(&mut store, "render_spec")
            .map_err(|e| {
                anyhow::anyhow!("artifact marked full but does not export render_spec: {e}")
            })?;

        // Stash hot handles on the Store data so `fetch_bytes` can call
        // `alloc` reentrantly.
        store.data_mut().memory = Some(memory);
        store.data_mut().alloc = Some(alloc.clone());

        let mut fetch_samples = Vec::with_capacity(iterations);
        let mut render_samples = Vec::with_capacity(iterations);
        let mut total_samples = Vec::with_capacity(iterations);
        let mut last_output: Option<String> = None;

        for _ in 0..iterations {
            store.data_mut().last_fetch = None;
            let total_start = Instant::now();
            let out = call_render(&mut store, memory, &alloc, &free, &render_spec, spec)?;
            let total = total_start.elapsed();

            let fetch = store.data().last_fetch.unwrap_or(Duration::ZERO);
            let render = total.saturating_sub(fetch);
            fetch_samples.push(fetch);
            render_samples.push(render);
            total_samples.push(total);
            last_output = Some(out);
        }

        let output = last_output.context("render_spec produced no output")?;
        Ok(RenderResult {
            output,
            fetch_median: median_duration(fetch_samples),
            render_median: median_duration(render_samples),
            total_median: median_duration(total_samples),
        })
    }

    struct HostState {
        client: reqwest::blocking::Client,
        cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        last_fetch: Option<Duration>,
        memory: Option<Memory>,
        alloc: Option<TypedFunc<u32, u32>>,
    }

    impl HostState {
        fn new() -> Result<Self> {
            Ok(Self {
                client: blocking_http_client()?,
                cache: Arc::new(Mutex::new(HashMap::new())),
                last_fetch: None,
                memory: None,
                alloc: None,
            })
        }
    }

    trait MaybeHostState {
        fn host(&mut self) -> Option<&mut HostState>;
    }
    impl MaybeHostState for () {
        fn host(&mut self) -> Option<&mut HostState> {
            None
        }
    }
    impl MaybeHostState for HostState {
        fn host(&mut self) -> Option<&mut HostState> {
            Some(self)
        }
    }

    fn build_linker<T: MaybeHostState + 'static>(engine: &Engine) -> Result<Linker<T>> {
        let mut linker: Linker<T> = Linker::new(engine);
        linker.func_wrap(
            "env",
            "fetch_bytes",
            |mut caller: Caller<'_, T>,
             url_ptr: u32,
             url_len: u32,
             buf_ptr_out: u32,
             buf_len_out: u32|
             -> i32 {
                fetch_bytes_impl(&mut caller, url_ptr, url_len, buf_ptr_out, buf_len_out)
                    .unwrap_or(-1)
            },
        )?;
        Ok(linker)
    }

    fn fetch_bytes_impl<T: MaybeHostState>(
        caller: &mut Caller<'_, T>,
        url_ptr: u32,
        url_len: u32,
        buf_ptr_out: u32,
        buf_len_out: u32,
    ) -> Result<i32> {
        let memory = caller.data_mut().host().and_then(|s| s.memory).context(
            "fetch_bytes invoked without a HostState (minimal artifact should not call it)",
        )?;
        let alloc_fn = caller
            .data_mut()
            .host()
            .and_then(|s| s.alloc.clone())
            .context("alloc handle missing from HostState")?;

        let mut url_bytes = vec![0u8; url_len as usize];
        memory.read(&*caller, url_ptr as usize, &mut url_bytes)?;
        let url = String::from_utf8(url_bytes).context("fetch_bytes: url not utf-8")?;

        let start = Instant::now();
        let body = {
            let cache = caller
                .data_mut()
                .host()
                .map(|s| Arc::clone(&s.cache))
                .expect("host state");
            let guard = cache.lock().unwrap();
            if let Some(cached) = guard.get(&url) {
                cached.clone()
            } else {
                let client = caller
                    .data_mut()
                    .host()
                    .map(|s| s.client.clone())
                    .expect("host state");
                drop(guard);
                let resp = client.get(&url).send().context("fetch_bytes: GET failed")?;
                let status = resp.status();
                if !status.is_success() {
                    return Ok(i32::from(status.as_u16()));
                }
                let bytes = resp.bytes().context("fetch_bytes: read body failed")?;
                let vec = bytes.to_vec();
                cache.lock().unwrap().insert(url.clone(), vec.clone());
                vec
            }
        };
        if let Some(state) = caller.data_mut().host() {
            state.last_fetch = Some(start.elapsed());
        }

        let body_len =
            u32::try_from(body.len()).context("fetch_bytes: response body too large for wasm")?;
        let buf_ptr = alloc_fn.call(&mut *caller, body_len)?;
        if buf_ptr == 0 {
            return Ok(-1);
        }
        memory.write(&mut *caller, usize::try_from(buf_ptr)?, &body)?;
        memory.write(
            &mut *caller,
            usize::try_from(buf_ptr_out)?,
            &buf_ptr.to_le_bytes(),
        )?;
        memory.write(
            &mut *caller,
            usize::try_from(buf_len_out)?,
            &body_len.to_le_bytes(),
        )?;
        Ok(0)
    }

    fn call_resolve<T>(
        store: &mut Store<T>,
        memory: Memory,
        alloc: &TypedFunc<u32, u32>,
        free: &TypedFunc<(u32, u32), ()>,
        resolve_url: &TypedFunc<(u32, u32, u32, u32, u32, u32), u32>,
        spec: &Spec,
    ) -> Result<Option<String>> {
        let spec_len =
            u32::try_from(spec.spec.len()).context("spec too large for wasm linear memory")?;
        let spec_ptr = alloc.call(&mut *store, spec_len)?;
        if spec_ptr == 0 {
            bail!("alloc(spec) returned null");
        }
        memory.write(
            &mut *store,
            usize::try_from(spec_ptr)?,
            spec.spec.as_bytes(),
        )?;

        let (target_ptr, target_len) = if let Some(target) = spec.target {
            let target_len =
                u32::try_from(target.len()).context("target too large for wasm linear memory")?;
            let ptr = alloc.call(&mut *store, target_len)?;
            if ptr == 0 {
                bail!("alloc(target) returned null");
            }
            memory.write(&mut *store, usize::try_from(ptr)?, target.as_bytes())?;
            (ptr, target_len)
        } else {
            (0, 0)
        };

        let out_ptr = alloc.call(&mut *store, OUT_CAP)?;
        if out_ptr == 0 {
            bail!("alloc(out) returned null");
        }

        let n = resolve_url.call(
            &mut *store,
            (spec_ptr, spec_len, target_ptr, target_len, out_ptr, OUT_CAP),
        )?;
        let output = if n == 0 {
            None
        } else {
            let mut buf = vec![0u8; usize::try_from(n)?];
            memory.read(&*store, usize::try_from(out_ptr)?, &mut buf)?;
            Some(String::from_utf8(buf).context("resolve_url returned non-UTF8 bytes")?)
        };

        free.call(&mut *store, (spec_ptr, spec_len))?;
        if target_len > 0 {
            free.call(&mut *store, (target_ptr, target_len))?;
        }
        free.call(&mut *store, (out_ptr, OUT_CAP))?;
        Ok(output)
    }

    fn call_render(
        store: &mut Store<HostState>,
        memory: Memory,
        alloc: &TypedFunc<u32, u32>,
        free: &TypedFunc<(u32, u32), ()>,
        render_spec: &TypedFunc<(u32, u32, u32, u32, u32, u32), i32>,
        spec: &Spec,
    ) -> Result<String> {
        let spec_len =
            u32::try_from(spec.spec.len()).context("spec too large for wasm linear memory")?;
        let spec_ptr = alloc.call(&mut *store, spec_len)?;
        if spec_ptr == 0 {
            bail!("alloc(spec) returned null");
        }
        memory.write(
            &mut *store,
            usize::try_from(spec_ptr)?,
            spec.spec.as_bytes(),
        )?;

        let (target_ptr, target_len) = if let Some(target) = spec.target {
            let target_len =
                u32::try_from(target.len()).context("target too large for wasm linear memory")?;
            let ptr = alloc.call(&mut *store, target_len)?;
            if ptr == 0 {
                bail!("alloc(target) returned null");
            }
            memory.write(&mut *store, usize::try_from(ptr)?, target.as_bytes())?;
            (ptr, target_len)
        } else {
            (0, 0)
        };

        // Reserve two u32 slots to receive the output ptr/len.
        let slot_ptr = alloc.call(&mut *store, 8)?;
        if slot_ptr == 0 {
            bail!("alloc(slots) returned null");
        }
        memory.write(&mut *store, usize::try_from(slot_ptr)?, &[0u8; 8])?;

        let rc = render_spec.call(
            &mut *store,
            (
                spec_ptr,
                spec_len,
                target_ptr,
                target_len,
                slot_ptr,
                slot_ptr + 4,
            ),
        )?;
        if rc != 0 {
            bail!("render_spec failed with code {rc}");
        }

        let mut slots = [0u8; 8];
        memory.read(&*store, usize::try_from(slot_ptr)?, &mut slots)?;
        let out_ptr = u32::from_le_bytes(slots[0..4].try_into().unwrap());
        let out_len = u32::from_le_bytes(slots[4..8].try_into().unwrap());
        let mut buf = vec![0u8; usize::try_from(out_len)?];
        memory.read(&*store, usize::try_from(out_ptr)?, &mut buf)?;
        let md = String::from_utf8(buf).context("render_spec returned non-UTF8 bytes")?;

        free.call(&mut *store, (spec_ptr, spec_len))?;
        if target_len > 0 {
            free.call(&mut *store, (target_ptr, target_len))?;
        }
        free.call(&mut *store, (slot_ptr, 8))?;
        free.call(&mut *store, (out_ptr, out_len))?;

        Ok(md)
    }
}

#[cfg(feature = "wasmer")]
mod wasmer_runner {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    use wasmer::{
        AsStoreMut, Function, FunctionEnv, FunctionEnvMut, Instance, Memory, Module, Store,
        TypedFunction, imports,
    };

    #[derive(Clone)]
    struct HostEnv {
        client: reqwest::blocking::Client,
        cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        last_fetch: Arc<Mutex<Option<Duration>>>,
        memory: Arc<Mutex<Option<Memory>>>,
        alloc: Arc<Mutex<Option<TypedFunction<u32, u32>>>>,
    }

    pub fn run_resolve(wasm_bytes: &[u8], spec: &Spec, iterations: usize) -> Result<ResolveResult> {
        let mut store = Store::default();
        let module = Module::new(&store, wasm_bytes)?;
        let instance = Instance::new(&mut store, &module, &imports! {})?;
        let memory = instance.exports.get_memory("memory")?.clone();
        let alloc: TypedFunction<u32, u32> =
            instance.exports.get_typed_function(&store, "alloc")?;
        let free: TypedFunction<(u32, u32), ()> =
            instance.exports.get_typed_function(&store, "free")?;
        let resolve_url: TypedFunction<(u32, u32, u32, u32, u32, u32), u32> =
            instance.exports.get_typed_function(&store, "resolve_url")?;

        let first = call_resolve(&mut store, &memory, &alloc, &free, &resolve_url, spec)?;

        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            let _ = call_resolve(&mut store, &memory, &alloc, &free, &resolve_url, spec)?;
            samples.push(start.elapsed());
        }
        let (median, p95) = stats(samples);
        Ok(ResolveResult {
            output: first,
            median,
            p95,
        })
    }

    pub fn run_render(wasm_bytes: &[u8], spec: &Spec, iterations: usize) -> Result<RenderResult> {
        let mut store = Store::default();
        let module = Module::new(&store, wasm_bytes)?;

        let env = HostEnv {
            client: blocking_http_client()?,
            cache: Arc::new(Mutex::new(HashMap::new())),
            last_fetch: Arc::new(Mutex::new(None)),
            memory: Arc::new(Mutex::new(None)),
            alloc: Arc::new(Mutex::new(None)),
        };
        let fn_env = FunctionEnv::new(&mut store, env.clone());

        let fetch_bytes = Function::new_typed_with_env(
            &mut store,
            &fn_env,
            |mut caller: FunctionEnvMut<HostEnv>,
             url_ptr: u32,
             url_len: u32,
             buf_ptr_out: u32,
             buf_len_out: u32|
             -> i32 {
                fetch_bytes_impl(&mut caller, url_ptr, url_len, buf_ptr_out, buf_len_out)
                    .unwrap_or(-1)
            },
        );

        let imports = imports! {
            "env" => { "fetch_bytes" => fetch_bytes },
        };
        let instance = Instance::new(&mut store, &module, &imports)?;
        let memory = instance.exports.get_memory("memory")?.clone();
        let alloc: TypedFunction<u32, u32> =
            instance.exports.get_typed_function(&store, "alloc")?;
        let free: TypedFunction<(u32, u32), ()> =
            instance.exports.get_typed_function(&store, "free")?;
        let render_spec: TypedFunction<(u32, u32, u32, u32, u32, u32), i32> = instance
            .exports
            .get_typed_function(&store, "render_spec")
            .context("artifact marked full but does not export render_spec")?;

        *env.memory.lock().unwrap() = Some(memory.clone());
        *env.alloc.lock().unwrap() = Some(alloc.clone());

        let mut fetch_samples = Vec::with_capacity(iterations);
        let mut render_samples = Vec::with_capacity(iterations);
        let mut total_samples = Vec::with_capacity(iterations);
        let mut last_output: Option<String> = None;

        for _ in 0..iterations {
            *env.last_fetch.lock().unwrap() = None;
            let total_start = Instant::now();
            let out = call_render(&mut store, &memory, &alloc, &free, &render_spec, spec)?;
            let total = total_start.elapsed();
            let fetch = env.last_fetch.lock().unwrap().unwrap_or(Duration::ZERO);
            let render = total.saturating_sub(fetch);
            fetch_samples.push(fetch);
            render_samples.push(render);
            total_samples.push(total);
            last_output = Some(out);
        }

        let output = last_output.context("render_spec produced no output")?;
        Ok(RenderResult {
            output,
            fetch_median: median_duration(fetch_samples),
            render_median: median_duration(render_samples),
            total_median: median_duration(total_samples),
        })
    }

    fn fetch_bytes_impl(
        caller: &mut FunctionEnvMut<HostEnv>,
        url_ptr: u32,
        url_len: u32,
        buf_ptr_out: u32,
        buf_len_out: u32,
    ) -> Result<i32> {
        let (memory, alloc_fn) = {
            let env = caller.data();
            (
                env.memory
                    .lock()
                    .unwrap()
                    .clone()
                    .context("memory not set")?,
                env.alloc.lock().unwrap().clone().context("alloc not set")?,
            )
        };

        let view = memory.view(&*caller);
        let mut url_bytes = vec![0u8; url_len as usize];
        view.read(url_ptr as u64, &mut url_bytes)?;
        let url = String::from_utf8(url_bytes).context("fetch_bytes: url not utf-8")?;

        let start = Instant::now();
        let body = {
            let (cache, client) = {
                let env = caller.data();
                (Arc::clone(&env.cache), env.client.clone())
            };
            let cached = cache.lock().unwrap().get(&url).cloned();
            if let Some(v) = cached {
                v
            } else {
                let resp = client.get(&url).send().context("fetch_bytes: GET failed")?;
                let status = resp.status();
                if !status.is_success() {
                    return Ok(status.as_u16() as i32);
                }
                let bytes = resp
                    .bytes()
                    .context("fetch_bytes: read body failed")?
                    .to_vec();
                cache.lock().unwrap().insert(url.clone(), bytes.clone());
                bytes
            }
        };
        *caller.data().last_fetch.lock().unwrap() = Some(start.elapsed());

        let buf_ptr = alloc_fn.call(&mut caller.as_store_mut(), body.len() as u32)?;
        if buf_ptr == 0 {
            return Ok(-1);
        }
        let view = memory.view(&*caller);
        view.write(buf_ptr as u64, &body)?;
        view.write(buf_ptr_out as u64, &buf_ptr.to_le_bytes())?;
        view.write(buf_len_out as u64, &(body.len() as u32).to_le_bytes())?;
        Ok(0)
    }

    fn call_resolve(
        store: &mut Store,
        memory: &Memory,
        alloc: &TypedFunction<u32, u32>,
        free: &TypedFunction<(u32, u32), ()>,
        resolve_url: &TypedFunction<(u32, u32, u32, u32, u32, u32), u32>,
        spec: &Spec,
    ) -> Result<Option<String>> {
        let spec_len = spec.spec.len() as u32;
        let spec_ptr = alloc.call(&mut *store, spec_len)?;
        if spec_ptr == 0 {
            bail!("alloc(spec) returned null");
        }
        memory
            .view(&*store)
            .write(spec_ptr as u64, spec.spec.as_bytes())?;

        let (target_ptr, target_len) = if let Some(t) = spec.target {
            let p = alloc.call(&mut *store, t.len() as u32)?;
            if p == 0 {
                bail!("alloc(target) returned null");
            }
            memory.view(&*store).write(p as u64, t.as_bytes())?;
            (p, t.len() as u32)
        } else {
            (0, 0)
        };

        let out_ptr = alloc.call(&mut *store, OUT_CAP)?;
        if out_ptr == 0 {
            bail!("alloc(out) returned null");
        }

        let n = resolve_url.call(
            &mut *store,
            spec_ptr,
            spec_len,
            target_ptr,
            target_len,
            out_ptr,
            OUT_CAP,
        )?;
        let output = if n == 0 {
            None
        } else {
            let mut buf = vec![0u8; n as usize];
            memory.view(&*store).read(out_ptr as u64, &mut buf)?;
            Some(String::from_utf8(buf).context("resolve_url returned non-UTF8 bytes")?)
        };

        free.call(&mut *store, spec_ptr, spec_len)?;
        if target_len > 0 {
            free.call(&mut *store, target_ptr, target_len)?;
        }
        free.call(&mut *store, out_ptr, OUT_CAP)?;
        Ok(output)
    }

    fn call_render(
        store: &mut Store,
        memory: &Memory,
        alloc: &TypedFunction<u32, u32>,
        free: &TypedFunction<(u32, u32), ()>,
        render_spec: &TypedFunction<(u32, u32, u32, u32, u32, u32), i32>,
        spec: &Spec,
    ) -> Result<String> {
        let spec_len = spec.spec.len() as u32;
        let spec_ptr = alloc.call(&mut *store, spec_len)?;
        if spec_ptr == 0 {
            bail!("alloc(spec) returned null");
        }
        memory
            .view(&*store)
            .write(spec_ptr as u64, spec.spec.as_bytes())?;

        let (target_ptr, target_len) = if let Some(t) = spec.target {
            let p = alloc.call(&mut *store, t.len() as u32)?;
            if p == 0 {
                bail!("alloc(target) returned null");
            }
            memory.view(&*store).write(p as u64, t.as_bytes())?;
            (p, t.len() as u32)
        } else {
            (0, 0)
        };

        let slot_ptr = alloc.call(&mut *store, 8)?;
        if slot_ptr == 0 {
            bail!("alloc(slots) returned null");
        }
        memory.view(&*store).write(slot_ptr as u64, &[0u8; 8])?;

        let rc = render_spec.call(
            &mut *store,
            spec_ptr,
            spec_len,
            target_ptr,
            target_len,
            slot_ptr,
            slot_ptr + 4,
        )?;
        if rc != 0 {
            bail!("render_spec failed with code {rc}");
        }

        let mut slots = [0u8; 8];
        memory.view(&*store).read(slot_ptr as u64, &mut slots)?;
        let out_ptr = u32::from_le_bytes(slots[0..4].try_into().unwrap());
        let out_len = u32::from_le_bytes(slots[4..8].try_into().unwrap());
        let mut buf = vec![0u8; out_len as usize];
        memory.view(&*store).read(out_ptr as u64, &mut buf)?;
        let md = String::from_utf8(buf).context("render_spec returned non-UTF8 bytes")?;

        free.call(&mut *store, spec_ptr, spec_len)?;
        if target_len > 0 {
            free.call(&mut *store, target_ptr, target_len)?;
        }
        free.call(&mut *store, slot_ptr, 8)?;
        free.call(&mut *store, out_ptr, out_len)?;
        Ok(md)
    }
}
