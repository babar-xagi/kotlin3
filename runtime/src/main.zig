const std = @import("std");

pub const Version = "0.1.0";

pub fn main(init: std.process.Init.Minimal) !void {
    var it = init.args.iterate();

    // Skip executable name
    _ = it.next();

    const command = it.next() orelse {
        printUsage();
        return;
    };

    if (std.mem.eql(u8, command, "version") or std.mem.eql(u8, command, "-v") or std.mem.eql(u8, command, "--version")) {
        std.debug.print("Kotlin3 (k3) Toolchain v{s}\nNative driver (Zig 0.17.1) + Core Engine (Rust)\nTarget: Self-contained, zero-dependency Kotlin toolchain\n", .{Version});
        return;
    }

    if (std.mem.eql(u8, command, "build")) {
        std.debug.print("[k3] Building Kotlin project...\n", .{});
        std.debug.print("[k3] Done.\n", .{});
        return;
    }

    if (std.mem.eql(u8, command, "run")) {
        std.debug.print("[k3] Running Kotlin application...\n", .{});
        return;
    }

    if (std.mem.eql(u8, command, "test")) {
        std.debug.print("[k3] Running test suite...\n", .{});
        return;
    }

    printUsage();
}

fn printUsage() void {
    std.debug.print(
        \\Kotlin3 (k3) - Fast, Unified Kotlin Toolchain
        \\
        \\Usage:
        \\  k3 <command> [options]
        \\
        \\Commands:
        \\  build    Compile a Kotlin source file or project
        \\  run      Compile and run a Kotlin main function
        \\  test     Run unit and conformance tests
        \\  version  Display Kotlin3 toolchain version
        \\
        \\
    , .{});
}
