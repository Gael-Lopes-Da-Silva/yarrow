from sys import argv
from enum import Enum
from dataclasses import dataclass

class TokenType(Enum):
    L_PAREN = "L_paren"
    R_PAREN = "R_paren"
    COMMA = "Comma"
    DOT = "Dot"
    PLUS = "Plus"
    MINUS = "Minus"
    MULTIPLICATION = "Multiplication"
    DIVISION = "Division"
    EUCLIDIAN = "Euclidian"
    REMINDER = "Reminder"
    POWER = "Power"
    EQUAL_EQUAL = "Equal_equal"
    NOT_EQUAL = "Not_equal"
    GREATER = "Greater"
    GREATER_EQUAL = "Greater_equal"
    LESS = "Less"
    LESS_EQUAL = "Less_equal"
    BITWISE_AND = "Bitwise_and"
    BITWISE_OR = "Bitwise_or"
    BITWISE_XOR = "Bitwise_xor"
    L_SHIFT = "L_shift"
    R_SHIFT = "R_shift"
    IDENTIFIER = "Identifier"
    STRING = "String"
    INTEGER = "Integer"
    FLOAT = "Float"
    AND = "And"
    CALL = "Call"
    CASE = "Case"
    CATCH = "Catch"
    CONST = "Const"
    DEFAULT = "Default"
    DEFER = "Defer"
    DISCARD = "Discard"
    DO = "Do"
    ELSE = "Else"
    END = "End"
    ENUM = "Enum"
    FUNCTION = "Function"
    IF = "If"
    MATCH = "Match"
    MUTABLE = "Mutable"
    NOT = "Not"
    OR = "Or"
    PRIVATE = "Private"
    PROTECTED = "Protected"
    PUBLIC = "Public"
    REQUIRE = "Require"
    RETURN = "Return"
    STRUCT = "Struct"
    TRY = "Try"
    UNION = "Union"
    WHILE = "While"
    WITH = "With"
    DROP = "Drop"
    DUP = "Dup"
    OVER = "Over"
    ROT = "Rot"
    SET = "Set"
    SWAP = "Swap"

@dataclass
class Token:
    type: TokenType
    lexeme: str
    line: int
    start: int
    end: int

@dataclass
class Keyword:
    name: str
    token: TokenType

