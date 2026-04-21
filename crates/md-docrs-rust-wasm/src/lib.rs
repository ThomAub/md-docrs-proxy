//! WASM-friendly C ABI over the `md_docrs_proxy` pure pipeline.
//!
//! Exposes the same `alloc` / `free` / `resolve_url` trio as the Zig build
//! (`zig/lib/wasm.zig`) so both modules are drop-in interchangeable behind
//! the same Cloudflare Worker. Adds `render_markdown` (host-fed JSON) and
//! `render_spec` (host-imported fetch + in-WASM zstd decode + render) for
//! the full pipeline comparison.
//!
//! ## Error codes returned by `render_spec`
//! The Zig full WASM build uses the same values.
//!
//! | code | meaning |
//! | ---- | ------- |
//! |  0   | success (ptr + len written to out-slots) |
//! | -1   | alloc failure |
//! | -2   | host `fetch_bytes` returned non-zero |
//! | -3   | zstd decode failure |
//! | -4   | JSON parse failure |
//! | -5   | spec parse / resolve miss / URL too long |
//! | -6   | output pointer write failure |

use md_docrs_core::ItemSpec;
#[cfg(feature = "render")]
use md_docrs_core::{render, resolve};
#[cfg(feature = "render")]
use rustdoc_types::Crate;
use std::alloc::{Layout, alloc as rust_alloc, dealloc};
use std::ptr;
use std::slice;

/// rustdoc JSON format version this build targets. Kept in sync with the Zig
/// build and the `rustdoc-types` dependency.
const FORMAT_VERSION: u32 = 57;
const DOCS_RS_BASE: &str = "https://docs.rs";

fn layout_for(len: usize) -> Option<Layout> {
    if len == 0 {
        None
    } else {
        Layout::array::<u8>(len).ok()
    }
}

/// Allocate `len` bytes inside the WASM linear memory. Returns null on failure
/// or when `len == 0`. Caller must free with `free(ptr, len)`.
#[must_use]
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    let Ok(len) = usize::try_from(len) else {
        return ptr::null_mut();
    };
    let Some(layout) = layout_for(len) else {
        return ptr::null_mut();
    };
    unsafe { rust_alloc(layout) }
}

/// Free memory previously returned by `alloc`. `len` must match the allocation.
///
/// # Safety
/// `ptr` must be a pointer returned by `alloc` with the exact same `len`.
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub unsafe extern "C" fn free(ptr: *mut u8, len: u32) {
    if ptr.is_null() {
        return;
    }
    let Ok(len) = usize::try_from(len) else {
        return;
    };
    let Some(layout) = layout_for(len) else {
        return;
    };
    unsafe { dealloc(ptr, layout) };
}

fn build_docs_rs_url(spec: &ItemSpec) -> String {
    match spec.target.as_deref() {
        Some(t) => format!(
            "{DOCS_RS_BASE}/crate/{}/{}/{}/json/{FORMAT_VERSION}.zst",
            spec.crate_name, spec.version, t
        ),
        None => format!(
            "{DOCS_RS_BASE}/crate/{}/{}/json/{FORMAT_VERSION}.zst",
            spec.crate_name, spec.version
        ),
    }
}

fn parse_spec_with_target(
    spec_ptr: *const u8,
    spec_len: u32,
    target_ptr: *const u8,
    target_len: u32,
) -> Option<ItemSpec> {
    let spec_len = usize::try_from(spec_len).ok()?;
    let spec_bytes = unsafe { slice::from_raw_parts(spec_ptr, spec_len) };
    let spec = std::str::from_utf8(spec_bytes).ok()?;
    let mut item_spec = ItemSpec::parse(spec).ok()?;
    if target_len > 0 {
        let target_len = usize::try_from(target_len).ok()?;
        let target_bytes = unsafe { slice::from_raw_parts(target_ptr, target_len) };
        let target = std::str::from_utf8(target_bytes).ok()?;
        item_spec = item_spec.with_target(Some(target.to_string()));
    }
    Some(item_spec)
}

/// Parse `spec` and write the docs.rs rustdoc JSON URL into `out_ptr`.
/// `target_len == 0` means "use the default host target".
/// Returns bytes written, or 0 on any failure.
///
/// # Safety
/// All four (ptr, len) pairs must describe valid readable / writable slices
/// inside WASM linear memory.
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub unsafe extern "C" fn resolve_url(
    spec_ptr: *const u8,
    spec_len: u32,
    target_ptr: *const u8,
    target_len: u32,
    out_ptr: *mut u8,
    out_cap: u32,
) -> u32 {
    let Some(spec) = parse_spec_with_target(spec_ptr, spec_len, target_ptr, target_len) else {
        return 0;
    };
    let Ok(out_cap) = usize::try_from(out_cap) else {
        return 0;
    };
    let url = build_docs_rs_url(&spec);
    let bytes = url.as_bytes();
    if bytes.len() > out_cap {
        return 0;
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out_ptr, bytes.len());
    }
    u32::try_from(bytes.len()).unwrap_or(0)
}

