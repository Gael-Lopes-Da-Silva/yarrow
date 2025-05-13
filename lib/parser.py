class Parser:
    def __init__(self, source: str, path: str) -> None:
        self.source = source
        self.path = path
        self.logger = Logger(source, path)
        self.tokens = []
        self.instructions = []
        self.current = 0

    def __repr__(self) -> str:
        return f"{self.instructions}"

    def parse(self, tokens: list) -> list:
        self.tokens = tokens.copy()

        while not self.eof():
            instruction = self.parse_instruction()
            if instruction is not None:
                self.instructions.append(instruction)

        return self.instructions

    def parse_instruction(self) -> Instruction | None:
        token = self.advance()

        match token.kind:
            case TokenKind.IDENTIFIER:
                return Instruction(
                    "push",
                    {"type": TypeKind.VOID, "value": token.lexeme},
                    token,
                )
            case TokenKind.STRING:
                return Instruction(
                    "push",
                    {"type": TypeKind.STRING, "value": str(token.lexeme[1:-1])},
                    token,
                )
            case TokenKind.RUNE:
                return Instruction(
                    "push",
                    {"type": TypeKind.RUNE, "value": str(token.lexeme[1:-1])},
                    token,
                )
            case TokenKind.INTEGER:
                int_value = int(token.lexeme)
                int_type = self.get_smallest_integer_type(int_value)

                return Instruction(
                    "push",
                    {"type": int_type, "value": int_value},
                    token,
                )
            case TokenKind.FLOAT:
                float_value = float(token.lexeme)
                float_type = self.get_smallest_float_type(float_value)

                return Instruction(
                    "push",
                    {"type": float_type, "value": float_value},
                    token,
                )
            case TokenKind.BOOLEAN:
                return Instruction(
                    "push",
                    {"type": TypeKind.BOOL, "value": token.lexeme.lower() == "true"},
                    token,
                )
            case TokenKind.TYPE:
                return Instruction(
                    "push",
                    {
                        "type": TypeKind.TYPE,
                        "value": self.parse_type(default_token=token),
                    },
                    token,
                )

            case TokenKind.PLUS:
                return Instruction(
                    "addition",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.MINUS:
                return Instruction(
                    "subtraction",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.STAR:
                return Instruction(
                    "multiplication",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.SLASH:
                return Instruction(
                    "division",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.SLASH_SLASH:
                return Instruction(
                    "euclidian_division",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.PERCENT:
                return Instruction(
                    "remainder",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.CARET:
                return Instruction(
                    "power",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.QUESTION:
                return Instruction(
                    "default",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case TokenKind.AND:
                return Instruction(
                    "and",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.OR:
                return Instruction(
                    "or",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.XOR:
                return Instruction(
                    "xor",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.NOT:
                return Instruction(
                    "not",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.LEFT_SHIFT:
                return Instruction(
                    "left_shift",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.RIGHT_SHIFT:
                return Instruction(
                    "right_shift",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case TokenKind.EQUAL_EQUAL:
                return Instruction(
                    "equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.NOT_EQUAL:
                return Instruction(
                    "not_equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.GREATER:
                return Instruction(
                    "greater",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.GREATER_EQUAL:
                return Instruction(
                    "greater_equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.LESS:
                return Instruction(
                    "less",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.LESS_EQUAL:
                return Instruction(
                    "less_equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case TokenKind.POP:
                return Instruction(
                    "pop",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.DROP:
                return Instruction(
                    "drop",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.DUP:
                return Instruction(
                    "dup",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.OVER:
                return Instruction(
                    "over",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.ROT:
                return Instruction(
                    "rot",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.SWAP:
                return Instruction(
                    "swap",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case TokenKind.RETURN:
                return Instruction(
                    "return",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.CALL:
                return Instruction(
                    "call",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.BREAK:
                return Instruction(
                    "break",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.CONTINUE:
                return Instruction(
                    "continue",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.UNWRAP:
                return Instruction(
                    "unwrap",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case TokenKind.SET:
                return Instruction(
                    "set",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case TokenKind.MUTABLE:
                return self.handle_mutables(token)
            case TokenKind.CONST:
                return self.handle_consts(token)
            case TokenKind.STATIC:
                return self.handle_statics(token)
            case TokenKind.FUNCTION:
                return self.handle_functions(token)
            case TokenKind.IF:
                return self.handle_if_elses(token)
            case TokenKind.MATCH:
                return self.handle_matchs(token)
            case TokenKind.WHILE:
                return self.handle_whiles(token)
            case TokenKind.FOR:
                return self.handle_fors(token)
            case TokenKind.STRUCT:
                return self.handle_structs(token)
            case TokenKind.IMPLEMENT:
                return self.handle_implements(token)
            case TokenKind.ENUM:
                return self.handle_enums(token)
            case TokenKind.UNION:
                return self.handle_unions(token)
            case TokenKind.REQUIRE:
                return self.handle_requires(token)
            case TokenKind.DOT:
                return self.handle_dots(token)
            case TokenKind.DEFER:
                return self.handle_defers(token)
            case TokenKind.HANDLE:
                return self.handle_handles(token)

            case TokenKind.LEFT_SQUARE:
                return self.handle_arrays(token)
            case TokenKind.LEFT_CURLY:
                return self.handle_hashmaps(token)
            case TokenKind.LEFT_PAREN:
                return self.handle_lists(token)

        return None

    def handle_mutables(self, token: Token) -> Instruction:
        variable_type = self.parse_type()

        return Instruction(
            "mutable",
            {
                "type": None,
                "value": variable_type,
            },
            token,
        )

    def handle_consts(self, token: Token) -> Instruction:
        variable_type = self.parse_type()

        return Instruction(
            "const",
            {
                "type": None,
                "value": variable_type,
            },
            token,
        )

    def handle_statics(self, token: Token) -> Instruction:
        variable_type = self.parse_type()

        return Instruction(
            "static",
            {
                "type": None,
                "value": variable_type,
            },
            token,
        )

    def handle_functions(self, token: Token) -> Instruction:
        parameters = []
        while not self.eof() and self.peek().kind != TokenKind.DO:
            if self.peek().kind != TokenKind.TYPE:
                self.logger.error(
                    "Invalid function syntax !",
                    location=self.peek().location,
                    location_message="parameters are only composed of types",
                )
                raise GlobalException

            parameter_type = self.parse_type(no_error=True)
            parameters.append(parameter_type)

        if self.eof() or self.expect(TokenKind.DO) is None:
            self.logger.error(
                "Invalid function syntax !",
                location=self.peek_previous().location,
                location_message="there should be a function body after this",
            )
            self.logger.info(
                "Open a function body with a `do` and close it with a `end` !"
            )
            raise GlobalException

        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Function not closed !",
                location=self.peek_previous().location,
                location_message="you need to close a function with `end`",
            )
            raise GlobalException

        return_type = None
        return_error = None
        if not self.eof() and self.expect(TokenKind.WITH) is not None:
            return_type = self.parse_type(no_error=True)
            if return_type is None:
                self.logger.error(
                    "Invalid function syntax !",
                    location=self.peek_previous().location,
                    location_message="there should be a type after this",
                )
                self.logger.info(
                    "If you don't want to specify a return type, don't put a `with`. It will return `void` by default !"
                )
                raise GlobalException

            if not self.eof() and self.expect(TokenKind.OR) is not None:
                return_error = self.parse_type(no_error=True)
                if return_error is None:
                    self.logger.error(
                        "Invalid function syntax !",
                        location=self.peek_previous().location,
                        location_message="there should be an error type after this",
                    )
                    raise GlobalException

        return Instruction(
            "function",
            {
                "type": None,
                "value": {
                    "parameters": parameters,
                    "body": body,
                    "return_type": return_type,
                    "return_error": return_error,
                },
            },
            token,
        )

    def handle_if_elses(self, token: Token) -> Instruction:
        if_body = []
        while not self.eof() and self.peek().kind not in [
            TokenKind.ELSE,
            TokenKind.END,
        ]:
            instruction = self.parse_instruction()
            if instruction is not None:
                if_body.append(instruction)

        else_body = []
        else_token = self.expect(TokenKind.ELSE)
        if not self.eof() and else_token is not None:
            while not self.eof() and self.peek().kind != TokenKind.END:
                instruction = self.parse_instruction()
                if instruction is not None:
                    else_body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "If statement not closed !",
                location=else_token.location
                if else_token is not None
                else token.location,
                location_message="you need to close an if statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "if",
            {
                "type": None,
                "value": {"if": if_body, "else": else_body},
            },
            token,
        )

    def handle_matchs(self, token: Token) -> Instruction:
        cases = []
        else_body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            else_token = self.expect(TokenKind.ELSE)
            if else_token is not None:
                while not self.eof() and self.peek().kind != TokenKind.END:
                    instruction = self.parse_instruction()
                    if instruction is not None:
                        else_body.append(instruction)

                if self.eof() or self.expect(TokenKind.END) is None:
                    self.logger.error(
                        "Case not closed !",
                        location=else_token.location,
                        location_message="you need to close a case with `end`",
                    )
                    raise GlobalException

                break

            case_condition = []
            while not self.eof() and self.peek().kind != TokenKind.CASE:
                instruction = self.parse_instruction()
                if instruction is not None:
                    case_condition.append(instruction)

            case_token = self.expect(TokenKind.CASE)
            if case_condition and case_token is None:
                self.logger.error(
                    "Invalid match syntax !",
                    location=case_condition[-1].token.location,
                    location_message="there should be a case body after this",
                )
                raise GlobalException
            elif not case_condition and case_token is not None:
                self.logger.error(
                    "Invalid match syntax !",
                    location=self.peek_previous().location,
                    location_message="there should be a condition before this",
                )
                raise GlobalException

            if not case_condition and case_token is None:
                break

            case_body = []
            while not self.eof() and self.peek().kind != TokenKind.END:
                instruction = self.parse_instruction()
                if instruction is not None:
                    case_body.append(instruction)

            if self.eof() or self.expect(TokenKind.END) is None:
                self.logger.error(
                    "Case not closed !",
                    location=case_token.location,
                    location_message="you need to close a case with `end`",
                )
                raise GlobalException

            cases.append({"condition": case_condition, "body": case_body})

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Match statement not closed !",
                location=token.location,
                location_message="you need to close a match statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "match",
            {
                "type": None,
                "value": {"cases": cases, "else": else_body},
            },
            token,
        )

    def handle_whiles(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "While statement not closed !",
                location=token.location,
                location_message="you need to close a while statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "while",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_fors(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "For statement not closed !",
                location=token.location,
                location_message="you need to close a for statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "for",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_structs(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            if self.peek().kind not in [
                TokenKind.TYPE,
                TokenKind.IDENTIFIER,
            ]:
                self.logger.error(
                    "Invalid struct syntax !",
                    location=self.peek().location,
                    location_message="struct fields are composed of a type followed by an identifier",
                )
                raise GlobalException

            variable_type = self.parse_type(no_error=True)
            variable_name = self.expect(TokenKind.IDENTIFIER)

            if variable_type is None:
                self.logger.error(
                    "Invalid struct syntax !",
                    location=variable_name.location,
                    location_message="there should be a type before this",
                )
                raise GlobalException
            elif variable_name is None:
                self.logger.error(
                    "Invalid struct syntax !",
                    location=variable_type.location,
                    location_message="there should be an identifier after this",
                )
                raise GlobalException

            body.append(
                {"variable_name": variable_name, "variable_type": variable_type}
            )

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Struct statement not closed !",
                location=token.location,
                location_message="you need to close a struct statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "struct",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_implements(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            if self.peek().kind not in [
                TokenKind.FUNCTION,
                TokenKind.IDENTIFIER,
            ]:
                self.logger.error(
                    "Invalid implement syntax !",
                    location=self.peek().location,
                    location_message="implement are composed of functions only",
                )
                raise GlobalException

            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Implement statement not closed !",
                location=token.location,
                location_message="you need to close an implement statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "implement",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_enums(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            identifier = self.expect(TokenKind.IDENTIFIER)
            if identifier is None:
                self.logger.error(
                    "Invalid enum syntax !",
                    location=self.peek().location,
                    location_message="there should be an identifier here",
                )
                self.logger.info(
                    "After an identifier, you can give an integer or a float to start the enum from !"
                )
                raise GlobalException

            value = self.expect(TokenKind.INTEGER) or self.expect(TokenKind.FLOAT)
            body.append({"identifier": identifier, "value": value})

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Enum statement not closed !",
                location=token.location,
                location_message="you need to close an enum statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "enum",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_unions(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            union_type = self.parse_type(no_error=True)
            if union_type is None:
                self.logger.error(
                    "Invalid union syntax !",
                    location=self.peek().location,
                    location_message="there should be a type here",
                )
                raise GlobalException

            body.append(union_type)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Union statement not closed !",
                location=token.location,
                location_message="you need to close an union statement with `end`",
            )
            raise GlobalException

        return Instruction(
            "union",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_requires(self, token: Token) -> Instruction:
        scope = self.expect(TokenKind.IDENTIFIER)

        return Instruction(
            "require",
            {
                "type": None,
                "value": {"scope": scope},
            },
            token,
        )

    def handle_dots(self, token: Token) -> Instruction:
        identifier = self.expect(TokenKind.IDENTIFIER)
        if identifier is None:
            self.logger.error(
                "Invalid dot syntax !",
                location=token.location,
                location_message="there should be an identifier after this",
            )
            raise GlobalException

        return Instruction(
            "dot",
            {
                "type": None,
                "value": {"identifier": identifier},
            },
            token,
        )

    def handle_defers(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Defer not closed !",
                location=token.location,
                location_message="you need to close a defer with `end`",
            )
            raise GlobalException

        return Instruction(
            "defer",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_handles(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.END:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.END) is None:
            self.logger.error(
                "Handle not closed !",
                location=token.location,
                location_message="you need to close a handle with `end`",
            )
            raise GlobalException

        return Instruction(
            "handle",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def handle_lists(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.RIGHT_PAREN:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.RIGHT_PAREN) is None:
            self.logger.error(
                "List not closed !",
                location=token.location,
                location_message="you need to close a list with `)`",
            )
            raise GlobalException

        return Instruction(
            "list",
            {"body": body},
            token,
        )

    def handle_arrays(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.RIGHT_SQUARE:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.RIGHT_SQUARE) is None:
            self.logger.error(
                "Array not closed !",
                location=token.location,
                location_message="you need to close an array with `]`",
            )
            raise GlobalException

        return Instruction(
            "array",
            {"body": body},
            token,
        )

    def handle_hashmaps(self, token: Token) -> Instruction:
        body = []
        while not self.eof() and self.peek().kind != TokenKind.RIGHT_CURLY:
            instruction = self.parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.eof() or self.expect(TokenKind.RIGHT_CURLY) is None:
            self.logger.error(
                "Hashmap not closed !",
                location=token.location,
                location_message="you need to close an hashmap with `}`",
            )
            raise GlobalException

        return Instruction(
            "hashmap",
            {"body": body},
            token,
        )

    def get_smallest_integer_type(self, value: int) -> TypeKind:
        if value >= 0:
            if value <= 2**8 - 1:
                return TypeKind.U8
            elif value <= 2**16 - 1:
                return TypeKind.U16
            elif value <= 2**32 - 1:
                return TypeKind.U32
            elif value <= 2**64 - 1:
                return TypeKind.U64
            elif value <= 2**128 - 1:
                return TypeKind.U128

        if -(2**7) <= value <= 2**7 - 1:
            return TypeKind.I8
        elif -(2**15) <= value <= 2**15 - 1:
            return TypeKind.I16
        elif -(2**31) <= value <= 2**31 - 1:
            return TypeKind.I32
        elif -(2**63) <= value <= 2**63 - 1:
            return TypeKind.I64
        else:
            return TypeKind.I128

    def get_smallest_float_type(self, value: float) -> TypeKind:
        if math.isnan(value) or math.isinf(value):
            self.logger.error(
                f"Invalid float value {value} (NaN or Infinity) !",
                location=self.get_location(),
                location_message="need to be a valid float value",
            )
            raise GlobalException

        if value == 0.0:
            return TypeKind.F16

        abs_value = abs(value)

        if abs_value < 1e-307:
            return TypeKind.F16

        significant_digits = 0
        if abs_value != 0.0:
            order = math.floor(math.log10(abs_value))
            normalized = abs_value / (10**order)

            temp = normalized
            epsilon = 1e-10
            max_digits = 34
            while temp > epsilon and significant_digits < max_digits:
                temp *= 10
                digit = int(temp)
                significant_digits += 1
                temp -= digit
                if abs(temp) < epsilon:
                    break

        if significant_digits > 0:
            str_value = f"{abs_value:.16e}".split("e")[0].replace(".", "").rstrip("0")
            significant_digits = min(significant_digits, len(str_value))

        if abs_value <= 65504 and significant_digits <= 4:
            return TypeKind.F16
        elif abs_value <= 3.4e38 and significant_digits <= 7:
            return TypeKind.F32
        elif abs_value <= 1.8e308 and significant_digits <= 16:
            return TypeKind.F64
        else:
            return TypeKind.F128

    def parse_type(
        self, no_error: bool = False, default_token: Token | None = None
    ) -> dict | None:
        variable_type = default_token
        if variable_type is None:
            variable_type = self.expect(TokenKind.TYPE) or self.expect(
                TokenKind.IDENTIFIER
            )
            if variable_type is None:
                if no_error:
                    return None

                self.logger.error(
                    "Invalid type syntax !",
                    location=self.peek_previous().location,
                    location_message="there should be a type after this",
                )
                raise GlobalException

        key_type = None
        value_type = None
        contained_size = None
        if not self.eof() and self.expect(TokenKind.LESS) is not None:
            key_type = self.parse_type()
            value_type = self.parse_type(no_error=True)

            if value_type is None:
                contained_size = self.expect(TokenKind.INTEGER)

            if self.eof() or self.expect(TokenKind.GREATER) is None:
                self.logger.error(
                    "Invalid type syntax !",
                    location=variable_type.location,
                    location_message="you need to close a contained type definition with `>`",
                )
                raise GlobalException

        return {
            "type": variable_type,
            "contained_type": {
                "key_type": key_type,
                "value_type": value_type,
            },
            "contained_size": contained_size,
        }

    def peek_previous(self) -> Token:
        return self.tokens[self.current - 1]

    def peek(self) -> Token:
        return self.tokens[self.current]

    def advance(self) -> Token:
        token = self.peek()
        self.current += 1
        return token

    def expect(self, expected_type: TokenKind) -> Token | None:
        if not self.eof() and self.peek().kind == expected_type:
            return self.advance()
        return None

    def eof(self) -> bool:
        return self.current >= len(self.tokens)
