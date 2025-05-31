local Tokens = require("utils.enums.tokens")
local Types = require("utils.enums.types")
local Token = require("utils.token")


local Tokenizer = {}
Tokenizer.__index = Tokenizer

function Tokenizer.new(args)
    local log     = args[1] or args.log

    local self    = setmetatable({}, Tokenizer)
    self.log      = log
    self.source   = ""
    self.start    = { 0, 0 }
    self.current  = { 0, 0 }
    self.line     = 1
    self.tokens   = {}
    self.keywords = {
        ["and"] = Tokens.AND,
        ["or"] = Tokens.OR,
        ["xor"] = Tokens.XOR,
        ["not"] = Tokens.NOT,
        ["lshift"] = Tokens.LEFT_SHIFT,
        ["rshift"] = Tokens.RIGHT_SHIFT,
        ["if"] = Tokens.IF,
        ["else"] = Tokens.ELSE,
        ["while"] = Tokens.WHILE,
        ["for"] = Tokens.FOR,
        ["break"] = Tokens.BREAK,
        ["continue"] = Tokens.CONTINUE,
        ["match"] = Tokens.MATCH,
        ["case"] = Tokens.CASE,
        ["unwrap"] = Tokens.UNWRAP,
        ["handle"] = Tokens.HANDLE,
        ["function"] = Tokens.FUNCTION,
        ["return"] = Tokens.RETURN,
        ["call"] = Tokens.CALL,
        ["do"] = Tokens.DO,
        ["with"] = Tokens.WITH,
        ["const"] = Tokens.CONST,
        ["static"] = Tokens.STATIC,
        ["mutable"] = Tokens.MUTABLE,
        ["set"] = Tokens.SET,
        ["struct"] = Tokens.STRUCT,
        ["implement"] = Tokens.IMPLEMENT,
        ["enum"] = Tokens.ENUM,
        ["union"] = Tokens.UNION,
        ["pop"] = Tokens.POP,
        ["drop"] = Tokens.DROP,
        ["dup"] = Tokens.DUP,
        ["over"] = Tokens.OVER,
        ["rot"] = Tokens.ROT,
        ["swap"] = Tokens.SWAP,
        ["require"] = Tokens.REQUIRE,
        ["defer"] = Tokens.DEFER,
        ["end"] = Tokens.END,
        ["true"] = Tokens.BOOLEAN,
        ["false"] = Tokens.BOOLEAN,
    }

    for name, _ in pairs(Types) do
        self.keywords[name:lower()] = Tokens.TYPE
    end

    return self
end

function Tokenizer:tokenize(args)
    local source = args[1] or args.source

    self.source = source

    while not self:_eof() do
        self.start[1] = self.current[1]
        self.start[2] = self.current[2]

        local lexeme = self:_advance()

        if lexeme == " " or lexeme == "\t" then
        elseif lexeme == "\n" then
            self.line = self.line + 1
            self.current[2] = 0
        elseif lexeme == "#" then
            while not self:_eof() and self:_peek() ~= "\n" do
                self:_advance()
            end
        elseif lexeme == "(" then
            self:_add_token(Tokens.LEFT_PAREN)
        elseif lexeme == ")" then
            self:_add_token(Tokens.RIGHT_PAREN)
        elseif lexeme == "{" then
            self:_add_token(Tokens.LEFT_CURLY)
        elseif lexeme == "}" then
            self:_add_token(Tokens.RIGHT_CURLY)
        elseif lexeme == "[" then
            self:_add_token(Tokens.LEFT_SQUARE)
        elseif lexeme == "]" then
            self:_add_token(Tokens.RIGHT_SQUARE)
        elseif lexeme == ":" then
            self:_add_token(Tokens.COLON)
        elseif lexeme == ";" then
            self:_add_token(Tokens.SEMI_COLON)
        elseif lexeme == "," then
            self:_add_token(Tokens.COMMA)
        elseif lexeme == "." then
            self:_add_token(Tokens.DOT)
        elseif lexeme == "?" then
            self:_add_token(Tokens.QUESTION)
        elseif lexeme == "%" then
            self:_add_token(Tokens.PERCENT)
        elseif lexeme == "&" then
            self:_add_token(Tokens.AMPERSAND)
        elseif lexeme == "|" then
            self:_add_token(Tokens.BAR)
        elseif lexeme == "*" then
            self:_add_token(Tokens.ASTERISK)
        elseif lexeme == "^" then
            self:_add_token(Tokens.CARET)
        elseif lexeme == "/" then
            if self:_match("/") then
                self:_add_token(Tokens.SLASH_SLASH)
            else
                self:_add_token(Tokens.SLASH)
            end
        elseif lexeme == "=" then
            if self:_match("=") then
                self:_add_token(Tokens.EQUAL_EQUAL)
            else
                self:_add_token(Tokens.EQUAL)
            end
        elseif lexeme == "<" then
            if self:_match("=") then
                self:_add_token(Tokens.LESS_EQUAL)
            else
                self:_add_token(Tokens.LESS)
            end
        elseif lexeme == ">" then
            if self:_match("=") then
                self:_add_token(Tokens.GREATER_EQUAL)
            else
                self:_add_token(Tokens.GREATER)
            end
        elseif lexeme == "!" then
            if self:_match("=") then
                self:_add_token(Tokens.NOT_EQUAL)
            else
                self:_add_token(Tokens.EXCLAMATION)
            end
        elseif lexeme == '"' then
            self:_handle_strings()
        elseif lexeme == "'" then
            self:_handle_runes()
        elseif lexeme == "-" or lexeme == "+" then
            if not self:_eof() and self:_peek():match("%d") then
                self:_handle_numbers()
            else
                self:_add_token(lexeme == "-" and Tokens.MINUS or Tokens.PLUS)
            end
        elseif lexeme:match("%d") then
            self:_handle_numbers()
        elseif lexeme:match("[a-zA-Z]") or lexeme == "_" or lexeme == "@" then
            self:_handle_identifiers()
        else
            self.log:print({
                "warning",
                "Unsupported symbol",
                location = self:_get_location(),
                code = "W001"
            })
        end
    end

    return self.tokens
