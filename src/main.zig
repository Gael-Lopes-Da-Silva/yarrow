const std = @import("std");

const Literal = union(enum) {
    void,
    int: i64,
    float: f64,
    str: []const u8,
};

const TokenType = enum {
    comma,
    dot,
    colon,
    l_paren,
    r_paren,

    plus,
    minus,
    multiplication,
    division,
    euclidian,
    reminder,
    power,

    identifier,
    string,
    integer,
    float,

    equal,
    equal_equal,
    not_equal,
    greater,
    greater_equal,
    less,
    less_equal,

    bitwise_and,
    bitwise_or,
    bitwise_xor,
    l_shift,
    r_shift,

    keyword_and,
    keyword_or,
    keyword_not,
    keyword_function,
    keyword_return,
    keyword_if,
    keyword_else,
    keyword_match,
    keyword_case,
    keyword_default,
    keyword_end,
    keyword_do,
    keyword_with,
    keyword_while,
    keyword_const,
    keyword_mutable,
    keyword_struct,
    keyword_union,
    keyword_enum,
    keyword_public,
    keyword_private,
    keyword_protected,
    keyword_try,
    keyword_discard,
    keyword_require,
};

const Location = struct {
    line: usize,
    start: usize,
    end: usize,
};

const Token = struct {
    type: TokenType,
    lexeme: []const u8,
    literal: Literal,
    location: Location,
};

const Keyword = struct {
    name: []const u8,
    token: TokenType,
};

