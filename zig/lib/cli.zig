/// Native CLI entry point. Thin wrapper over `resolve.resolveUrl` so the exact
/// same code path is exercised by `zig build cli -- <SPEC>` and the WASM worker.
const std = @import("std");
const Io = std.Io;
const resolve = @import("resolve.zig");

const usage =
    \\usage: md-docrs-zig <SPEC> [--target TRIPLE]
    \\
    \\Spec grammar: crate[@version][::path::to::item]
    \\
    \\Prints the rustdoc JSON URL that the Rust implementation would fetch.
    \\The full fetch + render pipeline lives in the Rust crate; this binary
    \\stays lean so the same logic can ship as a WebAssembly worker (see
    \\../src/index.ts) for a size comparison with the Rust WASM build.
    \\
;

pub fn main(init: std.process.Init) !u8 {
    const arena = init.arena.allocator();
    const io = init.io;

    const args = try init.minimal.args.toSlice(arena);

    var stdout_buf: [4096]u8 = undefined;
    var stdout_file_writer: Io.File.Writer = .init(.stdout(), io, &stdout_buf);
    const stdout = &stdout_file_writer.interface;

    var stderr_buf: [4096]u8 = undefined;
    var stderr_file_writer: Io.File.Writer = .init(.stderr(), io, &stderr_buf);
    const stderr = &stderr_file_writer.interface;

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
                return 2;
            }
            target = args[i];
        } else if (std.mem.eql(u8, a, "-h") or std.mem.eql(u8, a, "--help")) {
            try stdout.writeAll(usage);
            try stdout.flush();
            return 0;
        } else if (spec_arg == null) {
            spec_arg = a;
        } else {
            try stderr.print("error: unexpected argument: {s}\n", .{a});
            try stderr.flush();
            return 2;
        }
    }

    const raw = spec_arg orelse {
        try stderr.writeAll(usage);
        try stderr.flush();
        return 2;
    };

    var buf: [512]u8 = undefined;
    const n = resolve.resolveUrl(arena, raw, target, &buf);
    if (n == 0) {
        try stderr.print("error: could not resolve URL for '{s}'\n", .{raw});
        try stderr.flush();
        return 2;
    }

    try stdout.print("{s}\n", .{buf[0..n]});
    try stdout.flush();
    return 0;
}
