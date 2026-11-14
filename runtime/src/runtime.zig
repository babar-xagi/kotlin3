const std = @import("std");
const builtin = @import("builtin");

/// Runtime types supported by Kotlin3 Native Object Model
pub const K3Type = enum {
    Unit,
    Int,
    Long,
    Float,
    Double,
    String,
    Boolean,
    List,
    Object,
};

/// Tagged polymorphic container for Any Kotlin types
pub const K3Any = struct {
    tag: K3Type,
    int_val: i32 = 0,
    long_val: i64 = 0,
    double_val: f64 = 0.0,
    bool_val: bool = false,
    str_val: []const u8 = "",
    list_items: []const i64 = &[_]i64{},

    pub fn fromUnit() K3Any {
        return .{ .tag = .Unit };
    }

    pub fn fromInt(val: i32) K3Any {
        return .{ .tag = .Int, .int_val = val, .long_val = val };
    }

    pub fn fromLong(val: i64) K3Any {
        return .{ .tag = .Long, .long_val = val };
    }

    pub fn fromDouble(val: f64) K3Any {
        return .{ .tag = .Double, .double_val = val };
    }

    pub fn fromString(val: []const u8) K3Any {
        return .{ .tag = .String, .str_val = val };
    }

    pub fn fromBoolean(val: bool) K3Any {
        return .{ .tag = .Boolean, .bool_val = val };
    }

    pub fn isLong(self: K3Any) bool {
        return self.tag == .Long;
    }

    pub fn isString(self: K3Any) bool {
        return self.tag == .String;
    }

    pub fn isInt(self: K3Any) bool {
        return self.tag == .Int;
    }

    pub fn isDouble(self: K3Any) bool {
        return self.tag == .Double;
    }

    pub fn isBoolean(self: K3Any) bool {
        return self.tag == .Boolean;
    }

    pub fn equals(self: K3Any, other: K3Any) bool {
        if (self.tag != other.tag) return false;
        return switch (self.tag) {
            .Int => self.int_val == other.int_val,
            .Long => self.long_val == other.long_val,
            .Double => self.double_val == other.double_val,
            .String => std.mem.eql(u8, self.str_val, other.str_val),
            .Boolean => self.bool_val == other.bool_val,
            .Unit => true,
            else => false,
        };
    }

    pub fn to_string(self: K3Any, buf: []u8) []const u8 {
        return switch (self.tag) {
            .Int => std.fmt.bufPrint(buf, "{d}", .{self.int_val}) catch "",
            .Long => std.fmt.bufPrint(buf, "{d}", .{self.long_val}) catch "",
            .Double => std.fmt.bufPrint(buf, "{d}", .{self.double_val}) catch "",
            .String => self.str_val,
            .Boolean => if (self.bool_val) "true" else "false",
            .Unit => "kotlin.Unit",
            else => "Object",
        };
    }
};

/// Direct, zero-overhead OS write syscall to stdout
pub fn print(str: []const u8) void {
    if (str.len == 0) return;
    if (builtin.os.tag == .linux) {
        _ = std.os.linux.write(1, str.ptr, str.len);
    } else {
        _ = std.c.write(1, str.ptr, str.len);
    }
}

/// Print string followed by a newline
pub fn println(str: []const u8) void {
    print(str);
    print("\n");
}

pub fn print_int(val: i64) void {
    var buf: [32]u8 = undefined;
    const str = std.fmt.bufPrint(&buf, "{d}", .{val}) catch "";
    print(str);
}

pub fn println_int(val: i64) void {
    var buf: [32]u8 = undefined;
    const str = std.fmt.bufPrint(&buf, "{d}", .{val}) catch "";
    println(str);
}

/// Format a list of 64-bit integers as `[1, 2, 3]` without heap allocation
pub fn format_list(items: []const i64, buf: []u8) []const u8 {
    var pos: usize = 0;
    if (pos < buf.len) {
        buf[pos] = '[';
        pos += 1;
    }
    for (items, 0..) |item, i| {
        if (i > 0) {
            if (pos + 2 <= buf.len) {
                buf[pos] = ',';
                buf[pos + 1] = ' ';
                pos += 2;
            }
        }
        var num_buf: [32]u8 = undefined;
        const num_str = std.fmt.bufPrint(&num_buf, "{d}", .{item}) catch "";
        if (pos + num_str.len <= buf.len) {
            @memcpy(buf[pos .. pos + num_str.len], num_str);
            pos += num_str.len;
        }
    }
    if (pos < buf.len) {
        buf[pos] = ']';
        pos += 1;
    }
    return buf[0..pos];
}

/// Filter items matching a boolean predicate
pub fn filter(items: []const i64, predicate: *const fn (i64) bool, out_buf: []i64) []const i64 {
    var count: usize = 0;
    for (items) |x| {
        if (predicate(x)) {
            out_buf[count] = x;
            count += 1;
        }
    }
    return out_buf[0..count];
}

/// Map items via a transformation function
pub fn map(items: []const i64, mapper: *const fn (i64) i64, out_buf: []i64) []const i64 {
    for (items, 0..) |x, i| {
        out_buf[i] = mapper(x);
    }
    return out_buf[0..items.len];
}

/// Lightweight Continuation for Kotlin Coroutines CPS lowerings
pub const Continuation = struct {
    label: i32 = 0,
    result: ?*anyopaque = null,
    context: ?*anyopaque = null,

    pub fn resumeWith(self: *Continuation, res: *anyopaque) void {
        self.result = res;
    }
};

/// Global arena allocator for heap dynamic instances
pub var global_arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);

/// Allocate UTF-8 string copy on the arena heap
pub fn alloc_string(str: []const u8) []const u8 {
    const allocator = global_arena.allocator();
    return allocator.dupe(u8, str) catch str;
}

test "runtime K3Any equality and stringify" {
    const any_int = K3Any.fromInt(42);
    const any_long = K3Any.fromLong(42);
    try std.testing.expect(!any_int.equals(any_long));

    const any_str = K3Any.fromString("Kotlin3");
    var buf: [64]u8 = undefined;
    const s = any_str.to_string(&buf);
    try std.testing.expectEqualStrings("Kotlin3", s);
}

test "runtime list formatting" {
    const items = [_]i64{ 1, 2, 3, 4, 5 };
    var buf: [64]u8 = undefined;
    const s = format_list(&items, &buf);
    try std.testing.expectEqualStrings("[1, 2, 3, 4, 5]", s);
}
