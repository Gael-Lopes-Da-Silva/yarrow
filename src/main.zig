const std = @import("std");

const TokenType = enum {
    // Delimiters
    L_paren,
    R_paren,
    Comma,

    // Operators
    Dot,
    Plus,
    Minus,
    Multiplication,
    Division,
    Euclidian,
    Reminder,
    Power,

    Equal_equal,
    Not_equal,
    Greater,
    Greater_equal,
    Less,
    Less_equal,

    Bitwise_and,
    Bitwise_or,
    Bitwise_xor,
    L_shift,
    R_shift,

    // Literals
    Identifier,
    String,
    Integer,
    Float,

    // Keywords
    And,
    Call,
    Case,
    Catch,
    Const,
    Default,
    Defer,
    Discard,
    Do,
    Else,
    End,
    Enum,
    Function,
    If,
    Match,
    Mutable,
    Not,
    Or,
    Private,
    Protected,
    Public,
    Require,
    Return,
    Struct,
    Try,
    Union,
    While,
    With,

    Drop,
    Dup,
    Over,
    Rot,
    Set,
    Swap,
};

const Token = struct {
    type: TokenType,
    lexeme: []const u8,
    line: usize,
    start: usize,
    end: usize,
};

const Tokenizer = struct {
    const Keyword = struct {
        name: []const u8,
        token: TokenType,
    };

    start: usize,
    current: usize,
    line: usize,
    column: usize,
    path: []const u8,
    source: []const u8,
    keywords: []const Keyword,
    tokens: std.ArrayList(Token),

    fn init(alloc: std.mem.Allocator) !Tokenizer {
        return Tokenizer{
            .start = 0,
            .current = 0,
            .line = 1,
            .column = 1,
            .tokens = std.ArrayList(Token).init(alloc),
            .path = "",
            .source = "",
            .keywords = &[_]Keyword{
                .{ .name = "and", .token = .And },
                .{ .name = "call", .token = .Call },
                .{ .name = "case", .token = .Case },
                .{ .name = "catch", .token = .Try },
                .{ .name = "const", .token = .Const },
                .{ .name = "default", .token = .Default },
                .{ .name = "defer", .token = .Defer },
                .{ .name = "discard", .token = .Discard },
                .{ .name = "do", .token = .Do },
                .{ .name = "else", .token = .Else },
                .{ .name = "end", .token = .End },
                .{ .name = "enum", .token = .Enum },
                .{ .name = "function", .token = .Function },
                .{ .name = "if", .token = .If },
                .{ .name = "match", .token = .Match },
                .{ .name = "mutable", .token = .Mutable },
                .{ .name = "not", .token = .Not },
                .{ .name = "or", .token = .Or },
                .{ .name = "private", .token = .Private },
                .{ .name = "protected", .token = .Protected },
                .{ .name = "public", .token = .Public },
                .{ .name = "require", .token = .Require },
                .{ .name = "return", .token = .Return },
                .{ .name = "struct", .token = .Struct },
                .{ .name = "try", .token = .Try },
                .{ .name = "union", .token = .Union },
                .{ .name = "while", .token = .While },
                .{ .name = "with", .token = .With },

                .{ .name = "drop", .token = .Drop },
                .{ .name = "dup", .token = .Dup },
                .{ .name = "over", .token = .Over },
                .{ .name = "rot", .token = .Rot },
                .{ .name = "set", .token = .Set },
                .{ .name = "swap", .token = .Swap },
            },
        };
    }

    fn deinit(self: *Tokenizer) void {
        self.tokens.deinit();
    }

    fn scan(self: *Tokenizer, source: []const u8) !std.ArrayList(Token) {
        self.source = source;
        try self.scanTokens();
        return self.tokens;
    }

    fn lookupKeyword(self: *Tokenizer, key: []const u8) ?TokenType {
        for (self.keywords) |keyword| {
            if (std.mem.eql(u8, keyword.name, key)) return keyword.token;
        }

        return null;
    }

    fn isAtEnd(self: Tokenizer) bool {
        return self.current >= self.source.len;
    }

    fn peek(self: Tokenizer) u8 {
        return self.source[self.current];
    }

    fn peekNext(self: Tokenizer) u8 {
        return self.source[self.current + 1];
    }

    fn advance(self: *Tokenizer) u8 {
        const char = self.peek();
        self.current += 1;
        self.column += 1;
        return char;
    }

    fn match(self: *Tokenizer, expected: u8) bool {
        if (self.isAtEnd() or self.peek() != expected) return false;
        self.current += 1;
        self.column += 1;
        return true;
    }

    fn addToken(self: *Tokenizer, token_type: TokenType) !void {
        try self.tokens.append(Token{
            .type = token_type,
            .lexeme = self.source[self.start..self.current],
            .line = self.line,
            .start = self.column,
            .end = self.current,
        });
    }

    fn scanToken(self: *Tokenizer) !void {
        switch (self.advance()) {
            '#' => {
                while (self.peek() != '\n' and !self.isAtEnd()) _ = self.advance();
            },

            '(' => try self.addToken(.L_paren),
            ')' => try self.addToken(.R_paren),
            ',' => try self.addToken(.Comma),
            '.' => try self.addToken(.Dot),
            '-' => try self.addToken(.Minus),
            '+' => try self.addToken(.Plus),
            '%' => try self.addToken(.Reminder),

            '&' => try self.addToken(.Bitwise_and),
            '|' => try self.addToken(.Bitwise_or),
            '^' => try self.addToken(.Bitwise_xor),

            '*' => try self.addToken(
                if (self.match('*')) .Power else .Multiplication,
            ),
            '/' => try self.addToken(
                if (self.match('/')) .Euclidian else .Division,
            ),
            '<' => try self.addToken(
                if (self.match('=')) .Less_equal else if (self.match('<')) .L_shift else .Less,
            ),
            '>' => try self.addToken(
                if (self.match('=')) .Greater_equal else if (self.match('>')) .R_shift else .Greater,
            ),

            '=' => if (self.match('=')) try self.addToken(.Equal_equal),
            '!' => if (self.match('=')) try self.addToken(.Not_equal),

            '\n', '\r' => {
                self.line += 1;
                self.column = 1;
            },

            '"' => {
                while (!self.isAtEnd() and self.peek() != '"') {
                    if (self.peek() == '\n') {
                        std.debug.print("[ERROR] Unterminated string literal !\n", .{});
                        if (!std.ascii.eqlIgnoreCase(self.path, "")) std.debug.print("| {s}:{d}:{d}\n", .{
                            self.path,
                            self.current,
                            self.column,
                        });
                        return;
                    }

                    _ = self.advance();
                }

                if (self.isAtEnd()) {
                    std.debug.print("[ERROR] Unterminated string literal !\n", .{});
                    if (!std.ascii.eqlIgnoreCase(self.path, "")) std.debug.print("| {s}:{d}:{d}\n", .{
                        self.path,
                        self.current,
                        self.column,
                    });
                    return;
                }

                _ = self.advance();
                try self.addToken(.String);
            },

            '0'...'9' => {
                while (!self.isAtEnd() and std.ascii.isDigit(self.peek())) _ = self.advance();

                if (!self.isAtEnd() and self.peek() == '.' and std.ascii.isDigit(self.peekNext())) {
                    _ = self.advance();

                    while (!self.isAtEnd() and std.ascii.isDigit(self.peek())) _ = self.advance();

                    try self.addToken(.Float);
                } else {
                    try self.addToken(.Integer);
                }
            },

            'A'...'Z', 'a'...'z', '_' => {
                while (!self.isAtEnd() and (std.ascii.isAlphanumeric(self.peek()) or self.peek() == '_')) _ = self.advance();

                var token_type: TokenType = .Identifier;
                const keyword = self.lookupKeyword(self.source[self.start..self.current]);

                if (keyword) |value| {
                    token_type = value;
                }

                try self.addToken(token_type);
            },

            ' ', '\t' => {},

            else => {
                std.debug.print("[ERROR] Unexpected character found !\n", .{});
                if (!std.ascii.eqlIgnoreCase(self.path, "")) std.debug.print("| {s}:{d}:{d}\n", .{
                    self.path,
                    self.current,
                    self.column,
                });
            },
        }
    }

    fn scanTokens(self: *Tokenizer) !void {
        while (!self.isAtEnd()) {
            self.start = self.current;
            try self.scanToken();
        }
    }
};

