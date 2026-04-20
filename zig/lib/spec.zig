const std = @import("std");

pub const ParseError = error{
    Empty,
    BadCrateVersion,
    BadCrateName,
    BadPathSegment,
    OutOfMemory,
};

/// Parsed `crate[@version][::path::to::item]` reference.
/// Field slices borrow from the `raw` input passed to `parse`; keep it alive.
pub const ItemSpec = struct {
    crate_name: []const u8,
    version: []const u8,
    target: ?[]const u8 = null,
    path: [][]const u8,
    allocator: std.mem.Allocator,

    pub fn deinit(self: *ItemSpec) void {
        self.allocator.free(self.path);
    }

    pub fn isRoot(self: *const ItemSpec) bool {
        return self.path.len == 0;
    }

    pub fn parse(allocator: std.mem.Allocator, raw: []const u8) ParseError!ItemSpec {
        const trimmed = std.mem.trim(u8, raw, &std.ascii.whitespace);
        if (trimmed.len == 0) return ParseError.Empty;

        var head: []const u8 = trimmed;
        var rest: []const u8 = "";
        if (std.mem.indexOf(u8, trimmed, "::")) |i| {
            head = trimmed[0..i];
            rest = trimmed[i + 2 ..];
        }

        var path_list: std.ArrayList([]const u8) = .empty;
        errdefer path_list.deinit(allocator);

        if (rest.len > 0) {
            var it = std.mem.splitSequence(u8, rest, "::");
            while (it.next()) |seg| {
                if (!isValidIdent(seg)) return ParseError.BadPathSegment;
                try path_list.append(allocator, seg);
            }
        }

        var crate_name: []const u8 = head;
        var version: []const u8 = "latest";
        if (std.mem.indexOfScalar(u8, head, '@')) |i| {
            const c = head[0..i];
            const v = head[i + 1 ..];
            if (c.len == 0 or v.len == 0) return ParseError.BadCrateVersion;
            crate_name = c;
            version = v;
        }

        if (!isValidCrateName(crate_name)) return ParseError.BadCrateName;

        return ItemSpec{
            .crate_name = crate_name,
            .version = version,
            .path = try path_list.toOwnedSlice(allocator),
            .allocator = allocator,
        };
    }
};

const MAX_CRATE_NAME_LEN: usize = 64;

fn isValidCrateName(s: []const u8) bool {
    if (s.len == 0 or s.len > MAX_CRATE_NAME_LEN) return false;
    if (!std.ascii.isAlphabetic(s[0])) return false;
    for (s[1..]) |c| {
        if (!(std.ascii.isAlphanumeric(c) or c == '-' or c == '_')) return false;
    }
    return true;
}

fn isValidIdent(s: []const u8) bool {
    if (s.len == 0) return false;
    for (s) |c| {
        if (!(std.ascii.isAlphanumeric(c) or c == '_')) return false;
    }
    return true;
}

test "bare crate" {
    const a = std.testing.allocator;
    var s = try ItemSpec.parse(a, "serde");
    defer s.deinit();
    try std.testing.expectEqualStrings("serde", s.crate_name);
    try std.testing.expectEqualStrings("latest", s.version);
    try std.testing.expect(s.isRoot());
}

test "crate with version" {
    const a = std.testing.allocator;
    var s = try ItemSpec.parse(a, "serde@1.0.200");
    defer s.deinit();
    try std.testing.expectEqualStrings("1.0.200", s.version);
}

test "crate with path" {
    const a = std.testing.allocator;
    var s = try ItemSpec.parse(a, "serde::de::Deserialize");
    defer s.deinit();
    try std.testing.expectEqualStrings("serde", s.crate_name);
    try std.testing.expectEqual(@as(usize, 2), s.path.len);
    try std.testing.expectEqualStrings("de", s.path[0]);
    try std.testing.expectEqualStrings("Deserialize", s.path[1]);
}

test "crate with version and path" {
    const a = std.testing.allocator;
    var s = try ItemSpec.parse(a, "anyhow@~1::Error");
    defer s.deinit();
    try std.testing.expectEqualStrings("anyhow", s.crate_name);
    try std.testing.expectEqualStrings("~1", s.version);
    try std.testing.expectEqual(@as(usize, 1), s.path.len);
    try std.testing.expectEqualStrings("Error", s.path[0]);
}

test "rejects empty" {
    try std.testing.expectError(ParseError.Empty, ItemSpec.parse(std.testing.allocator, ""));
}

test "rejects bad at" {
    try std.testing.expectError(ParseError.BadCrateVersion, ItemSpec.parse(std.testing.allocator, "@1.0"));
    try std.testing.expectError(ParseError.BadCrateVersion, ItemSpec.parse(std.testing.allocator, "serde@"));
}

test "rejects invalid crate names" {
    const a = std.testing.allocator;
    try std.testing.expectError(ParseError.BadCrateName, ItemSpec.parse(a, "1serde"));
    try std.testing.expectError(ParseError.BadCrateName, ItemSpec.parse(a, "-serde"));
    try std.testing.expectError(ParseError.BadCrateName, ItemSpec.parse(a, "_serde"));
    try std.testing.expectError(ParseError.BadCrateName, ItemSpec.parse(a, "ser de"));
    try std.testing.expectError(ParseError.BadCrateName, ItemSpec.parse(a, "ser.de"));
    const long = "a" ** 65;
    try std.testing.expectError(ParseError.BadCrateName, ItemSpec.parse(a, long));
}

test "accepts valid crate names" {
    const a = std.testing.allocator;
    const max_name = "a" ** 64;
    inline for (.{ "serde", "serde_json", "tracing-subscriber", "a", max_name }) |name| {
        var s = try ItemSpec.parse(a, name);
        defer s.deinit();
    }
}
