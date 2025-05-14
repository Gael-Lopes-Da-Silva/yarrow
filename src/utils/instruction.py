class Instruction:
    def __init__(self, name, content, token):
        self.name = name
        self.content = content
        self.token = token

    def __repr__(self):
        return f"Instruction('{self.name}', {self.content})"