end

function Tokenizer:_handle_numbers()
    while not self:_eof() and (self:_peek():match("[0-9_,]")) do
        self:_advance()
    end

    if not self:_eof() and self:_peek() == "." and self:_peek_next():match("[0-9_,]") then
        self:_advance()
        while not self:_eof() and (self:_peek():match("[0-9_,]")) do
            self:_advance()
        end
        self:_add_token(Tokens.FLOAT)
    else
        self:_add_token(Tokens.INTEGER)
    end
end

function Tokenizer:_handle_strings()
    while not self:_eof() and self:_peek() ~= '"' do
        if self:_peek() == "\n" then
            self.log:print({
                "error",
                "Invalid string syntax",
                location = self:_get_location(),
                information = "new lines are not supported inside strings",
                code = "E120"
            })
            os.exit(120)
        end

        if self:_match("\\") then
            if self:_eof() then
                self.log:print({
                    "error",
                    "Invalid string syntax",
                    location = { self.line, self.current[2] - 1, self.current[2] },
                    information = "escape symbols should be followed by a valid letter",
                    code = "E121"
                })
                os.exit(121)
            end

            local escape_rune = self:_peek()
            if escape_rune:match("[\\\"'nrtvabf]") then
                self:_advance()
            else
                self.log:print({
                    "error",
                    "Invalid string syntax",
                    location = { self.line, self.current[2] - 1, self.current[2] + 1 },
                    information = "escape symbols should be followed by a valid letter",
                    code = "E121"
                })
                os.exit(121)
            end
        else
            self:_advance()
        end
    end

    if self:_eof() or not self:_match('"') then
        self.log:print({
            "error",
            "Invalid string syntax",
            location = self:_get_location(),
            information = "string literals need to be closed with a corresponding quote",
            code = "E122"
        })
        os.exit(122)
    end

    self:_add_token(Tokens.STRING)
end

function Tokenizer:_handle_runes()
    while not self:_eof() and self:_peek() ~= "'" do
        if self:_peek() == "\n" then
            self.log:print({
                "error",
                "Invalid rune syntax",
                location = self:_get_location(),
                information = "new lines are not supported inside runes",
                code = "E130"
            })
            os.exit(130)
        end

        if self:_peek() == "\\" then
            self:_advance()
            if self:_eof() then
                self.log:print({
                    "error",
                    "Invalid rune syntax",
                    location = { self.line, self.current[2] - 1, self.current[2] },
                    information = "escape symbols should be followed by a valid letter",
                    code = "E131"
                })
                os.exit(131)
            end

            local escape_rune = self:_peek()
            if escape_rune:match("[\\\'\"nrtvabf]") then
                self:_advance()
            else
                self.log:print({
                    "error",
                    "Invalid rune syntax",
                    location = { self.line, self.current[2] - 1, self.current[2] + 1 },
                    information = "escape symbols should be followed by a valid letter",
                    code = "E131"
                })
                os.exit(131)
            end
        else
            self:_advance()
        end
    end

    if self:_eof() or not self:_match("'") then
        self.log:print({
            "error",
            "Invalid rune syntax",
            location = self:_get_location(),
            information = "rune literals need to be closed with a corresponding quote",
            code = "E132"
        })
        os.exit(132)
    end

    local content = self.source:sub(self.start[1] + 2, self.current[1] - 1)
    if #content:gsub("\\.", "") > 1 then
        self.log:print({
            "error",
            "Invalid rune syntax",
            location = self:_get_location(),
            information = "runes should only contain a letter or an escape sequence",
            code = "E133"
        })
        os.exit(133)
    end

    self:_add_token(Tokens.RUNE)
end

function Tokenizer:_handle_identifiers()
    while not self:_eof() and (self:_peek():match("[a-zA-Z0-9_@]")) do
        self:_advance()
    end

    local text = self.source:sub(self.start[1] + 1, self.current[1])
    local token_type = self.keywords[text:lower()] or Tokens.IDENTIFIER
    self:_add_token(token_type)
end

function Tokenizer:_add_token(token_kind)
    table.insert(self.tokens, Token.new(
        token_kind,
        self.source:sub(self.start[1] + 1, self.current[1]),
        self:_get_location()
    ))
end

function Tokenizer:_advance()
    local lexeme = self:_peek()
    self.current[1] = self.current[1] + 1
    self.current[2] = self.current[2] + 1
    return lexeme
end

function Tokenizer:_match(expected)
    if self:_eof() or self:_peek() ~= expected then
        return false
    end
    self:_advance()
    return true
end

function Tokenizer:_get_location()
    return { self.line, self.start[2], self.current[2] }
end

function Tokenizer:_eof()
    return self.current[1] >= #self.source
end

function Tokenizer:_peek()
    return self.source:sub(self.current[1] + 1, self.current[1] + 1)
end

function Tokenizer:_peek_next()
    return self.source:sub(self.current[1] + 2, self.current[1] + 2)
end

return Tokenizer