const Cli = struct {
    alloc: std.mem.Allocator,

    fn init(alloc: std.mem.Allocator) !Cli {
        return Cli{
            .alloc = alloc,
        };
    }

    fn scanFile(self: *Cli, path: []const u8) !void {
        const file = std.fs.cwd().openFile(path, .{}) catch {
            std.debug.print("[ERROR] Failed to open file !\n", .{});
            return;
        };
        defer file.close();

        const source = file.reader().readAllAlloc(self.alloc, std.math.maxInt(usize)) catch {
            std.debug.print("[ERROR] Failed to read file !\n", .{});
            return;
        };
        defer self.alloc.free(source);

        var scanner = try Tokenizer.init(self.alloc);
        scanner.path = path;
        defer scanner.deinit();

        if (source.len > 0 and !std.ascii.eqlIgnoreCase(source, "")) {
            const tokens = try scanner.scan(source);
            _ = tokens;
        }
    }

    fn scanPrompt(self: *Cli) !void {
        const in = std.io.getStdIn().reader();
        const out = std.io.getStdOut().writer();

        while (true) {
            var scanner = try Tokenizer.init(self.alloc);
            defer scanner.deinit();

            try out.print("> ", .{});
            const source = try in.readUntilDelimiterAlloc(self.alloc, '\n', std.math.maxInt(usize));
            defer self.alloc.free(source);

            if (std.ascii.eqlIgnoreCase(source, "quit")) break;
            if (std.ascii.eqlIgnoreCase(source, "exit")) break;

            if (source.len > 0 and !std.ascii.eqlIgnoreCase(source, "")) {
                const tokens = try scanner.scan(source);
                _ = tokens;
            }
        }
    }
};

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{ .thread_safe = true }){};
    const alloc = gpa.allocator();
    defer {
        if (gpa.deinit() == .leak) {
            std.debug.print("[WARNING] Memory leak !\n", .{});
        }
    }

    const args = try std.process.argsAlloc(alloc);
    defer std.process.argsFree(alloc, args);

    var cli = try Cli.init(alloc);

    if (args.len == 1) {
        try cli.scanPrompt();
    } else if (args.len == 2) {
        try cli.scanFile(args[1]);
    } else {
        std.debug.print("[ERROR] Too many arguments provided !\n", .{});
    }
}