class Tokenizer:
    def __init__(self):
        self.start = 0
        self.current = 0
        self.line = 1
        self.column = 1
        self.path = ""
        self.source = ""
        self.tokens = []
        self.keywords = [
            Keyword("and", TokenType.AND),
            Keyword("call", TokenType.CALL),
            Keyword("case", TokenType.CASE),
            Keyword("catch", TokenType.TRY),
            Keyword("const", TokenType.CONST),
            Keyword("default", TokenType.DEFAULT),
            Keyword("defer", TokenType.DEFER),
            Keyword("discard", TokenType.DISCARD),
            Keyword("do", TokenType.DO),
            Keyword("drop", TokenType.DROP),
            Keyword("dup", TokenType.DUP),
            Keyword("else", TokenType.ELSE),
            Keyword("end", TokenType.END),
            Keyword("enum", TokenType.ENUM),
            Keyword("function", TokenType.FUNCTION),
            Keyword("if", TokenType.IF),
            Keyword("match", TokenType.MATCH),
            Keyword("mutable", TokenType.MUTABLE),
            Keyword("not", TokenType.NOT),
            Keyword("or", TokenType.OR),
            Keyword("over", TokenType.OVER),
            Keyword("private", TokenType.PRIVATE),
            Keyword("protected", TokenType.PROTECTED),
            Keyword("public", TokenType.PUBLIC),
            Keyword("require", TokenType.REQUIRE),
            Keyword("return", TokenType.RETURN),
            Keyword("rot", TokenType.ROT),
            Keyword("set", TokenType.SET),
            Keyword("struct", TokenType.STRUCT),
            Keyword("swap", TokenType.SWAP),
            Keyword("try", TokenType.TRY),
            Keyword("union", TokenType.UNION),
            Keyword("while", TokenType.WHILE),
            Keyword("with", TokenType.WITH),
        ]

    def eof(self) -> bool:
        return self.current >= len(self.source)

    def peek(self) -> str:
        return self.source[self.current]

    def peek_next(self) -> str:
        return self.source[self.current + 1]

    def advance(self) -> str:
        char = self.peek()
        self.current += 1
        self.column += 1
        return char

    def match(self, expected: str) -> bool:
        if self.eof() or self.peek() != expected: return False
        self.current += 1
        self.column += 1
        return True

    def get_keyword(self, key: str) -> TokenType | None:
        for keyword in self.keywords:
            if keyword.name == key: return keyword.token
        return None

    def add_token(self, type: TokenType) -> None:
        self.tokens.append(Token(
            type=type,
            lexeme=self.source[self.start:self.current],
            line=self.line,
            start=self.column,
            end=self.current,
        ))

    def scan_token(self) -> None:
        char = self.advance()
        match char:
            case "#":
                while not self.eof() and self.peek() != "\n": self.advance()

            case "\n" | "\r":
                self.line += 1
                self.column = 1

            case " " | "\t":
                pass

            case "(": self.add_token(TokenType.L_PAREN)
            case ")": self.add_token(TokenType.R_PAREN)
            case ",": self.add_token(TokenType.COMMA)
            case ".": self.add_token(TokenType.DOT)
            case "%": self.add_token(TokenType.REMINDER)
            case "&": self.add_token(TokenType.BITWISE_AND)
            case "|": self.add_token(TokenType.BITWISE_OR)
            case "^": self.add_token(TokenType.BITWISE_XOR)

            case "*": self.add_token(TokenType.POWER if self.match("*") else TokenType.MULTIPLICATION)
            case "/": self.add_token(TokenType.EUCLIDIAN if self.match("/") else TokenType.DIVISION)

            case "<":
                if self.match("="): self.add_token(TokenType.LESS_EQUAL)
                elif self.match("<"): self.add_token(TokenType.L_SHIFT)
                else: self.add_token(TokenType.LESS)

            case ">":
                if self.match("="): self.add_token(TokenType.GREATER_EQUAL)
                elif self.match(">"): self.add_token(TokenType.R_SHIFT)
                else: self.add_token(TokenType.GREATER)

            case "=":
                if self.match("="): self.add_token(TokenType.EQUAL_EQUAL)

            case "!":
                if self.match("="): self.add_token(TokenType.NOT_EQUAL)

            case "\"":
                self.handle_strings()

            case "-":
                if not self.eof() and self.peek().isdigit():
                    self.handle_numbers()
                else:
                    self.add_token(TokenType.MINUS)

            case "+":
                if not self.eof() and self.peek().isdigit():
                    self.handle_numbers()
                else:
                    self.add_token(TokenType.PLUS)

            case _ if char.isdigit():
                self.handle_numbers()

            case _ if char.isalpha() or char == "_":
                self.handle_identifiers()

            case _:
                # TODO: set error
                pass

    def handle_numbers(self) -> None:
        while not self.eof() and self.peek().isdigit(): self.advance()

        if not self.eof() and self.peek() == "." and self.peek_next().isdigit():
            self.advance()
            while not self.eof() and self.peek().isdigit(): self.advance()
            self.add_token(TokenType.FLOAT)
        else:
            self.add_token(TokenType.INTEGER)

    def handle_strings(self) -> None:
        while not self.eof() and self.peek() != "\"":
            # if self.peek() == "\\":
            #     self.advance()

            if self.peek() == "\n":
                # TODO: set error
                return
            self.advance()

        if self.eof():
            # TODO: set error
            return

        self.advance()
        self.add_token(TokenType.STRING)

    def handle_identifiers(self) -> None:
        while not self.eof() and (self.peek().isalnum() or self.peek() == "_"): self.advance()
        text = self.source[self.start:self.current]
        token_type = self.get_keyword(text) or TokenType.IDENTIFIER
        self.add_token(token_type)


    def tokenize(self, source: str) -> list:
        self.source = source
        self.current = 0
        self.start = 0
        self.tokens.clear()

        while not self.eof():
            self.start = self.current
            self.scan_token()

        return self.tokens

@dataclass
class Instruction:
    name: str
    value: any
    token: Token

