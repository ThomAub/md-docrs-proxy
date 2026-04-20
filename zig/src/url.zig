const std = @import("std");

/// rustdoc JSON format version this build targets, matching the Rust crate's
/// `rustdoc_types::FORMAT_VERSION`. Bump when upgrading rustdoc-types.
pub const FORMAT_VERSION: u32 = 57;

pub const DEFAULT_BASE: []const u8 = "https://docs.rs";

/// Mirrors `build_url` in src/fetch.rs. Caller owns the returned slice.
pub fn buildUrl(
    allocator: std.mem.Allocator,
    base: []const u8,
    crate_name: []const u8,
    version: []const u8,
    target: ?[]const u8,
    format_version: ?u32,
) std.mem.Allocator.Error![]u8 {
    if (target) |t| {
        if (format_version) |fv| {
            return std.fmt.allocPrint(
                allocator,
                "{s}/crate/{s}/{s}/{s}/json/{d}.zst",
                .{ base, crate_name, version, t, fv },
            );
        }
        return std.fmt.allocPrint(
            allocator,
            "{s}/crate/{s}/{s}/{s}/json.zst",
            .{ base, crate_name, version, t },
        );
    }
    if (format_version) |fv| {
        return std.fmt.allocPrint(
            allocator,
            "{s}/crate/{s}/{s}/json/{d}.zst",
            .{ base, crate_name, version, fv },
        );
    }
    return std.fmt.allocPrint(
        allocator,
        "{s}/crate/{s}/{s}/json.zst",
        .{ base, crate_name, version },
    );
}

test "url basic" {
    const a = std.testing.allocator;
    const s = try buildUrl(a, DEFAULT_BASE, "serde", "latest", null, null);
    defer a.free(s);
    try std.testing.expectEqualStrings("https://docs.rs/crate/serde/latest/json.zst", s);
}

test "url with target" {
    const a = std.testing.allocator;
    const s = try buildUrl(a, DEFAULT_BASE, "serde", "latest", "x86_64-pc-windows-msvc", null);
    defer a.free(s);
    try std.testing.expectEqualStrings(
        "https://docs.rs/crate/serde/latest/x86_64-pc-windows-msvc/json.zst",
        s,
    );
}

test "url format pinned" {
    const a = std.testing.allocator;
    const s = try buildUrl(a, DEFAULT_BASE, "serde", "1.0.200", null, 57);
    defer a.free(s);
    try std.testing.expectEqualStrings("https://docs.rs/crate/serde/1.0.200/json/57.zst", s);
}

test "url format pinned with target" {
    const a = std.testing.allocator;
    const s = try buildUrl(a, DEFAULT_BASE, "tokio", "1.52.1", "x86_64-unknown-linux-gnu", 57);
    defer a.free(s);
    try std.testing.expectEqualStrings(
        "https://docs.rs/crate/tokio/1.52.1/x86_64-unknown-linux-gnu/json/57.zst",
        s,
    );
}