/// Render rustdoc JSON to Markdown.
///
/// The caller owns `json_ptr` / `spec_ptr` / `target_ptr`. On success this
/// returns a pointer to a fresh buffer (allocated with `alloc`) containing
/// the Markdown, and writes the byte length to `*len_out`. The caller is
/// responsible for `free(ptr, *len_out)`.
///
/// Returns null on any error (invalid spec, JSON parse failure, resolve
/// miss, alloc failure). `*len_out` is only meaningful when the return
/// value is non-null.
///
/// # Safety
/// All input (ptr, len) pairs must describe valid readable slices. `len_out`
/// must be a writable `u32`.
#[cfg(feature = "render")]
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub unsafe extern "C" fn render_markdown(
    json_ptr: *const u8,
    json_len: u32,
    spec_ptr: *const u8,
    spec_len: u32,
    target_ptr: *const u8,
    target_len: u32,
    len_out: *mut u32,
) -> *mut u8 {
    let Ok(json_len) = usize::try_from(json_len) else {
        return ptr::null_mut();
    };
    let json = unsafe { slice::from_raw_parts(json_ptr, json_len) };
    let Some(spec) = parse_spec_with_target(spec_ptr, spec_len, target_ptr, target_len) else {
        return ptr::null_mut();
    };

    let Ok(krate) = serde_json::from_slice::<Crate>(json) else {
        return ptr::null_mut();
    };
    let Ok(resolved) = resolve::resolve(&krate, &spec) else {
        return ptr::null_mut();
    };
    let md = render::render(&krate, &resolved, &spec);

    let bytes = md.as_bytes();
    let Some(layout) = layout_for(bytes.len()) else {
        return ptr::null_mut();
    };
    let out = unsafe { rust_alloc(layout) };
    if out.is_null() {
        return ptr::null_mut();
    }
    let Ok(len_out_value) = u32::try_from(bytes.len()) else {
        unsafe { dealloc(out, layout) };
        return ptr::null_mut();
    };
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
        *len_out = len_out_value;
    }
    out
}

// ---------------------------------------------------------------------------
// Full-pipeline entry. Imports `env.fetch_bytes` from the host and decodes
// the zstd-compressed rustdoc JSON in-module.
// ---------------------------------------------------------------------------

#[cfg(all(feature = "render", feature = "fetch"))]
unsafe extern "C" {
    /// Host-provided: perform an HTTP GET against `url` and hand back the raw
    /// response body (still zstd-compressed, as served by docs.rs).
    ///
    /// The host allocates the destination buffer inside WASM memory by
    /// calling the exported `alloc`, writes the body into it, then stores the
    /// pointer at `*buf_ptr_out` and the length at `*buf_len_out`.
    /// Returns 0 on success, non-zero on any HTTP / transport failure (no
    /// buffer written in that case).
    fn fetch_bytes(
        url_ptr: *const u8,
        url_len: u32,
        buf_ptr_out: *mut u32,
        buf_len_out: *mut u32,
    ) -> i32;
}

