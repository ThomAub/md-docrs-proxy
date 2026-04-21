//! Side-by-side comparison harness for the Zig and Rust `resolve_url` wasm
//! builds. Loads each .wasm inside an embedded wasmtime (or wasmer, via the
//! `wasmer` feature) and drives the same sequence of specs through both,
//! reporting artifact size, output parity, and per-call latency.
//!
//! Expects artifacts under `artifacts/` by default. Run `build.sh` first.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};

const OUT_CAP: u32 = 512;
const DEFAULT_ITERATIONS: usize = 200;

#[derive(Clone, Debug)]
struct Spec {
    spec: &'static str,
    target: Option<&'static str>,
}

const DEFAULT_SPECS: &[Spec] = &[
    Spec { spec: "serde", target: None },
    Spec { spec: "tokio@1.52.1::sync::Mutex", target: None },
    Spec { spec: "anyhow::Error", target: Some("x86_64-unknown-linux-gnu") },
    Spec { spec: "rustdoc-types@0.57::Crate", target: None },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Runtime {
    Wasmtime,
    #[cfg(feature = "wasmer")]
    Wasmer,
}

struct Args {
    runtime: Runtime,
    iterations: usize,
    artifacts_dir: PathBuf,
}

fn parse_args() -> Result<Args> {
    let mut runtime = Runtime::Wasmtime;
    let mut iterations = DEFAULT_ITERATIONS;
    let mut artifacts_dir = default_artifacts_dir();

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
                    "wasmer" => bail!(
                        "wasmer runtime not compiled in; rebuild with `--features wasmer`"
                    ),
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
            "--artifacts-dir" => {
                artifacts_dir = PathBuf::from(iter.next().context("--artifacts-dir expects a value")?);
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}"),
        }
    }

    Ok(Args { runtime, iterations, artifacts_dir })
}

fn print_help() {
    println!(
        "usage: wasm-compare [--runtime wasmtime|wasmer] [--iterations N] [--artifacts-dir PATH]\n\
         \n\
         Runs the same specs through each .wasm in the artifacts directory and\n\
         reports size, output, and median / p95 call latency.\n\
         \n\
         Artifacts expected under --artifacts-dir (default: wasm/artifacts):\n\
           zig.wasm          - Zig ReleaseSmall resolve_url build\n\
           rust-minimal.wasm - Rust wasm-release, --no-default-features\n\
           rust-full.wasm    - Rust wasm-release, full pipeline (optional)\n\
         \n\
         Any of the three may be missing; the harness just skips them."
    );
}

fn default_artifacts_dir() -> PathBuf {
    // When invoked via `cargo run -p md-docrs-wasm-compare`, CARGO_MANIFEST_DIR
    // points at wasm/, so `artifacts/` is right there.
    if let Some(dir) = option_env!("CARGO_MANIFEST_DIR") {
        return Path::new(dir).join("artifacts");
    }
    PathBuf::from("wasm/artifacts")
}

fn main() -> Result<()> {
    let args = parse_args()?;

    let artifacts = [
        ("zig", args.artifacts_dir.join("zig.wasm")),
        ("rust-minimal", args.artifacts_dir.join("rust-minimal.wasm")),
        ("rust-full", args.artifacts_dir.join("rust-full.wasm")),
    ];

    let present: Vec<_> = artifacts
        .iter()
        .filter(|(_, path)| path.exists())
        .collect();

    if present.is_empty() {
        bail!(
            "no .wasm artifacts found under {}\n\
             run `{}/build.sh` first, or pass --artifacts-dir",
            args.artifacts_dir.display(),
            env!("CARGO_MANIFEST_DIR"),
        );
    }

    println!("runtime:    {:?}", args.runtime);
    println!("iterations: {}", args.iterations);
    println!("artifacts:  {}", args.artifacts_dir.display());
    println!();

    println!("{:<14} {:>10}", "artifact", "bytes");
    println!("{:-<14} {:->10}", "", "");
    for (label, path) in &present {
        let meta = fs::metadata(path)?;
        println!("{:<14} {:>10}", label, meta.len());
    }
    println!();

    for spec in DEFAULT_SPECS {
        println!(
            "spec: {}{}",
            spec.spec,
            spec.target.map(|t| format!(" (target={t})")).unwrap_or_default(),
        );
        println!(
            "{:<14}  {:<60}  {:>10}  {:>10}",
            "artifact", "output", "median µs", "p95 µs"
        );
        println!("{:-<14}  {:-<60}  {:->10}  {:->10}", "", "", "", "");
        for (label, path) in &present {
            let bytes = fs::read(path)?;
            match run_spec(args.runtime, &bytes, spec, args.iterations) {
                Ok(result) => {
                    let output = result
                        .output
                        .as_deref()
                        .unwrap_or("<resolve_url returned 0>");
                    let shown = if output.len() > 60 {
                        format!("{}...", &output[..57])
                    } else {
                        output.to_string()
                    };
                    println!(
                        "{:<14}  {:<60}  {:>10}  {:>10}",
                        label,
                        shown,
                        result.median.as_micros(),
                        result.p95.as_micros(),
                    );
                }
                Err(e) => println!("{:<14}  error: {}", label, e),
            }
        }
        println!();
    }

    Ok(())
}