class Parser:
    def __init__(self):
        self.instructions = []
        self.tokens = []

    def parse(self, tokens: list) -> list:
        self.instructions.clear()
        self.tokens = tokens

        for token in self.tokens:
            match token.type:
                case TokenType.INTEGER: self.instructions.append(Instruction("PushInt", int(token.lexeme), token))
                case TokenType.FLOAT: self.instructions.append(Instruction("PushFloat", float(token.lexeme), token))
                case TokenType.STRING: self.instructions.append(Instruction("PushString", token.lexeme[1:len(token.lexeme)-1], token))

                case TokenType.PLUS: self.instructions.append(Instruction("Plus", token.lexeme, token))
                case TokenType.MINUS: self.instructions.append(Instruction("Minus", token.lexeme, token))
                case TokenType.MULTIPLICATION: self.instructions.append(Instruction("Multiplication", token.lexeme, token))
                case TokenType.DIVISION: self.instructions.append(Instruction("Division", token.lexeme, token))
                case TokenType.EUCLIDIAN: self.instructions.append(Instruction("Euclidian", token.lexeme, token))
                case TokenType.REMINDER: self.instructions.append(Instruction("Reminder", token.lexeme, token))
                case TokenType.POWER: self.instructions.append(Instruction("Power", token.lexeme, token))

                case TokenType.DROP: self.instructions.append(Instruction("Drop", token.lexeme, token))
                case TokenType.DUP: self.instructions.append(Instruction("Dup", token.lexeme, token))
                case TokenType.OVER: self.instructions.append(Instruction("Over", token.lexeme, token))
                case TokenType.ROT: self.instructions.append(Instruction("Rot", token.lexeme, token))
                case TokenType.SWAP: self.instructions.append(Instruction("Swap", token.lexeme, token))

                case _:
                    # TODO: warn when token is found without instruction
                    pass

        return self.instructions

class Interpreter:
    def __init__(self):
        self.stack = []
        self.functions = []
        self.calls = []
        self.instructions = []

    def interpret(self, instructions: list) -> None:
        self.instructions = instructions

        for instruction in self.instructions:
            match instruction.name:
                case "PushInt": self.stack.append(instruction.value)
                case "PushFloat": self.stack.append(instruction.value)
                case "PushString": self.stack.append(instruction.value)

                case "Plus":
                    if len(self.stack) < 2:
                        # TODO: set error
                        break

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        break

                    self.stack.append(a + b)

                case "Minus":
                    if len(self.stack) < 2:
                        # TODO: set error
                        break

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        break

                    self.stack.append(a - b)

                case "Multiplication":
                    if len(self.stack) < 2:
                        # TODO: set error
                        break

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        break

                    self.stack.append(a * b)

                case "Division":
                    if len(self.stack) < 2:
                        # TODO: set error
                        break

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        break

                    self.stack.append(a / b)

                case "Euclidian":
                    if len(self.stack) < 2:
                        # TODO: set error
                        break

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        break

                    self.stack.append(a // b)

                case "Reminder":
                    if len(self.stack) < 2:
                        # TODO: set error
                        break

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        break

                    self.stack.append(a % b)

                case "Power":
                    if len(self.stack) < 2:
                        # TODO: set error
                        break

                    b = self.stack.pop()
                    a = self.stack.pop()

                    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):
                        # TODO: set error
                        break

                    self.stack.append(a ** b)

                case "Drop":
                    if len(self.stack) < 1:
                        # TODO: set error
                        break

                    self.stack.pop()

                case "Dup":
                    if len(self.stack) < 1:
                        # TODO: set error
                        break

                    self.stack.append(self.stack[-1])

                case "Over":
                    if len(self.stack) < 2:
                        # TODO: set error
                        break

                    self.stack.append(self.stack[-2])

                case "Rot":
                    if len(self.stack) < 3:
                        # TODO: set error
                        break

                    self.stack[-3], self.stack[-2], self.stack[-1] = self.stack[-2], self.stack[-1], self.stack[-3]

                case "Swap":
                    if len(self.stack) < 2:
                        # TODO: set error
                        break

                    self.stack[-1], self.stack[-2] = self.stack[-2], self.stack[-1]

                case _:
                    pass

class Compiler:
    def __init__(self):
        pass

    def compile(self, instructions: list) -> None:
        pass

class Cli:
    def __init__(self):
        pass

    def scan_file(self, path: str) -> None:
        try:
            with open(path, "r", encoding="utf-8") as file:
                source = file.read().strip()
        except Exception:
            # TODO: set error
            pass

        tokenizer = Tokenizer()
        parser = Parser()

        if source != "":
            tokenizer.path = path
            tokens = tokenizer.tokenize(source)
            instructions = parser.parse(tokens)
            print(instructions)

    def scan_prompt(self) -> None:
        tokenizer = Tokenizer()
        parser = Parser()
        interpreter = Interpreter()

        while True:
            try:
                source = input("> ").strip()
            except Exception:
                # TODO: set error
                break

            if source.lower() in ("quit", "exit"): break

            if source != "":
                tokens = tokenizer.tokenize(source)
                instructions = parser.parse(tokens)
                interpreter.interpret(instructions)
                print(instructions)
                print(interpreter.stack)

def main():
    cli = Cli()

    if len(argv) == 1: cli.scan_prompt()
    elif len(argv) == 2: cli.scan_file(argv[1])
    else:
        # TODO: print help or something
        pass

if __name__ == "__main__":
    main()
