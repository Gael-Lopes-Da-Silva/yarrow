from utils.log import Log
from tokenizer import Tokenizer
from parser import Parser

if __name__ == "__main__":
    with open("docs/syntax.yar", "r") as file:
        content = file.read()

    log = Log(content, "docs/syntax.yar")
    tokenize = Tokenizer(log)
    parse = Parser(log)

    tokens = tokenize(content)
    instructions = parse(tokens)
