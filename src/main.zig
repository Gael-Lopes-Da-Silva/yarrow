const std = @import("std");

const Tokenizer = struct {
    const TokenType = enum {
        L_paren,
        R_paren,
        Comma,

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

        Identifier,
        String,
        Integer,
        Float,

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

    const Literal = union(enum) {
        Void,
        Int: i64,
        Float: f64,
        String: []const u8,
    };

    const Token = struct {
        type: TokenType,
        lexeme: []const u8,
        literal: Literal,
        line: usize,
        start: usize,
        end: usize,
    };

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

    fn init(alloc: std.mem.Allocator) Tokenizer {
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

    fn tokenize(self: *Tokenizer, source: []const u8) !std.ArrayList(Token) {
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

    fn addToken(self: *Tokenizer, token_type: TokenType, literal: Literal) !void {
        try self.tokens.append(Token{
            .type = token_type,
            .lexeme = self.source[self.start..self.current],
            .literal = literal,
            .line = self.line,
            .start = self.column,
            .end = self.current,
        });
    }

    fn scanTokens(self: *Tokenizer) !void {
        while (!self.isAtEnd()) {
            self.start = self.current;

            switch (self.advance()) {
                '#' => {
                    while (self.peek() != '\n' and !self.isAtEnd()) _ = self.advance();
                },

                '(' => try self.addToken(.L_paren, .{ .Void = {} }),
                ')' => try self.addToken(.R_paren, .{ .Void = {} }),
                ',' => try self.addToken(.Comma, .{ .Void = {} }),
                '.' => try self.addToken(.Dot, .{ .Void = {} }),
                '-' => try self.addToken(.Minus, .{ .Void = {} }),
                '+' => try self.addToken(.Plus, .{ .Void = {} }),
                '%' => try self.addToken(.Reminder, .{ .Void = {} }),

                '&' => try self.addToken(.Bitwise_and, .{ .Void = {} }),
                '|' => try self.addToken(.Bitwise_or, .{ .Void = {} }),
                '^' => try self.addToken(.Bitwise_xor, .{ .Void = {} }),

                '*' => try self.addToken(
                    if (self.match('*')) .Power else .Multiplication,
                    .{ .Void = {} },
                ),
                '/' => try self.addToken(
                    if (self.match('/')) .Euclidian else .Division,
                    .{ .Void = {} },
                ),
                '<' => try self.addToken(
                    if (self.match('=')) .Less_equal else if (self.match('<')) .L_shift else .Less,
                    .{ .Void = {} },
                ),
                '>' => try self.addToken(
                    if (self.match('=')) .Greater_equal else if (self.match('>')) .R_shift else .Greater,
                    .{ .Void = {} },
                ),

                '=' => if (self.match('=')) try self.addToken(.Equal_equal, .{ .Void = {} }),
                '!' => if (self.match('=')) try self.addToken(.Not_equal, .{ .Void = {} }),

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
                    try self.addToken(.String, .{ .String = self.source[self.start + 1 .. self.current - 1] });
                },

                '0'...'9' => {
                    while (!self.isAtEnd() and std.ascii.isDigit(self.peek())) _ = self.advance();

                    if (!self.isAtEnd() and self.peek() == '.' and std.ascii.isDigit(self.peekNext())) {
                        _ = self.advance();

                        while (!self.isAtEnd() and std.ascii.isDigit(self.peek())) _ = self.advance();

                        try self.addToken(.Float, .{ .Float = try std.fmt.parseFloat(f64, self.source[self.start..self.current]) });
                    } else {
                        try self.addToken(.Integer, .{ .Int = try std.fmt.parseInt(i32, self.source[self.start..self.current], 10) });
                    }
                },

                'A'...'Z', 'a'...'z', '_' => {
                    while (!self.isAtEnd() and (std.ascii.isAlphanumeric(self.peek()) or self.peek() == '_')) _ = self.advance();

                    var token_type: TokenType = .Identifier;
                    const keyword = self.lookupKeyword(self.source[self.start..self.current]);

                    if (keyword) |value| {
                        token_type = value;
                    }

                    try self.addToken(token_type, .{ .Void = {} });
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
    }
};

const Parser = struct {
    const Instruction = union(enum) {
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

        And,
        Not,
        Or,

        Drop,
        Dup,
        Over,
        Rot,
        Set,
        Swap,

        PushInt: i64,
        PushFloat: f64,
        PushString: []const u8,

        Load: []const u8,
        Store: []const u8,
    };

    instructions: std.ArrayList(Instruction),

    fn init(alloc: std.mem.Allocator) Parser {
        return Parser{
            .instructions = std.ArrayList(Instruction).init(alloc),
        };
    }

    fn deinit(self: *Parser) void {
        self.instructions.deinit();
    }

    fn parse(self: *Parser, tokens: std.ArrayList(Tokenizer.Token)) !std.ArrayList(Instruction) {
        for (tokens.items) |token| {
            switch (token.type) {
                .Integer => try self.instructions.append(.{ .PushInt = token.literal.Int }),
                .Float => try self.instructions.append(.{ .PushFloat = token.literal.Float }),
                .String => try self.instructions.append(.{ .PushString = token.literal.String }),
                .Identifier => try self.instructions.append(.{ .Load = token.lexeme }),

                .And => try self.instructions.append(.And),
                .Or => try self.instructions.append(.Or),
                .Not => try self.instructions.append(.Not),

                .Plus => try self.instructions.append(.Plus),
                .Minus => try self.instructions.append(.Minus),
                .Multiplication => try self.instructions.append(.Multiplication),
                .Division => try self.instructions.append(.Division),
                .Euclidian => try self.instructions.append(.Euclidian),
                .Reminder => try self.instructions.append(.Reminder),
                .Power => try self.instructions.append(.Power),

                .Equal_equal => try self.instructions.append(.Equal_equal),
                .Not_equal => try self.instructions.append(.Not_equal),
                .Greater => try self.instructions.append(.Greater),
                .Greater_equal => try self.instructions.append(.Greater_equal),
                .Less => try self.instructions.append(.Less),
                .Less_equal => try self.instructions.append(.Less_equal),

                .Bitwise_and => try self.instructions.append(.Bitwise_and),
                .Bitwise_or => try self.instructions.append(.Bitwise_or),
                .Bitwise_xor => try self.instructions.append(.Bitwise_xor),
                .L_shift => try self.instructions.append(.L_shift),
                .R_shift => try self.instructions.append(.R_shift),

                .Drop => try self.instructions.append(.Drop),
                .Dup => try self.instructions.append(.Dup),
                .Over => try self.instructions.append(.Over),
                .Rot => try self.instructions.append(.Rot),
                .Set => try self.instructions.append(.Set),
                .Swap => try self.instructions.append(.Swap),

                else => {},
            }
        }

        return self.instructions;
    }
};

const Interpreter = struct {
    const Value = union(enum) {
        Int: i64,
        Float: f64,
        Bool: bool,
        String: []const u8,
    };

    stack: std.ArrayList(Value),

    fn init(alloc: std.mem.Allocator) Interpreter {
        return Interpreter{
            .stack = std.ArrayList(Value).init(alloc),
        };
    }

    fn deinit(self: *Interpreter) void {
        self.stack.deinit();
    }

    fn push(self: *Interpreter, value: Value) !void {
        try self.stack.append(value);
    }

    fn drop(self: *Interpreter) !Value {
        if (self.stack.items.len <= 0) return error.StackUnderflow;
        return self.stack.pop();
    }

    fn interpret(self: *Interpreter, instructions: std.ArrayList(Parser.Instruction)) !void {
        for (instructions.items) |instruction| {
            switch (instruction) {
                .PushInt => |value| {
                    try self.push(.{ .Int = value });
                },
                .PushFloat => |value| {
                    try self.push(.{ .Float = value });
                },
                .PushString => |value| {
                    try self.push(.{ .String = value });
                },

                .Plus => {
                    const b = try self.drop();
                    const a = try self.drop();

                    const result = switch (a) {
                        .Int => switch (b) {
                            .Int => Value{ .Int = a.Int + b.Int },

                            else => return error.TypeMismatch,
                        },
                        .Float => switch (b) {
                            .Float => Value{ .Float = a.Float + b.Float },

                            else => return error.TypeMismatch,
                        },

                        else => return error.TypeMismatch,
                    };

                    try self.push(result);
                },

                else => {},
            }
        }
    }
};

const Compiler = struct {
    fn compile(instructions: std.ArrayList(Parser.Instruction)) !void {
        _ = instructions;
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

        var scanner = Tokenizer.init(self.alloc);
        scanner.path = path;
        defer scanner.deinit();

        if (source.len > 0 and !std.ascii.eqlIgnoreCase(source, "")) {
            const tokens = try scanner.tokenize(source);
            _ = tokens;
        }
    }

    fn scanPrompt(self: *Cli) !void {
        const in = std.io.getStdIn().reader();
        const out = std.io.getStdOut().writer();

        // var parser = Parser.init(self.alloc);
        // defer parser.deinit();
        //
        // var interpreter = Interpreter.init(self.alloc);
        // defer interpreter.deinit();

        while (true) {
            var tokenizer = Tokenizer.init(self.alloc);
            defer tokenizer.deinit();

            try out.print("> ", .{});
            const source = try in.readUntilDelimiterAlloc(self.alloc, '\n', std.math.maxInt(usize));
            defer self.alloc.free(source);

            if (std.ascii.eqlIgnoreCase(source, "quit")) break;
            if (std.ascii.eqlIgnoreCase(source, "exit")) break;
            // if (std.ascii.eqlIgnoreCase(source, "print")) std.debug.print("{?}\n", .{interpreter.stack});

            if (source.len > 0 and !std.ascii.eqlIgnoreCase(source, "")) {
                const tokens = try tokenizer.tokenize(source);

                std.debug.print("{d}\n", .{tokens.items.len});
                // const instructions = try parser.parse(tokens);
                // try interpreter.interpret(instructions);
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
