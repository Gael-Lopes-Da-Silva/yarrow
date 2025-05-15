from tokenizer import Tokenizer
from parser import Parser

if __name__ == "__main__":
    with open("docs/syntax.yar", "r") as file:
        content = file.read()

    tokenizer = Tokenizer()
    parser = Parser()

    tokens = tokenizer.tokenize(content)
    instructions = parser.parse(tokens)

    for instruction in instructions:
        print(instruction)
