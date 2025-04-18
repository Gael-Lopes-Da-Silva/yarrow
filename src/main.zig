const std = @import("std");

const Literal = union(enum) {
    void,
    int: i64,
    float: f64,
    str: []const u8,
};

const TokenType = enum {
    COMMA,
    DOT,
    COLON,
    LEFT_PAREN,
    RIGHT_PAREN,

    PLUS,
    MINUS,
    MULTIPLICATION,
    DIVISION,
    EUCLIDIAN,
    REMINDER,
    POWER,

    IDENTIFIER,
    STRING,
    INTEGER,
    FLOAT,

    EQUAL,
    EQUAL_EQUAL,
    NOT_EQUAL,
    GREATER,
    GREATER_EQUAL,
    LESS,
    LESS_EQUAL,

    TYPE,

    AND,
    OR,
    NOT,

    FUNCTION,
    RETURN,
    IF,
    ELSE,
    MATCH,
    CASE,
    DEFAULT,
    END,
    DO,
    WITH,
    WHILE,
    CONST,
    MUTABLE,
    STRUCT,
    UNION,
    ENUM,
    PUBLIC,
    PRIVATE,
    PROTECTED,
    TRY,
    DISCARD,
    REQUIRE,
};

const Token = struct {
    type: TokenType,
    lexeme: []const u8,
    literal: Literal,
    line: usize,
};

const Keyword = struct {
    name: []const u8,
    token: TokenType,
};