#[cfg(all(feature = "render", feature = "fetch"))]
fn zstd_decode(input: &[u8]) -> Option<Vec<u8>> {
    use ruzstd::decoding::StreamingDecoder;
    use std::io::Read;

    let mut decoder = StreamingDecoder::new(input).ok()?;
    let mut out = Vec::with_capacity(input.len() * 4);
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

/// Full pipeline: parse spec → fetch via host → zstd decode → JSON parse →
/// resolve → render. On success writes `(ptr, len)` of an `alloc`-owned
/// Markdown buffer into `*buf_ptr_out` / `*buf_len_out` and returns 0.
///
/// See module docs for error codes.
///
/// # Safety
/// All (ptr, len) pairs must describe valid slices; both `*_out` pointers
/// must reference writable `u32` slots inside WASM linear memory.
#[cfg(all(feature = "render", feature = "fetch"))]
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub unsafe extern "C" fn render_spec(
    spec_ptr: *const u8,
    spec_len: u32,
    target_ptr: *const u8,
    target_len: u32,
    buf_ptr_out: *mut u32,
    buf_len_out: *mut u32,
) -> i32 {
    let Some(spec) = parse_spec_with_target(spec_ptr, spec_len, target_ptr, target_len) else {
        return -5;
    };
    let url = build_docs_rs_url(&spec);
    let Ok(url_len) = u32::try_from(url.len()) else {
        return -5;
    };

    let mut resp_ptr: u32 = 0;
    let mut resp_len: u32 = 0;
    let rc = unsafe { fetch_bytes(url.as_ptr(), url_len, &mut resp_ptr, &mut resp_len) };
    if rc != 0 {
        return -2;
    }
    if resp_ptr == 0 || resp_len == 0 {
        return -2;
    }

    let Ok(resp_ptr_usize) = usize::try_from(resp_ptr) else {
        unsafe { free(resp_ptr as *mut u8, resp_len) };
        return -3;
    };
    let Ok(resp_len_usize) = usize::try_from(resp_len) else {
        unsafe { free(resp_ptr as *mut u8, resp_len) };
        return -3;
    };

    // Take ownership of the host-written buffer; free it once decoded.
    let compressed = unsafe { slice::from_raw_parts(resp_ptr_usize as *const u8, resp_len_usize) };
    let decoded = zstd_decode(compressed);
    unsafe { free(resp_ptr as *mut u8, resp_len) };
    let Some(json) = decoded else {
        return -3;
    };

    let Ok(krate) = serde_json::from_slice::<Crate>(&json) else {
        return -4;
    };
    drop(json);

    let Ok(resolved) = resolve::resolve(&krate, &spec) else {
        return -5;
    };
    let md = render::render(&krate, &resolved, &spec);

    let bytes = md.as_bytes();
    let Some(layout) = layout_for(bytes.len()) else {
        return -1;
    };
    let out = unsafe { rust_alloc(layout) };
    if out.is_null() {
        return -1;
    }
    let Ok(out_ptr_value) = u32::try_from(out as usize) else {
        unsafe { dealloc(out, layout) };
        return -6;
    };
    let Ok(out_len_value) = u32::try_from(bytes.len()) else {
        unsafe { dealloc(out, layout) };
        return -6;
    };
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
        *buf_ptr_out = out_ptr_value;
        *buf_len_out = out_len_value;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_url_basic() {
        let spec = b"serde";
        let mut out = [0u8; 128];
        let n = unsafe {
            resolve_url(
                spec.as_ptr(),
                u32::try_from(spec.len()).unwrap(),
                ptr::null(),
                0,
                out.as_mut_ptr(),
                u32::try_from(out.len()).unwrap(),
            )
        };
        assert_eq!(
            std::str::from_utf8(&out[..usize::try_from(n).unwrap()]).unwrap(),
            "https://docs.rs/crate/serde/latest/json/57.zst",
        );
    }

    #[test]
    fn resolve_url_with_target_and_pinned_version() {
        let spec = b"tokio@1.52.1::sync::Mutex";
        let target = b"x86_64-unknown-linux-gnu";
        let mut out = [0u8; 256];
        let n = unsafe {
            resolve_url(
                spec.as_ptr(),
                u32::try_from(spec.len()).unwrap(),
                target.as_ptr(),
                u32::try_from(target.len()).unwrap(),
                out.as_mut_ptr(),
                u32::try_from(out.len()).unwrap(),
            )
        };
        assert_eq!(
            std::str::from_utf8(&out[..usize::try_from(n).unwrap()]).unwrap(),
            "https://docs.rs/crate/tokio/1.52.1/x86_64-unknown-linux-gnu/json/57.zst",
        );
    }

    #[test]
    fn resolve_url_bad_spec_returns_zero() {
        let spec = b"1bad";
        let mut out = [0u8; 128];
        let n = unsafe {
            resolve_url(
                spec.as_ptr(),
                u32::try_from(spec.len()).unwrap(),
                ptr::null(),
                0,
                out.as_mut_ptr(),
                u32::try_from(out.len()).unwrap(),
            )
        };
        assert_eq!(n, 0);
    }

    #[test]
    fn resolve_url_output_too_small() {
        let spec = b"serde";
        let mut out = [0u8; 8];
        let n = unsafe {
            resolve_url(
                spec.as_ptr(),
                u32::try_from(spec.len()).unwrap(),
                ptr::null(),
                0,
                out.as_mut_ptr(),
                u32::try_from(out.len()).unwrap(),
            )
        };
        assert_eq!(n, 0);
    }

    #[test]
    fn alloc_and_free_roundtrip() {
        let ptr = alloc(64);
        assert!(!ptr.is_null());
        unsafe {
            *ptr = 42;
            assert_eq!(*ptr, 42);
            free(ptr, 64);
        }
    }
}
