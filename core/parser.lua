local Tokens = require("utils.enums.tokens")
local Types = require("utils.enums.types")
local Instruction = require("utils.instruction")


local Parser = {}
Parser.__index = Parser

function Parser.new(args)
    local log = args[1] or args.log

    local self = setmetatable({}, Parser)
    self.log = log
    self.tokens = {}
    self.instructions = {}
    self.current = 0
    return self
end

function Parser:parse(args)
    local tokens = args[1] or args.tokens

    self.tokens = tokens

    while not self:_eof() do
        local instruction = self:_parse_instruction()
        if instruction ~= nil then
            table.insert(self.instructions, instruction)
        end
    end

    return self.instructions
end

function Parser:_parse_instruction()
    local token = self:_advance()

    local kind = token.kind
    local lexeme = token.lexeme

    if kind == Tokens.IDENTIFIER then
        return Instruction.new("push", { type = Types.VOID, value = lexeme }, token)
    elseif kind == Tokens.STRING then
        return Instruction.new("push", { type = Types.STRING, value = lexeme:sub(2, -2) }, token)
    elseif kind == Tokens.RUNE then
        return Instruction.new("push", { type = Types.RUNE, value = lexeme:sub(2, -2) }, token)
    elseif kind == Tokens.INTEGER then
        local int_value = tonumber(lexeme:gsub("[_,]", ""))
        local int_type = self:_get_smallest_integer_type(int_value)
        return Instruction.new("push", { type = int_type, value = int_value }, token)
    elseif kind == Tokens.FLOAT then
        local float_value = tonumber(lexeme:gsub("[_,]", ""))
        local float_type = self:_get_smallest_float_type(float_value)
        return Instruction.new("push", { type = float_type, value = float_value }, token)
    elseif kind == Tokens.BOOLEAN then
        return Instruction.new("push", { type = Types.BOOL, value = lexeme:lower() == "true" }, token)
    elseif kind == Tokens.TYPE then
        return Instruction.new("push", { type = Types.TYPE, value = self:_handle_types(token) }, token)
    elseif kind == Tokens.PLUS then
        return Instruction.new("addition", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.MINUS then
        return Instruction.new("subtraction", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.ASTERISK then
        return Instruction.new("multiplication", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.SLASH then
        return Instruction.new("division", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.SLASH_SLASH then
        return Instruction.new("euclidian_division", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.PERCENT then
        return Instruction.new("remainder", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.CARET then
        return Instruction.new("power", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.QUESTION then
        return Instruction.new("default", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.AND then
        return Instruction.new("and", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.OR then
        return Instruction.new("or", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.XOR then
        return Instruction.new("xor", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.NOT then
        return Instruction.new("not", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.LEFT_SHIFT then
        return Instruction.new("left_shift", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.RIGHT_SHIFT then
        return Instruction.new("right_shift", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.EQUAL_EQUAL then
        return Instruction.new("equal", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.NOT_EQUAL then
        return Instruction.new("not_equal", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.GREATER then
        return Instruction.new("greater", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.GREATER_EQUAL then
        return Instruction.new("greater_equal", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.LESS then
        return Instruction.new("less", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.LESS_EQUAL then
        return Instruction.new("less_equal", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.POP then
        return Instruction.new("pop", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.DROP then
        return Instruction.new("drop", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.DUP then
        return Instruction.new("dup", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.OVER then
        return Instruction.new("over", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.ROT then
        return Instruction.new("rot", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.SWAP then
        return Instruction.new("swap", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.RETURN then
        return Instruction.new("return", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.CALL then
        return Instruction.new("call", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.BREAK then
        return Instruction.new("break", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.CONTINUE then
        return Instruction.new("continue", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.UNWRAP then
        return Instruction.new("unwrap", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.SET then
        return Instruction.new("set", { type = nil, value = lexeme }, token)
    elseif kind == Tokens.MUTABLE then
        return self:_handle_mutables(token)
    elseif kind == Tokens.CONST then
        return self:_handle_consts(token)
    elseif kind == Tokens.STATIC then
        return self:_handle_statics(token)
    elseif kind == Tokens.FUNCTION then
        return self:_handle_functions(token)
    elseif kind == Tokens.IF then
        return self:_handle_if_elses(token)
    elseif kind == Tokens.MATCH then
        return self:_handle_matchs(token)
    elseif kind == Tokens.WHILE then
        return self:_handle_whiles(token)
    elseif kind == Tokens.FOR then
        return self:_handle_fors(token)
    elseif kind == Tokens.STRUCT then
        return self:_handle_structs(token)
    elseif kind == Tokens.IMPLEMENT then
        return self:_handle_implements(token)
    elseif kind == Tokens.ENUM then
        return self:_handle_enums(token)
    elseif kind == Tokens.UNION then
        return self:_handle_unions(token)
    elseif kind == Tokens.REQUIRE then
        return self:_handle_requires(token)
    elseif kind == Tokens.DOT then
        return self:_handle_dots(token)
    elseif kind == Tokens.DEFER then
        return self:_handle_defers(token)
    elseif kind == Tokens.HANDLE then
        return self:_handle_handles(token)
    elseif kind == Tokens.LEFT_SQUARE then
        return self:_handle_arrays(token)
    elseif kind == Tokens.LEFT_CURLY then
        return self:_handle_hashmaps(token)
    elseif kind == Tokens.LEFT_PAREN then
        return self:_handle_lists(token)
    end

    return nil
end

function Parser:_handle_mutables(token)
    local variable_type = self:_handle_types()
    return Instruction.new("mutable", { type = nil, value = variable_type }, token)
end

function Parser:_handle_consts(token)
    local variable_type = self:_handle_types()
    return Instruction.new("const", { type = nil, value = variable_type }, token)
end

function Parser:_handle_statics(token)
    local variable_type = self:_handle_types()
    return Instruction.new("static", { type = nil, value = variable_type }, token)
end

function Parser:_handle_functions(token)
    local parameters = {}
    while not self:_eof() and self:_peek().kind ~= Tokens.DO do
        if self:_peek().kind ~= Tokens.TYPE then
            self.log:print({
                "error",
                "Invalid function syntax",
                location = self:__peek().location,
                information = "function parameters are only composed of types",
                code = "E140"
            })
            os.exit(140)
        end

        local parameter_type = self:_handle_types({ no_error = true })
        table.insert(parameters, parameter_type)
    end

    local do_token = self:_expect(Tokens.DO)
    if do_token == nil then
        self.log:print({
            "error",
            "Invalid function syntax",
            location = token.location,
            information = "function need a body openned with `do` and closed with `end`",
            code = "E141"
        })
        os.exit(141)
    end

    local body = {}
    while not self:_eof() and self:_peek().kind ~= Tokens.END do
        local instruction = self:_parse_instruction()
        if instruction ~= nil then
            table.insert(body, instruction)
        end
    end

    if self:_expect(Tokens.END) == nil then
        self.log:print({
            "error",
            "Invalid function syntax",
            location = do_token.location,
            information = "function body need to be closed with `end`",
            code = "E142"
        })
        os.exit(142)
    end

    local return_type = nil
    local return_error = nil
    local with_token = self:_expect(Tokens.WITH)
    if with_token ~= nil then
        return_type = self:_handle_types({ no_error = true })
        if return_type == nil then
            self.log:print({
                "error",
                "Invalid function syntax",
                location = with_token.location,
                information = "there should be a type after a `with` statement in a function definition",
                code = "E143"
            })
            os.exit(143)
        end

        local or_token = self:_expect(Tokens.OR)
        if or_token ~= nil then
            return_error = self:_handle_types({ no_error = true })
            if return_error == nil then
                self.log:print({
                    "error",
                    "Invalid function syntax",
                    location = or_token.location,
                    information = "there should be an error type after an `or` statement in a function definition",
                    code = "E144"
                })
                os.exit(144)
            end
        end
    end

    return Instruction.new(
        "function",
        {
            type = nil,
            value = {
                parameters = parameters,
                body = body,
                return_type = return_type,
                return_error = return_error
            }
        },
        token
    )
end

function Parser:_handle_if_elses(token)
    local if_body = {}
    while not self:_eof() and self:_peek().kind ~= Tokens.ELSE and self:_peek().kind ~= Tokens.END do
        local instruction = self:_parse_instruction()
        if instruction ~= nil then
            table.insert(if_body, instruction)
        end
    end

    local else_body = {}
    local else_token = self:_expect(Tokens.ELSE)
    if else_token ~= nil then
        while not self:_eof() and self:_peek().kind ~= Tokens.END do
            local instruction = self:_parse_instruction()
            if instruction ~= nil then
                table.insert(else_body, instruction)
            end
        end
    end

    if self:_expect(Tokens.END) == nil then
        self.log({
            "error",
            "Invalid if/else syntax",
            location = else_token ~= nil and else_token.location or token.location,
            information = "if/else body need to be closed with `end`",
            code = "E145"
        })
        os.exit(145)
    end

    return Instruction.new(
        "if",
        {
            type = nil,
            value = { if_body = if_body, else_body = else_body }
        },
        token
    )
end

function Parser:_handle_matchs(token)
    local cases = {}
    local else_body = {}
    while not self:_eof() and self:_peek().kind ~= Tokens.END do
        local case_token = self:_expect(Tokens.CASE)
        if case_token ~= nil then
            local case_body = {}
            while not self:_eof() and self:_peek().kind ~= Tokens.END do
                local instruction = self:_parse_instruction()
                if instruction ~= nil then
                    table.insert(case_body, instruction)
                end
            end

            if self:_expect(Tokens.END) == nil then
                self.log:print({
                    "error",
                    "Invalid match syntax",
                    location = case_token.location,
                    information = "case body need to be closed with `end`",
                    code = "E146"
                })
                os.exit(146)
            end

            table.insert(cases, case_body)
        end

        local else_token = self:_expect(Tokens.ELSE)
        if else_token ~= nil then
            while not self:_eof() and self:_peek().kind ~= Tokens.END do
                local instruction = self:_parse_instruction()
                if instruction ~= nil then
                    table.insert(else_body, instruction)
                end
            end

            if self:_expect(Tokens.END) == nil then
                self.log:print({
                    "error",
                    "Invalid match syntax",
                    location = else_token.location,
                    information = "case body need to be closed with `end`",
                    code = "E146"
                })
                os.exit(146)
            end

            break
        end
    end

    if self:_expect(Tokens.END) == nil then
        self.log({
            "error",
            "Invalid match syntax",
            location = token.location,
            information = "match body need to be closed with `end`",
            code = "E147"
        })
        os.exit(147)
    end

    return Instruction.new(
        "match",
        {
            type = nil,
            value = { cases = cases, else_body = else_body }
        },
        token
    )
end

function Parser:_handle_whiles(token)
    local body = {}
    while not self:_eof() and self:_peek().kind ~= Tokens.END do
        local instruction = self:_parse_instruction()
        if instruction ~= nil then
            table.insert(body, instruction)
        end
    end

    if self:_expect(Tokens.END) == nil then
        self.log:print({
            "error",
            "Invalid while syntax",
            location = token.location,
            information = "while body need to be closed with `end`",
            code = "E148"
        })
        os.exit(148)
    end

    return Instruction.new(
        "while",
        {
            type = nil,
            value = { body = body }
        },
        token
    )
end

function Parser:_handle_fors(token)
    local body = {}
    while not self:_eof() and self:_peek().kind ~= Tokens.END do
        local instruction = self:_parse_instruction()
        if instruction ~= nil then
            table.insert(body, instruction)
        end
    end

    if self:_expect(Tokens.END) == nil then
        self.log:print({
            "error",
            "Invalid for syntax",
            location = token.location,
            information = "for body need to be closed with `end`",
            code = "E149"
        })
        os.exit(149)
    end

    return Instruction.new(
        "for",
        {
            type = nil,
            value = { body = body }
        },
        token
    )
end

function Parser:_handle_structs(token)
    local body = {}
    while not self:_eof() and self:_peek().kind ~= Tokens.END do
        if self:_peek().kind ~= Tokens.TYPE and self:_peek().kind ~= Tokens.IDENTIFIER then
            self.log:print({
                "error",
                "Invalid struct syntax",
                location = self:_peek().location,
                information = "struct parameters are only composed of types and identifiers",
                code = "E150"
            })
            os.exit(150)
        end

        local variable_type = self:_handle_types({ no_error = true })
        local variable_name = self:_expect(Tokens.IDENTIFIER)

        if variable_type == nil then
            self.log:print({
                "error",
                "Invalid struct syntax",
                location = variable_name.location,
                information = "there should be a type before this",
                code = "E151"
            })
            os.exit(151)
        elseif variable_name == nil then
            self.log:print({
                "error",
                "Invalid struct syntax",
                location = variable_type.location,
                information = "there should be a name after this",
                code = "E152"
            })
            os.exit(152)
        end

        table.insert(body, { variable_name = variable_name, variable_type = variable_type })
    end

    if self:_expect(Tokens.END) == nil then
        self.log:print({
            "error",
            "Invalid struct syntax",
            location = token.location,
            information = "struct body need to be closed with `end`",
            code = "E153"
        })
        os.exit(153)
    end

    return Instruction.new(
        "struct",
        {
            type = nil,
            value = { body = body }
        },
        token
    )
end

function Parser:_handle_implements(token)
    local body = {}
    while not self:_eof() and self:_peek().kind ~= Tokens.END do
        if self:_peek().kind ~= Tokens.FUNCTION and self:_peek().kind ~= Tokens.IDENTIFIER then
            self.log:print({
                "error",
                "Invalid implement syntax",
                location = self:_peek().location,
                information = "implement are only composed of functions",
                code = "E154"
            })
            os.exit(154)
        end

        local instruction = self:_parse_instruction()
        if instruction ~= nil then
            table.insert(body, instruction)
        end
    end

    if self:_expect(Tokens.END) == nil then
        self.log:print({
            "error",
            "Invalid implement syntax",
            location = token.location,
            information = "implement body need to be closed with `end`",
            code = "E155"
        })
        os.exit(155)
    end

    return Instruction.new(
        "implement",
        {
            type = nil,
            value = { body = body }
        },
        token
    )
end

function Parser:_handle_enums(token)
    local body = {}
    while not self:_eof() and self:_peek().kind ~= Tokens.END do
        local identifier = self:_expect(Tokens.IDENTIFIER)
        if identifier == nil then
            self.log:print({
                "error",
                "Invalid enum syntax",
                location = token.location,
                information = "enum parameters are only composed of identifiers",
                code = "E156"
            })
            os.exit(156)
        end

        local value = self:_expect(Tokens.INTEGER) or self:_expect(Tokens.FLOAT)

        table.insert(body, { identifier = identifier, value = value })
    end

    if self:_expect(Tokens.END) == nil then
        self.log:print({
            "error",
            "Invalid enum syntax",
            location = token.location,
            information = "enum body need to be closed with `end`",
            code = "E157"
        })
        os.exit(157)
    end
end

function Parser:_peek_previous()
    return self.tokens[self.current - 1]
end

function Parser:_peek()
    return self.tokens[self.current]
end

function Parser:_advance()
    local token = self:_peek()
    self.current = self.current + 1
    return token
end

function Parser:_expect(expected)
    if not self:_eof() and self:_peek().kind == expected then
        return self:_advance()
    end
    return nil
end

function Parser:_eof()
    return self.current >= #self.tokens
end

return Parser