struct RunResult {
    output: Option<String>,
    median: Duration,
    p95: Duration,
}

fn run_spec(runtime: Runtime, wasm_bytes: &[u8], spec: &Spec, iterations: usize) -> Result<RunResult> {
    match runtime {
        Runtime::Wasmtime => wasmtime_runner::run(wasm_bytes, spec, iterations),
        #[cfg(feature = "wasmer")]
        Runtime::Wasmer => wasmer_runner::run(wasm_bytes, spec, iterations),
    }
}

fn stats(mut samples: Vec<Duration>) -> (Duration, Duration) {
    samples.sort();
    let median = samples[samples.len() / 2];
    let p95_idx = ((samples.len() as f64) * 0.95) as usize;
    let p95 = samples[p95_idx.min(samples.len() - 1)];
    (median, p95)
}

mod wasmtime_runner {
    use super::*;
    use wasmtime::{Engine, Instance, Memory, Module, Store, TypedFunc};

    pub fn run(wasm_bytes: &[u8], spec: &Spec, iterations: usize) -> Result<RunResult> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm_bytes)?;
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .context("wasm module is missing the `memory` export")?;
        let alloc: TypedFunc<u32, u32> =
            instance.get_typed_func(&mut store, "alloc")?;
        let free: TypedFunc<(u32, u32), ()> =
            instance.get_typed_func(&mut store, "free")?;
        let resolve_url: TypedFunc<(u32, u32, u32, u32, u32, u32), u32> =
            instance.get_typed_func(&mut store, "resolve_url")?;

        let first = call(&mut store, memory, &alloc, &free, &resolve_url, spec)?;

        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            let _ = call(&mut store, memory, &alloc, &free, &resolve_url, spec)?;
            samples.push(start.elapsed());
        }
        let (median, p95) = stats(samples);

        Ok(RunResult { output: first, median, p95 })
    }

    fn call(
        store: &mut Store<()>,
        memory: Memory,
        alloc: &TypedFunc<u32, u32>,
        free: &TypedFunc<(u32, u32), ()>,
        resolve_url: &TypedFunc<(u32, u32, u32, u32, u32, u32), u32>,
        spec: &Spec,
    ) -> Result<Option<String>> {
        let spec_len = spec.spec.len() as u32;
        let spec_ptr = alloc.call(&mut *store, spec_len)?;
        if spec_ptr == 0 {
            bail!("alloc(spec) returned null");
        }
        memory.write(&mut *store, spec_ptr as usize, spec.spec.as_bytes())?;

        let (target_ptr, target_len) = if let Some(t) = spec.target {
            let p = alloc.call(&mut *store, t.len() as u32)?;
            if p == 0 {
                bail!("alloc(target) returned null");
            }
            memory.write(&mut *store, p as usize, t.as_bytes())?;
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
            (spec_ptr, spec_len, target_ptr, target_len, out_ptr, OUT_CAP),
        )?;

        let output = if n == 0 {
            None
        } else {
            let mut buf = vec![0u8; n as usize];
            memory.read(&*store, out_ptr as usize, &mut buf)?;
            Some(String::from_utf8(buf).context("resolve_url returned non-UTF8 bytes")?)
        };

        free.call(&mut *store, (spec_ptr, spec_len))?;
        if target_len > 0 {
            free.call(&mut *store, (target_ptr, target_len))?;
        }
        free.call(&mut *store, (out_ptr, OUT_CAP))?;

        Ok(output)
    }
}

#[cfg(feature = "wasmer")]
mod wasmer_runner {
    use super::*;
    use wasmer::{Instance, Memory, Module, Store, TypedFunction, imports};

    pub fn run(wasm_bytes: &[u8], spec: &Spec, iterations: usize) -> Result<RunResult> {
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

        let first = call(&mut store, &memory, &alloc, &free, &resolve_url, spec)?;

        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            let _ = call(&mut store, &memory, &alloc, &free, &resolve_url, spec)?;
            samples.push(start.elapsed());
        }
        let (median, p95) = stats(samples);

        Ok(RunResult { output: first, median, p95 })
    }

    fn call(
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
}
