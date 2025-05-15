import math

from utils.enums.tokens import Tokens
from utils.enums.types import Types
from utils.instruction import Instruction
from utils.token import Token


class Parser:
    def __init__(self):
        self.tokens = []
        self.instructions = []
        self.current = 0

    def parse(self, tokens):
        self.tokens = tokens.copy()

        while not self.__eof():
            instruction = self.__parse_instruction()
            if instruction is not None:
                self.instructions.append(instruction)

        return self.instructions

    def __parse_instruction(self):
        token = self.__advance()

        match token.kind:
            case Tokens.IDENTIFIER:
                return Instruction(
                    "push",
                    {"type": Types.VOID, "value": token.lexeme},
                    token,
                )
            case Tokens.STRING:
                return Instruction(
                    "push",
                    {"type": Types.STRING, "value": str(token.lexeme[1:-1])},
                    token,
                )
            case Tokens.RUNE:
                return Instruction(
                    "push",
                    {"type": Types.RUNE, "value": str(token.lexeme[1:-1])},
                    token,
                )
            case Tokens.INTEGER:
                int_value = int(token.lexeme.replace("_", "").replace(",", ""))
                int_type = self.__get_smallest_integer_type(int_value)

                return Instruction(
                    "push",
                    {"type": int_type, "value": int_value},
                    token,
                )
            case Tokens.FLOAT:
                float_value = float(token.lexeme.replace("_", "").replace(",", ""))
                float_type = self.__get_smallest_float_type(float_value)

                return Instruction(
                    "push",
                    {"type": float_type, "value": float_value},
                    token,
                )
            case Tokens.BOOLEAN:
                return Instruction(
                    "push",
                    {"type": Types.BOOL, "value": token.lexeme.lower() == "true"},
                    token,
                )
            case Tokens.TYPE:
                return Instruction(
                    "push",
                    {
                        "type": Types.TYPE,
                        "value": self.__handle_types(default_token=token),
                    },
                    token,
                )

            case Tokens.PLUS:
                return Instruction(
                    "addition",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.MINUS:
                return Instruction(
                    "subtraction",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.ASTERISK:
                return Instruction(
                    "multiplication",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.SLASH:
                return Instruction(
                    "division",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.SLASH_SLASH:
                return Instruction(
                    "euclidian_division",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.PERCENT:
                return Instruction(
                    "remainder",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.CARET:
                return Instruction(
                    "power",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.QUESTION:
                return Instruction(
                    "default",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case Tokens.AND:
                return Instruction(
                    "and",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.OR:
                return Instruction(
                    "or",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.XOR:
                return Instruction(
                    "xor",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.NOT:
                return Instruction(
                    "not",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.LEFT_SHIFT:
                return Instruction(
                    "left_shift",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.RIGHT_SHIFT:
                return Instruction(
                    "right_shift",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case Tokens.EQUAL_EQUAL:
                return Instruction(
                    "equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.NOT_EQUAL:
                return Instruction(
                    "not_equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.GREATER:
                return Instruction(
                    "greater",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.GREATER_EQUAL:
                return Instruction(
                    "greater_equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.LESS:
                return Instruction(
                    "less",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.LESS_EQUAL:
                return Instruction(
                    "less_equal",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case Tokens.POP:
                return Instruction(
                    "pop",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.DROP:
                return Instruction(
                    "drop",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.DUP:
                return Instruction(
                    "dup",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.OVER:
                return Instruction(
                    "over",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.ROT:
                return Instruction(
                    "rot",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.SWAP:
                return Instruction(
                    "swap",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case Tokens.RETURN:
                return Instruction(
                    "return",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.CALL:
                return Instruction(
                    "call",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.BREAK:
                return Instruction(
                    "break",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.CONTINUE:
                return Instruction(
                    "continue",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.UNWRAP:
                return Instruction(
                    "unwrap",
                    {"type": None, "value": token.lexeme},
                    token,
                )
            case Tokens.SET:
                return Instruction(
                    "set",
                    {"type": None, "value": token.lexeme},
                    token,
                )

            case Tokens.MUTABLE:
                return self.__handle_mutables(token)
            case Tokens.CONST:
                return self.__handle_consts(token)
            case Tokens.STATIC:
                return self.__handle_statics(token)
            case Tokens.FUNCTION:
                return self.__handle_functions(token)
            case Tokens.IF:
                return self.__handle_if_elses(token)
            case Tokens.MATCH:
                return self.__handle_matchs(token)
            case Tokens.WHILE:
                return self.__handle_whiles(token)
            case Tokens.FOR:
                return self.__handle_fors(token)
            case Tokens.STRUCT:
                return self.__handle_structs(token)
            case Tokens.IMPLEMENT:
                return self.__handle_implements(token)
            case Tokens.ENUM:
                return self.__handle_enums(token)
            case Tokens.UNION:
                return self.__handle_unions(token)
            case Tokens.REQUIRE:
                return self.__handle_requires(token)
            case Tokens.DOT:
                return self.__handle_dots(token)
            case Tokens.DEFER:
                return self.__handle_defers(token)
            case Tokens.HANDLE:
                return self.__handle_handles(token)

            case Tokens.LEFT_SQUARE:
                return self.__handle_arrays(token)
            case Tokens.LEFT_CURLY:
                return self.__handle_hashmaps(token)
            case Tokens.LEFT_PAREN:
                return self.__handle_lists(token)

        return None

    def __handle_mutables(self, token):
        variable_type = self.__handle_types()

        return Instruction(
            "mutable",
            {
                "type": None,
                "value": variable_type,
            },
            token,
        )

    def __handle_consts(self, token):
        variable_type = self.__handle_types()

        return Instruction(
            "const",
            {
                "type": None,
                "value": variable_type,
            },
            token,
        )

    def __handle_statics(self, token):
        variable_type = self.__handle_types()

        return Instruction(
            "static",
            {
                "type": None,
                "value": variable_type,
            },
            token,
        )

    def __handle_functions(self, token):
        parameters = []
        while not self.__eof() and self.__peek().kind != Tokens.DO:
            if self.__peek().kind != Tokens.TYPE:
                # FIXME: add error
                pass

            parameter_type = self.__handle_types(no_error=True)
            parameters.append(parameter_type)

        if self.__eof() or self.__expect(Tokens.DO) is None:
            # FIXME: add error
            pass

        body = []
        while not self.__eof() and self.__peek().kind != Tokens.END:
            instruction = self.__parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return_type = None
        return_error = None
        if not self.__eof() and self.__expect(Tokens.WITH) is not None:
            return_type = self.__handle_types(no_error=True)
            if return_type is None:
                # FIXME: add error
                pass

            if not self.__eof() and self.__expect(Tokens.OR) is not None:
                return_error = self.__handle_types(no_error=True)
                if return_error is None:
                    # FIXME: add error
                    pass

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

    def __handle_if_elses(self, token):
        if_body = []
        while not self.__eof() and self.__peek().kind not in [
            Tokens.ELSE,
            Tokens.END,
        ]:
            instruction = self.__parse_instruction()
            if instruction is not None:
                if_body.append(instruction)

        else_body = []
        else_token = self.__expect(Tokens.ELSE)
        if not self.__eof() and else_token is not None:
            while not self.__eof() and self.__peek().kind != Tokens.END:
                instruction = self.__parse_instruction()
                if instruction is not None:
                    else_body.append(instruction)

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return Instruction(
            "if",
            {
                "type": None,
                "value": {"if": if_body, "else": else_body},
            },
            token,
        )

    def __handle_matchs(self, token):
        cases = []
        else_body = []
        while not self.__eof() and self.__peek().kind != Tokens.END:
            else_token = self.__expect(Tokens.ELSE)
            if else_token is not None:
                while not self.__eof() and self.__peek().kind != Tokens.END:
                    instruction = self.__parse_instruction()
                    if instruction is not None:
                        else_body.append(instruction)

                if self.__eof() or self.__expect(Tokens.END) is None:
                    # FIXME: add error
                    pass

                break

            case_condition = []
            while not self.__eof() and self.__peek().kind != Tokens.CASE:
                instruction = self.__parse_instruction()
                if instruction is not None:
                    case_condition.append(instruction)

            case_token = self.__expect(Tokens.CASE)
            if case_condition and case_token is None:
                # FIXME: add error
                pass
            elif not case_condition and case_token is not None:
                # FIXME: add error
                pass

            if not case_condition and case_token is None:
                break

            case_body = []
            while not self.__eof() and self.__peek().kind != Tokens.END:
                instruction = self.__parse_instruction()
                if instruction is not None:
                    case_body.append(instruction)

            if self.__eof() or self.__expect(Tokens.END) is None:
                # FIXME: add error
                pass

            cases.append({"condition": case_condition, "body": case_body})

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return Instruction(
            "match",
            {
                "type": None,
                "value": {"cases": cases, "else": else_body},
            },
            token,
        )

    def __handle_whiles(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.END:
            instruction = self.__parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return Instruction(
            "while",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def __handle_fors(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.END:
            instruction = self.__parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return Instruction(
            "for",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def __handle_structs(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.END:
            if self.__peek().kind not in [
                Tokens.TYPE,
                Tokens.IDENTIFIER,
            ]:
                # FIXME: add error
                pass

            variable_type = self.__handle_types(no_error=True)
            variable_name = self.__expect(Tokens.IDENTIFIER)

            if variable_type is None:
                # FIXME: add error
                pass
            elif variable_name is None:
                # FIXME: add error
                pass

            body.append(
                {"variable_name": variable_name, "variable_type": variable_type}
            )

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return Instruction(
            "struct",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def __handle_implements(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.END:
            if self.__peek().kind not in [
                Tokens.FUNCTION,
                Tokens.IDENTIFIER,
            ]:
                # FIXME: add error
                pass

            instruction = self.__parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return Instruction(
            "implement",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def __handle_enums(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.END:
            identifier = self.__expect(Tokens.IDENTIFIER)
            if identifier is None:
                # FIXME: add error
                pass

            value = self.__expect(Tokens.INTEGER) or self.__expect(Tokens.FLOAT)
            body.append({"identifier": identifier, "value": value})

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return Instruction(
            "enum",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def __handle_unions(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.END:
            union_type = self.__handle_types(no_error=True)
            if union_type is None:
                # FIXME: add error
                pass

            body.append(union_type)

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return Instruction(
            "union",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def __handle_requires(self, token):
        scope = self.__expect(Tokens.IDENTIFIER)

        return Instruction(
            "require",
            {
                "type": None,
                "value": {"scope": scope},
            },
            token,
        )

    def __handle_dots(self, token):
        identifier = self.__expect(Tokens.IDENTIFIER)
        if identifier is None:
            # FIXME: add error
            pass

        return Instruction(
            "dot",
            {
                "type": None,
                "value": {"identifier": identifier},
            },
            token,
        )

    def __handle_defers(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.END:
            instruction = self.__parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return Instruction(
            "defer",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def __handle_handles(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.END:
            instruction = self.__parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.__eof() or self.__expect(Tokens.END) is None:
            # FIXME: add error
            pass

        return Instruction(
            "handle",
            {
                "type": None,
                "value": {"body": body},
            },
            token,
        )

    def __handle_lists(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.RIGHT_PAREN:
            instruction = self.__parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.__eof() or self.__expect(Tokens.RIGHT_PAREN) is None:
            # FIXME: add error
            pass

        return Instruction(
            "list",
            {"body": body},
            token,
        )

    def __handle_arrays(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.RIGHT_SQUARE:
            instruction = self.__parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.__eof() or self.__expect(Tokens.RIGHT_SQUARE) is None:
            # FIXME: add error
            pass

        return Instruction(
            "array",
            {"body": body},
            token,
        )

    def __handle_hashmaps(self, token):
        body = []
        while not self.__eof() and self.__peek().kind != Tokens.RIGHT_CURLY:
            instruction = self.__parse_instruction()
            if instruction is not None:
                body.append(instruction)

        if self.__eof() or self.__expect(Tokens.RIGHT_CURLY) is None:
            # FIXME: add error
            pass

        return Instruction(
            "hashmap",
            {"body": body},
            token,
        )

    def __handle_types(self, no_error=False, default_token=None):
        variable_type = default_token
        if variable_type is None:
            variable_type = self.__expect(Tokens.TYPE) or self.__expect(Tokens.IDENTIFIER)
            if variable_type is None:
                if no_error:
                    return None

                # FIXME: add error
                pass

        key_type = None
        value_type = None
        contained_size = None
        if not self.__eof() and self.__expect(Tokens.LESS) is not None:
            key_type = self.__handle_types()
            value_type = self.__handle_types(no_error=True)

            if value_type is None:
                contained_size = self.__expect(Tokens.INTEGER)

            if self.__eof() or self.__expect(Tokens.GREATER) is None:
                # FIXME: add error
                pass

        return {
            "type": variable_type,
            "contained_type": {
                "key_type": key_type,
                "value_type": value_type,
            },
            "contained_size": contained_size,
        }

    def __get_smallest_integer_type(self, value):
        if value >= 0:
            if value <= 2**8 - 1:
                return Types.U8
            elif value <= 2**16 - 1:
                return Types.U16
            elif value <= 2**32 - 1:
                return Types.U32
            elif value <= 2**64 - 1:
                return Types.U64
            elif value <= 2**128 - 1:
                return Types.U128

        if -(2**7) <= value <= 2**7 - 1:
            return Types.I8
        elif -(2**15) <= value <= 2**15 - 1:
            return Types.I16
        elif -(2**31) <= value <= 2**31 - 1:
            return Types.I32
        elif -(2**63) <= value <= 2**63 - 1:
            return Types.I64
        else:
            return Types.I128

    def __get_smallest_float_type(self, value):
        if math.isnan(value) or math.isinf(value):
            # FIXME: add error
            pass

        if value == 0.0:
            return Types.F16

        abs_value = abs(value)

        if abs_value < 1e-307:
            return Types.F16

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
            return Types.F16
        elif abs_value <= 3.4e38 and significant_digits <= 7:
            return Types.F32
        elif abs_value <= 1.8e308 and significant_digits <= 16:
            return Types.F64
        else:
            return Types.F128

    def __peek_previous(self):
        return self.tokens[self.current - 1]

    def __peek(self):
        return self.tokens[self.current]

    def __advance(self):
        token = self.__peek()
        self.current += 1
        return token

    def __expect(self, expected):
        if not self.__eof() and self.__peek().kind == expected:
            return self.__advance()
        return None

    def __eof(self) -> bool:
        return self.current >= len(self.tokens)
