from tokenizer import Tokenizer

if __name__ == "__main__":
    with open("docs/syntax.yar", "r") as file:
        content = file.read()

    tokenizer = Tokenizer()
    tokens = tokenizer.tokenize(content)

    for token in tokens:
        print(token)
