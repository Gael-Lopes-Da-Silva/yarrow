class Token:
    def __init__(self, kind, lexeme, location):
        self.kind = kind
        self.lexeme = lexeme
        self.location = location

    def __repr__(self):
        return f"Token({self.kind}, '{self.lexeme}', Location({self.location[0]}, {self.location[1]}, {self.location[2]}))"