const Tokenizer = struct {
    start: usize,
    current: usize,
    line: usize,
    source: []const u8,
    keywords: []const Keyword,
    tokens: std.ArrayList(Token),

    fn init(alloc: std.mem.Allocator) !Tokenizer {
        return Tokenizer{
            .start = 0,
            .current = 0,
            .line = 1,
            .tokens = std.ArrayList(Token).init(alloc),
            .source = "",
            .keywords = &[_]Keyword{
                .{ .name = "and", .token = .keyword_and },
                .{ .name = "case", .token = .keyword_case },
                .{ .name = "const", .token = .keyword_const },
                .{ .name = "default", .token = .keyword_default },
                .{ .name = "discard", .token = .keyword_discard },
                .{ .name = "do", .token = .keyword_do },
                .{ .name = "else", .token = .keyword_else },
                .{ .name = "end", .token = .keyword_end },
                .{ .name = "enum", .token = .keyword_enum },
                .{ .name = "function", .token = .keyword_function },
                .{ .name = "if", .token = .keyword_if },
                .{ .name = "match", .token = .keyword_match },
                .{ .name = "mutable", .token = .keyword_mutable },
                .{ .name = "not", .token = .keyword_not },
                .{ .name = "or", .token = .keyword_or },
                .{ .name = "private", .token = .keyword_private },
                .{ .name = "protected", .token = .keyword_protected },
                .{ .name = "public", .token = .keyword_public },
                .{ .name = "require", .token = .keyword_require },
                .{ .name = "return", .token = .keyword_return },
                .{ .name = "struct", .token = .keyword_struct },
                .{ .name = "try", .token = .keyword_try },
                .{ .name = "union", .token = .keyword_union },
                .{ .name = "while", .token = .keyword_while },
                .{ .name = "with", .token = .keyword_with },
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
            if (std.ascii.eqlIgnoreCase(keyword.name, key)) return keyword.token;
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
        return char;
    }

    fn match(self: *Tokenizer, expected: u8) bool {
        if (self.isAtEnd() or self.peek() != expected) return false;
        self.current += 1;
        return true;
    }

    fn addToken(self: *Tokenizer, token_type: TokenType, literal: Literal) !void {
        try self.tokens.append(Token{
            .type = token_type,
            .literal = literal,
            .lexeme = self.source[self.start..self.current],
            .location = .{
                .line = self.line,
                .start = self.start,
                .end = self.current,
            },
        });
    }

    fn scanToken(self: *Tokenizer) !void {
        switch (self.advance()) {
            '#' => {
                while (self.peek() != '\n' and !self.isAtEnd()) _ = self.advance();
            },

            '(' => try self.addToken(.l_paren, Literal{ .void = {} }),
            ')' => try self.addToken(.r_paren, Literal{ .void = {} }),
            ',' => try self.addToken(.comma, Literal{ .void = {} }),
            '.' => try self.addToken(.dot, Literal{ .void = {} }),
            ':' => try self.addToken(.colon, Literal{ .void = {} }),
            '-' => try self.addToken(.minus, Literal{ .void = {} }),
            '+' => try self.addToken(.plus, Literal{ .void = {} }),
            '%' => try self.addToken(.reminder, Literal{ .void = {} }),

            '&' => try self.addToken(.bitwise_and, Literal{ .void = {} }),
            '|' => try self.addToken(.bitwise_or, Literal{ .void = {} }),
            '^' => try self.addToken(.bitwise_xor, Literal{ .void = {} }),

            '*' => try self.addToken(
                if (self.match('*')) .power else .multiplication,
                Literal{ .void = {} },
            ),
            '/' => try self.addToken(
                if (self.match('/')) .euclidian else .division,
                Literal{ .void = {} },
            ),
            '=' => try self.addToken(
                if (self.match('=')) .equal_equal else .equal,
                Literal{ .void = {} },
            ),
            '<' => try self.addToken(
                if (self.match('=')) .less_equal else if (self.match('<')) .l_shift else .less,
                Literal{ .void = {} },
            ),

            '>' => try self.addToken(
                if (self.match('=')) .greater_equal else if (self.match('>')) .r_shift else .greater,
                Literal{ .void = {} },
            ),

            '!' => if (self.match('=')) try self.addToken(.not_equal, Literal{ .void = {} }),

            ' ' => {},
            '\t' => {},
            '\n' => self.line += 1,
            '\r' => self.line += 1,

            '"' => {
                while (self.peek() != '"' and !self.isAtEnd()) {
                    if (self.peek() == '\n') {
                        std.log.err("Invalid string literal !\n{d}", .{self.line});
                        self.line += 1;
                        return;
                    }

                    _ = self.advance();
                }

                if (self.isAtEnd()) {
                    std.log.err("Unterminated string !\n{d}", .{self.line});
                    return;
                }

                _ = self.advance();
                const value = self.source[self.start + 1 .. self.current - 1];
                try self.addToken(.string, Literal{ .str = value });
            },

            '0'...'9' => {
                while (std.ascii.isDigit(self.peek())) _ = self.advance();

                if (self.peek() == '.' and std.ascii.isDigit(self.peekNext())) {
                    _ = self.advance();

                    while (std.ascii.isDigit(self.peek())) _ = self.advance();

                    const float = try std.fmt.parseFloat(f64, self.source[self.start..self.current]);
                    try self.addToken(.float, .{ .float = float });
                } else {
                    const int = try std.fmt.parseInt(i64, self.source[self.start..self.current], 10);
                    try self.addToken(.integer, .{ .int = int });
                }
            },

            'A'...'Z', 'a'...'z', '_' => {
                while (std.ascii.isAlphanumeric(self.peek()) or self.peek() == '_') _ = self.advance();

                var token_type: TokenType = .identifier;
                const keyword = self.lookupKeyword(self.source[self.start..self.current]);

                if (keyword) |value| {
                    token_type = value;
                }

                try self.addToken(token_type, Literal{ .void = {} });
            },

            else => {
                std.log.err("Unexpected character !\n{d}", .{self.line});
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
        const file = std.fs.cwd().openFile(path, .{}) catch |err| {
            std.log.err("Failed to open file !\n{s}: {s}", .{ path, @errorName(err) });
            return;
        };
        defer file.close();

        const source = file.reader().readAllAlloc(self.alloc, std.math.maxInt(usize)) catch |err| {
            std.log.err("Failed to read file !\n{s}: {s}", .{ path, @errorName(err) });
            return;
        };
        defer self.alloc.free(source);

        var scanner = try Tokenizer.init(self.alloc);
        defer scanner.deinit();

        const tokens = try scanner.scan(source);
        _ = tokens;
    }

    fn scanPrompt(self: *Cli) !void {
        const in = std.io.getStdIn().reader();
        const out = std.io.getStdOut().writer();

        var scanner = try Tokenizer.init(self.alloc);
        defer scanner.deinit();

        var running = true;
        while (running) {
            try out.print("> ", .{});
            const source = try in.readUntilDelimiterAlloc(self.alloc, '\n', std.math.maxInt(usize));
            defer self.alloc.free(source);

            if (std.ascii.eqlIgnoreCase(source, "quit")) {
                running = false;
                break;
            }

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
            std.log.warn("Memory leak detected !", .{});
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
        std.log.err("Too many arguments provided !", .{});
        return;
    }
}
