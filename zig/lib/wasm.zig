/// WASM entry point. Exports `alloc`, `free`, and `resolve_url` for use from
/// a Cloudflare Worker (or any other WebAssembly host).
///
/// Memory protocol mirrors zigflare:
///   1. Host calls `alloc(n)` to reserve input/output buffers inside WASM memory.
///   2. Host writes input bytes into those buffers from JS.
///   3. Host calls `resolve_url(...)`, which returns the number of bytes written
///      to the output buffer (0 on failure).
///   4. Host reads the output, then calls `free(ptr, len)` on each buffer.
///
/// See ../src/index.ts for the worker side and ../doc/memory.md for the model.
const std = @import("std");
const resolve = @import("resolve.zig");

const allocator = std.heap.wasm_allocator;

export fn alloc(len: u32) ?[*]u8 {
    const buf = allocator.alloc(u8, len) catch return null;
    return buf.ptr;
}

export fn free(ptr: [*]u8, len: u32) void {
    allocator.free(ptr[0..len]);
}

/// Parse `spec` and write the docs.rs rustdoc JSON URL into the output buffer.
/// `target_len == 0` means "use the default host target".
export fn resolve_url(
    spec_ptr: [*]const u8,
    spec_len: u32,
    target_ptr: [*]const u8,
    target_len: u32,
    out_ptr: [*]u8,
    out_cap: u32,
) u32 {
    const target: ?[]const u8 = if (target_len == 0) null else target_ptr[0..target_len];
    return resolve.resolveUrl(
        allocator,
        spec_ptr[0..spec_len],
        target,
        out_ptr[0..out_cap],
    );
}
