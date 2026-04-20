/// Pure, allocator-driven core: parse a spec and emit the docs.rs URL.
/// Shared by the native CLI (`cli.zig`) and the WASM entry (`wasm.zig`).
const std = @import("std");
const spec_mod = @import("spec.zig");
const url_mod = @import("url.zig");

pub const DEFAULT_BASE = url_mod.DEFAULT_BASE;
pub const FORMAT_VERSION = url_mod.FORMAT_VERSION;

/// Parse `raw_spec`, combine with `target`, write the resolved URL into `out`.
/// Returns the number of bytes written, or 0 on any failure (invalid spec,
/// OOM, or `out` too small). The output is *not* null-terminated.
pub fn resolveUrl(
    allocator: std.mem.Allocator,
    raw_spec: []const u8,
    target: ?[]const u8,
    out: []u8,
) u32 {
    var spec = spec_mod.ItemSpec.parse(allocator, raw_spec) catch return 0;
    defer spec.deinit();
    spec.target = target;

    const url = url_mod.buildUrl(
        allocator,
        DEFAULT_BASE,
        spec.crate_name,
        spec.version,
        spec.target,
        FORMAT_VERSION,
    ) catch return 0;
    defer allocator.free(url);

    if (url.len > out.len) return 0;
    @memcpy(out[0..url.len], url);
    return @intCast(url.len);
}

// Pull spec.zig and url.zig tests into the `zig build test` run.
test {
    std.testing.refAllDecls(@This());
    _ = @import("spec.zig");
    _ = @import("url.zig");
}

test "resolve bare crate" {
    var buf: [256]u8 = undefined;
    const n = resolveUrl(std.testing.allocator, "serde", null, &buf);
    try std.testing.expectEqualStrings(
        "https://docs.rs/crate/serde/latest/json/57.zst",
        buf[0..n],
    );
}

test "resolve pinned with target" {
    var buf: [256]u8 = undefined;
    const n = resolveUrl(
        std.testing.allocator,
        "tokio@1.52.1::sync::Mutex",
        "x86_64-unknown-linux-gnu",
        &buf,
    );
    try std.testing.expectEqualStrings(
        "https://docs.rs/crate/tokio/1.52.1/x86_64-unknown-linux-gnu/json/57.zst",
        buf[0..n],
    );
}

test "resolve invalid spec returns zero" {
    var buf: [256]u8 = undefined;
    try std.testing.expectEqual(
        @as(u32, 0),
        resolveUrl(std.testing.allocator, "1bad", null, &buf),
    );
}

test "resolve output buffer too small returns zero" {
    var buf: [8]u8 = undefined;
    try std.testing.expectEqual(
        @as(u32, 0),
        resolveUrl(std.testing.allocator, "serde", null, &buf),
    );
}
