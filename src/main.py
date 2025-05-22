from utils.log import Log
from tokenizer import Tokenizer
from parser import Parser

if __name__ == "__main__":
    path = "test.yar"

    with open(path, "r") as file:
        content = file.read()

    log = Log(content, path)
    tokenize = Tokenizer(log)
    parse = Parser(log)

    tokens = tokenize(content)
    instructions = parse(tokens)

    for instruction in instructions:
        print(instruction)
