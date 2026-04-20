//! WASM-friendly C ABI over the `md_docrs_proxy` pure pipeline.
//!
//! Exposes the same `alloc` / `free` / `resolve_url` trio as the Zig build
//! (`zig/lib/wasm.zig`) so both modules are drop-in interchangeable behind
//! the same Cloudflare Worker. Adds `render_markdown`, which takes an already
//! decoded rustdoc JSON blob plus a spec and returns rendered Markdown — the
//! piece the Zig side will eventually mirror for the full-pipeline benchmark.

use md_docrs_proxy::ItemSpec;
#[cfg(feature = "render")]
use md_docrs_proxy::{render, resolve};
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
    if len == 0 { None } else { Layout::array::<u8>(len).ok() }
}

/// Allocate `len` bytes inside the WASM linear memory. Returns null on failure
/// or when `len == 0`. Caller must free with `free(ptr, len)`.
#[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    let Some(layout) = layout_for(len as usize) else {
        return ptr::null_mut();
    };
    // SAFETY: layout has non-zero size (checked above).
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
    let Some(layout) = layout_for(len as usize) else {
        return;
    };
    unsafe { dealloc(ptr, layout) };
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
    let spec_bytes = unsafe { slice::from_raw_parts(spec_ptr, spec_len as usize) };
    let Ok(spec_str) = std::str::from_utf8(spec_bytes) else {
        return 0;
    };
    let Ok(mut spec) = ItemSpec::parse(spec_str) else {
        return 0;
    };

    if target_len > 0 {
        let t = unsafe { slice::from_raw_parts(target_ptr, target_len as usize) };
        let Ok(t_str) = std::str::from_utf8(t) else {
            return 0;
        };
        spec = spec.with_target(Some(t_str.to_string()));
    }

    let url = match spec.target.as_deref() {
        Some(t) => format!(
            "{DOCS_RS_BASE}/crate/{}/{}/{}/json/{FORMAT_VERSION}.zst",
            spec.crate_name, spec.version, t
        ),
        None => format!(
            "{DOCS_RS_BASE}/crate/{}/{}/json/{FORMAT_VERSION}.zst",
            spec.crate_name, spec.version
        ),
    };

    let bytes = url.as_bytes();
    if bytes.len() > out_cap as usize {
        return 0;
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out_ptr, bytes.len());
    }
    bytes.len() as u32
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
/// Output length is not known in advance (varies with the item's doc size)
/// so we allocate here rather than asking the caller to guess a bound.
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
    let json = unsafe { slice::from_raw_parts(json_ptr, json_len as usize) };
    let spec_bytes = unsafe { slice::from_raw_parts(spec_ptr, spec_len as usize) };

    let Ok(spec_str) = std::str::from_utf8(spec_bytes) else {
        return ptr::null_mut();
    };
    let Ok(mut spec) = ItemSpec::parse(spec_str) else {
        return ptr::null_mut();
    };
    if target_len > 0 {
        let t = unsafe { slice::from_raw_parts(target_ptr, target_len as usize) };
        let Ok(t_str) = std::str::from_utf8(t) else {
            return ptr::null_mut();
        };
        spec = spec.with_target(Some(t_str.to_string()));
    }

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
    // SAFETY: non-zero layout.
    let out = unsafe { rust_alloc(layout) };
    if out.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
        *len_out = bytes.len() as u32;
    }
    out
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
                spec.len() as u32,
                ptr::null(),
                0,
                out.as_mut_ptr(),
                out.len() as u32,
            )
        };
        assert_eq!(
            std::str::from_utf8(&out[..n as usize]).unwrap(),
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
                spec.len() as u32,
                target.as_ptr(),
                target.len() as u32,
                out.as_mut_ptr(),
                out.len() as u32,
            )
        };
        assert_eq!(
            std::str::from_utf8(&out[..n as usize]).unwrap(),
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
                spec.len() as u32,
                ptr::null(),
                0,
                out.as_mut_ptr(),
                out.len() as u32,
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
                spec.len() as u32,
                ptr::null(),
                0,
                out.as_mut_ptr(),
                out.len() as u32,
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
