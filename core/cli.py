class Cli:
    def __init__(self) -> None:
        self.path = ""
        self.source = ""
        self.logger = Logger()

    def scan_file(self, path: str) -> None:
        self.path = path

        try:
            with open(self.path, "r", encoding="utf-8") as file:
                self.source = self.sanitize_source(file.read().strip())
        except Exception:
            self.logger.error(
                "No sush file or directory !",
                f"path: {self.path}",
            )
            exit(1)

        if self.source != "":
            try:
                tokenizer = Tokenizer(self.source, self.path)
                parser = Parser(self.source, self.path)
                compiler = Compiler(self.source, self.path)

                tokens = tokenizer.tokenize()
                instructions = parser.parse(tokens)
                compiler.compile(instructions)
            except Exception as error:
                if not isinstance(error, GlobalException):
                    raise

    def sanitize_source(self, source: str) -> str:
        return source.replace("\t", "").replace("\r\n", "\n").replace("\r", "\n")
