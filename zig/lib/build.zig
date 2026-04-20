const std = @import("std");

pub fn build(b: *std.Build) void {
    // ── WASM target (Cloudflare Workers / any WebAssembly host) ─────────────
    //
    // Mirrors the zigflare recipe: wasm32-freestanding, ReleaseSmall, strip,
    // explicit export list, no entry point. This is the artifact we compare
    // against the Rust wasm build.
    const wasm_mod = b.createModule(.{
        .root_source_file = b.path("wasm.zig"),
        .target = b.resolveTargetQuery(.{
            .cpu_arch = .wasm32,
            .os_tag = .freestanding,
        }),
        .optimize = .ReleaseSmall,
        .strip = true,
    });
    wasm_mod.export_symbol_names = &.{ "alloc", "free", "resolve_url" };

    const wasm = b.addExecutable(.{
        .name = "md-docrs",
        .root_module = wasm_mod,
    });
    wasm.entry = .disabled;

    const install_wasm = b.addInstallArtifact(wasm, .{});
    b.getInstallStep().dependOn(&install_wasm.step);

    // ── Native CLI ──────────────────────────────────────────────────────────
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const cli_mod = b.createModule(.{
        .root_source_file = b.path("cli.zig"),
        .target = target,
        .optimize = optimize,
    });
    const cli = b.addExecutable(.{
        .name = "md-docrs-zig",
        .root_module = cli_mod,
    });

    // Install CLI under a named step so the default `zig build` only produces
    // the WASM artifact — keeps `npm run build:wasm` focused.
    const install_cli = b.addInstallArtifact(cli, .{});
    const cli_step = b.step("cli", "Build the native CLI");
    cli_step.dependOn(&install_cli.step);

    const run_cli = b.addRunArtifact(cli);
    run_cli.step.dependOn(&install_cli.step);
    if (b.args) |args| run_cli.addArgs(args);
    const run_step = b.step("run", "Run the native CLI");
    run_step.dependOn(&run_cli.step);

    // ── Unit tests (native) ─────────────────────────────────────────────────
    const test_mod = b.createModule(.{
        .root_source_file = b.path("resolve.zig"),
        .target = target,
        .optimize = optimize,
    });
    const tests = b.addTest(.{ .root_module = test_mod });
    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_tests.step);
}