const Scanner = struct {
    start: usize,
    current: usize,
    line: usize,
    tokens: std.ArrayList(Token),
    source: []const u8,
    keywords: []const Keyword,

    pub fn init(alloc: std.mem.Allocator) !Scanner {
        return Scanner{
            .start = 0,
            .current = 0,
            .line = 1,
            .tokens = std.ArrayList(Token).init(alloc),
            .source = "",
            .keywords = &[_]Keyword{
                .{ .name = "and", .token = TokenType.AND },
                .{ .name = "or", .token = TokenType.OR },
                .{ .name = "not", .token = TokenType.NOT },
                .{ .name = "function", .token = TokenType.FUNCTION },
                .{ .name = "return", .token = TokenType.RETURN },
                .{ .name = "if", .token = TokenType.IF },
                .{ .name = "else", .token = TokenType.ELSE },
                .{ .name = "match", .token = TokenType.MATCH },
                .{ .name = "case", .token = TokenType.CASE },
                .{ .name = "default", .token = TokenType.DEFAULT },
                .{ .name = "end", .token = TokenType.END },
                .{ .name = "do", .token = TokenType.DO },
                .{ .name = "with", .token = TokenType.WITH },
                .{ .name = "while", .token = TokenType.WHILE },
                .{ .name = "const", .token = TokenType.CONST },
                .{ .name = "mutable", .token = TokenType.MUTABLE },
                .{ .name = "struct", .token = TokenType.STRUCT },
                .{ .name = "union", .token = TokenType.UNION },
                .{ .name = "enum", .token = TokenType.ENUM },
                .{ .name = "public", .token = TokenType.PUBLIC },
                .{ .name = "private", .token = TokenType.PRIVATE },
                .{ .name = "protected", .token = TokenType.PROTECTED },
                .{ .name = "try", .token = TokenType.TRY },
                .{ .name = "discard", .token = TokenType.DISCARD },
                .{ .name = "require", .token = TokenType.REQUIRE },

                .{ .name = "i8", .token = TokenType.TYPE },
                .{ .name = "i16", .token = TokenType.TYPE },
                .{ .name = "i32", .token = TokenType.TYPE },
                .{ .name = "i64", .token = TokenType.TYPE },
                .{ .name = "i128", .token = TokenType.TYPE },
                .{ .name = "u8", .token = TokenType.TYPE },
                .{ .name = "u16", .token = TokenType.TYPE },
                .{ .name = "u32", .token = TokenType.TYPE },
                .{ .name = "u64", .token = TokenType.TYPE },
                .{ .name = "u128", .token = TokenType.TYPE },
                .{ .name = "f16", .token = TokenType.TYPE },
                .{ .name = "f32", .token = TokenType.TYPE },
                .{ .name = "f64", .token = TokenType.TYPE },
                .{ .name = "f128", .token = TokenType.TYPE },
                .{ .name = "bool", .token = TokenType.TYPE },
                .{ .name = "void", .token = TokenType.TYPE },
                .{ .name = "string", .token = TokenType.TYPE },
                .{ .name = "array", .token = TokenType.TYPE },
                .{ .name = "vector", .token = TokenType.TYPE },
                .{ .name = "hashmap", .token = TokenType.TYPE },
                .{ .name = "stack", .token = TokenType.TYPE },
                .{ .name = "queue", .token = TokenType.TYPE },
                .{ .name = "ptr", .token = TokenType.TYPE },
                .{ .name = "isize", .token = TokenType.TYPE },
                .{ .name = "usize", .token = TokenType.TYPE },
                .{ .name = "c_char", .token = TokenType.TYPE },
                .{ .name = "c_short", .token = TokenType.TYPE },
                .{ .name = "c_ushort", .token = TokenType.TYPE },
                .{ .name = "c_int", .token = TokenType.TYPE },
                .{ .name = "c_uint", .token = TokenType.TYPE },
                .{ .name = "c_long", .token = TokenType.TYPE },
                .{ .name = "c_ulong", .token = TokenType.TYPE },
                .{ .name = "c_longlong", .token = TokenType.TYPE },
                .{ .name = "c_ulonglong", .token = TokenType.TYPE },
                .{ .name = "c_double", .token = TokenType.TYPE },
                .{ .name = "c_longdouble", .token = TokenType.TYPE },
            },
        };
    }

    pub fn deinit(self: *Scanner) void {
        self.tokens.deinit();
    }

    pub fn scan(self: *Scanner, source: []const u8) !std.ArrayList(Token) {
        self.source = source;
        try self.scanTokens();
        return self.tokens;
    }

    fn lookupKeyword(self: *Scanner, key: []const u8) ?TokenType {
        for (self.keywords) |keyword| {
            if (std.ascii.eqlIgnoreCase(keyword.name, key)) return keyword.token;
        }

        return null;
    }

    fn isAtEnd(self: Scanner) bool {
        return self.current >= self.source.len;
    }

    fn peek(self: Scanner) u8 {
        return self.source[self.current];
    }

    fn peekNext(self: Scanner) u8 {
        return self.source[self.current + 1];
    }

    fn advance(self: *Scanner) u8 {
        const char = self.peek();
        self.current += 1;
        return char;
    }

    fn match(self: *Scanner, expected: u8) bool {
        if (self.isAtEnd() or self.peek() != expected) return false;
        self.current += 1;
        return true;
    }

    fn addToken(self: *Scanner, token_type: TokenType, literal: Literal) !void {
        try self.tokens.append(Token{
            .type = token_type,
            .literal = literal,
            .line = self.line,
            .lexeme = self.source[self.start..self.current],
        });
    }

    fn scanToken(self: *Scanner) !void {
        switch (self.advance()) {
            '#' => {
                while (self.peek() != '\n' and !self.isAtEnd()) _ = self.advance();
            },

            '(' => try self.addToken(TokenType.LEFT_PAREN, Literal{ .void = {} }),
            ')' => try self.addToken(TokenType.RIGHT_PAREN, Literal{ .void = {} }),
            ',' => try self.addToken(TokenType.COMMA, Literal{ .void = {} }),
            '.' => try self.addToken(TokenType.DOT, Literal{ .void = {} }),
            ':' => try self.addToken(TokenType.COLON, Literal{ .void = {} }),
            '-' => try self.addToken(TokenType.MINUS, Literal{ .void = {} }),
            '+' => try self.addToken(TokenType.PLUS, Literal{ .void = {} }),
            '*' => try self.addToken(TokenType.MULTIPLICATION, Literal{ .void = {} }),
            '%' => try self.addToken(TokenType.REMINDER, Literal{ .void = {} }),
            '^' => try self.addToken(TokenType.POWER, Literal{ .void = {} }),

            '<' => try self.addToken(if (self.match('=')) TokenType.LESS_EQUAL else TokenType.LESS, Literal{ .void = {} }),
            '>' => try self.addToken(if (self.match('=')) TokenType.GREATER_EQUAL else TokenType.GREATER, Literal{ .void = {} }),
            '!' => try self.addToken(if (self.match('=')) TokenType.NOT_EQUAL else TokenType.NOT, Literal{ .void = {} }),
            '=' => try self.addToken(if (self.match('=')) TokenType.EQUAL_EQUAL else TokenType.EQUAL, Literal{ .void = {} }),
            '/' => try self.addToken(if (self.match('/')) TokenType.EUCLIDIAN else TokenType.DIVISION, Literal{ .void = {} }),

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
                try self.addToken(TokenType.STRING, Literal{ .str = value });
            },

            '0'...'9' => {
                while (std.ascii.isDigit(self.peek())) _ = self.advance();

                if (self.peek() == '.' and std.ascii.isDigit(self.peekNext())) {
                    _ = self.advance();

                    while (std.ascii.isDigit(self.peek())) _ = self.advance();

                    const float = try std.fmt.parseFloat(f64, self.source[self.start..self.current]);
                    try self.addToken(TokenType.FLOAT, .{ .float = float });
                } else {
                    const int = try std.fmt.parseInt(i64, self.source[self.start..self.current], 10);
                    try self.addToken(TokenType.INTEGER, .{ .int = int });
                }
            },

            'A'...'Z', 'a'...'z', '_' => {
                while (std.ascii.isAlphanumeric(self.peek()) or self.peek() == '_') _ = self.advance();

                var token_type = TokenType.IDENTIFIER;
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

    fn scanTokens(self: *Scanner) !void {
        while (!self.isAtEnd()) {
            self.start = self.current;
            try self.scanToken();
        }
    }
};

const Cli = struct {
    alloc: std.mem.Allocator,

    pub fn init(alloc: std.mem.Allocator) !Cli {
        return Cli{
            .alloc = alloc,
        };
    }

    pub fn scanFile(self: *Cli, path: []const u8) !void {
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

        var scanner = try Scanner.init(self.alloc);
        defer scanner.deinit();

        const tokens = try scanner.scan(source);
        _ = tokens;
    }

    pub fn scanPrompt(self: *Cli) !void {
        const in = std.io.getStdIn().reader();
        const out = std.io.getStdOut().writer();

        var scanner = try Scanner.init(self.alloc);
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
