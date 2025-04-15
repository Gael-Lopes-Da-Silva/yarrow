const std = @import("std");

const Literal = union(enum) {
    void,
    int: i64,
    float: f64,
    str: []const u8,
};

const TokenType = enum {
    LEFT_PAREN,
    RIGHT_PAREN,
    LEFT_BRACE,
    RIGHT_BRACE,

    COMMA,
    DOT,
    COLON,
    SEMICOLON,
    HASH,
    DOUBLE_HASH,
    LEFT_MULTHASH,
    RIGHT_MULTHASH,

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
    BOOLEAN,

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
    NAMESPACE,

    NEWLINE,
    EOF,
};

const Token = struct {
    type: TokenType,
    lexeme: []const u8,
    literal: Literal,
    line: usize,

    pub fn print(self: Token) void {
        switch (self.literal) {
            .void => std.debug.print("{s} ", .{@tagName(self.type)}),
            .int => |value| std.debug.print("({s}: {d}) ", .{ @tagName(self.type), value }),
            .float => |value| std.debug.print("({s}: {d:.2}) ", .{ @tagName(self.type), value }),
            .str => |value| std.debug.print("({s}: {s}) ", .{ @tagName(self.type), value }),
        }
    }
};

const Scanner = struct {
    start: usize,
    current: usize,
    line: usize,

    tokens: std.ArrayList(Token),
    alloc: std.mem.Allocator,

    source: []const u8,

    keyword_map: std.StringHashMap(TokenType),

    pub fn init(alloc: std.mem.Allocator) !Scanner {
        var keywords = std.StringHashMap(TokenType).init(alloc);

        try keywords.put("and", TokenType.AND);
        try keywords.put("or", TokenType.OR);
        try keywords.put("not", TokenType.NOT);
        try keywords.put("function", TokenType.FUNCTION);
        try keywords.put("return", TokenType.RETURN);
        try keywords.put("if", TokenType.IF);
        try keywords.put("else", TokenType.ELSE);
        try keywords.put("match", TokenType.MATCH);
        try keywords.put("case", TokenType.CASE);
        try keywords.put("default", TokenType.DEFAULT);
        try keywords.put("end", TokenType.END);
        try keywords.put("do", TokenType.DO);
        try keywords.put("with", TokenType.WITH);
        try keywords.put("while", TokenType.WHILE);
        try keywords.put("const", TokenType.CONST);
        try keywords.put("mutable", TokenType.MUTABLE);
        try keywords.put("struct", TokenType.STRUCT);
        try keywords.put("union", TokenType.UNION);
        try keywords.put("enum", TokenType.ENUM);
        try keywords.put("public", TokenType.PUBLIC);
        try keywords.put("private", TokenType.PRIVATE);
        try keywords.put("protected", TokenType.PROTECTED);
        try keywords.put("try", TokenType.TRY);
        try keywords.put("discard", TokenType.DISCARD);
        try keywords.put("require", TokenType.REQUIRE);
        try keywords.put("namespace", TokenType.NAMESPACE);
        try keywords.put("true", TokenType.BOOLEAN);
        try keywords.put("false", TokenType.BOOLEAN);

        return Scanner{
            .start = 0,
            .current = 0,
            .line = 1,
            .tokens = std.ArrayList(Token).init(alloc),
            .alloc = alloc,
            .source = "",
            .keyword_map = keywords,
        };
    }

    pub fn deinit(self: *Scanner) void {
        self.tokens.deinit();
        self.keyword_map.deinit();
    }

    pub fn addToken(self: *Scanner, token_type: TokenType, literal: Literal) !void {
        try self.tokens.append(Token{
            .type = token_type,
            .literal = literal,
            .line = self.line,
            .lexeme = self.source[self.start..self.current],
        });
    }

    pub fn scanToken(self: *Scanner) !void {
        const char = self.advance();

        switch (char) {
            '#' => {
                while (self.source[self.current] != '\n' and !self.isAtEnd()) _ = self.advance();
            },

            '(' => try self.addToken(TokenType.LEFT_PAREN, Literal{ .void = {} }),
            ')' => try self.addToken(TokenType.RIGHT_PAREN, Literal{ .void = {} }),
            '{' => try self.addToken(TokenType.LEFT_BRACE, Literal{ .void = {} }),
            '}' => try self.addToken(TokenType.RIGHT_BRACE, Literal{ .void = {} }),
            ',' => try self.addToken(TokenType.COMMA, Literal{ .void = {} }),
            '.' => try self.addToken(TokenType.DOT, Literal{ .void = {} }),
            ':' => try self.addToken(TokenType.COLON, Literal{ .void = {} }),
            ';' => try self.addToken(TokenType.SEMICOLON, Literal{ .void = {} }),
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

            '"' => try self.string(),
            '0'...'9' => try self.number(),
            'A'...'Z', 'a'...'z', '_' => try self.identifier(),

            else => {
                std.log.err("Unexpected character !\n{d}", .{self.line});
            },
        }
    }

    pub fn scanTokens(self: *Scanner) !std.ArrayList(Token) {
        while (!self.isAtEnd()) {
            self.start = self.current;
            try self.scanToken();
        }

        return self.tokens;
    }

    pub fn number(self: *Scanner) !void {
        var is_int = true;
        while (std.ascii.isDigit(self.source[self.current])) _ = self.advance();

        if (self.source[self.current] == '.' and std.ascii.isDigit(self.source[self.current + 1])) {
            is_int = false;
            _ = self.advance();
            while (std.ascii.isDigit(self.source[self.current])) _ = self.advance();
        }

        if (is_int) {
            const int = try std.fmt.parseInt(i64, self.source[self.start..self.current], 10);
            try self.addToken(TokenType.INTEGER, .{ .int = int });
        } else {
            const float = try std.fmt.parseFloat(f64, self.source[self.start..self.current]);
            try self.addToken(TokenType.FLOAT, .{ .float = float });
        }
    }

    pub fn identifier(self: *Scanner) !void {
        while (std.ascii.isAlphanumeric(self.source[self.current])) _ = self.advance();

        var token_type = TokenType.IDENTIFIER;
        const keyword = self.keyword_map.get(self.source[self.start..self.current]);

        if (keyword) |value| {
            token_type = value;
        }

        try self.addToken(token_type, Literal{ .void = {} });
    }

    pub fn string(self: *Scanner) !void {
        while (self.source[self.current] != '"' and !self.isAtEnd()) {
            if (self.source[self.current] == '\n') {
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
    }

    pub fn advance(self: *Scanner) u8 {
        const char = self.source[self.current];
        self.current += 1;
        return char;
    }

    pub fn isAtEnd(self: Scanner) bool {
        return self.current >= self.source.len;
    }

    pub fn match(self: *Scanner, expected: u8) bool {
        if (self.isAtEnd()) return false;
        if (self.source[self.current] != expected) return false;

        self.current += 1;
        return true;
    }

    pub fn scan(self: *Scanner, source: []const u8) !void {
        self.source = source;
        const tokens = try self.scanTokens();
        var line: usize = 1;
        for (tokens.items) |token| {
            while (line < token.line) {
                line += 1;
                std.debug.print("\n", .{});
            }

            token.print();
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

        try scanner.scan(source);
    }

    pub fn scanPrompt(self: *Cli) !void {
        const in = std.io.getStdIn().reader();
        const out = std.io.getStdOut().writer();

        var scanner = try Scanner.init(self.alloc);
        defer scanner.deinit();

        while (true) {
            try out.print("> ", .{});
            const source = try in.readUntilDelimiterAlloc(self.alloc, '\n', std.math.maxInt(usize));
            defer self.alloc.free(source);

            if (source.len > 0 and !std.ascii.eqlIgnoreCase(source, "")) {
                try scanner.scan(source);
                try out.print("\n", .{});
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
