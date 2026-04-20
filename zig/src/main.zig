const std = @import("std");
const spec_mod = @import("spec.zig");
const url_mod = @import("url.zig");

const ItemSpec = spec_mod.ItemSpec;

const usage =
    \\usage: md-docrs-zig <SPEC> [--target TRIPLE]
    \\
    \\Spec grammar: crate[@version][::path::to::item]
    \\
    \\v0 prints the rustdoc JSON URL that the Rust implementation would fetch.
    \\The full fetch + render pipeline lives in the Rust crate; this binary is
    \\kept lean so it can be compiled to WebAssembly for a size comparison.
    \\
;

pub fn main() !void {
    var gpa: std.heap.GeneralPurposeAllocator(.{}) = .{};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    var stdout_buf: [4096]u8 = undefined;
    var stdout_file = std.fs.File.stdout().writer(&stdout_buf);
    const stdout = &stdout_file.interface;

    var stderr_buf: [4096]u8 = undefined;
    var stderr_file = std.fs.File.stderr().writer(&stderr_buf);
    const stderr = &stderr_file.interface;

    var spec_arg: ?[]const u8 = null;
    var target: ?[]const u8 = null;

    var i: usize = 1;
    while (i < args.len) : (i += 1) {
        const a = args[i];
        if (std.mem.eql(u8, a, "--target")) {
            i += 1;
            if (i >= args.len) {
                try stderr.writeAll("error: --target requires a value\n");
                try stderr.flush();
                std.process.exit(2);
            }
            target = args[i];
        } else if (std.mem.eql(u8, a, "-h") or std.mem.eql(u8, a, "--help")) {
            try stdout.writeAll(usage);
            try stdout.flush();
            return;
        } else if (spec_arg == null) {
            spec_arg = a;
        } else {
            try stderr.print("error: unexpected argument: {s}\n", .{a});
            try stderr.flush();
            std.process.exit(2);
        }
    }

    const raw = spec_arg orelse {
        try stderr.writeAll(usage);
        try stderr.flush();
        std.process.exit(2);
    };

    var spec = ItemSpec.parse(allocator, raw) catch |err| {
        try stderr.print("invalid spec '{s}': {s}\n", .{ raw, @errorName(err) });
        try stderr.flush();
        std.process.exit(2);
    };
    defer spec.deinit();
    spec.target = target;

    const url = try url_mod.buildUrl(
        allocator,
        url_mod.DEFAULT_BASE,
        spec.crate_name,
        spec.version,
        spec.target,
        url_mod.FORMAT_VERSION,
    );
    defer allocator.free(url);

    try stdout.print("{s}\n", .{url});
    try stdout.flush();
}

test {
    _ = @import("spec.zig");
    _ = @import("url.zig");
}
