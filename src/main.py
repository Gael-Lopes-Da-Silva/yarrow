from tokenizer import Tokenizer

if __name__ == "__main__":
    tokenizer = Tokenizer()
    tokens = tokenizer.tokenize("test function i32 u64 type")
    print(tokens)
