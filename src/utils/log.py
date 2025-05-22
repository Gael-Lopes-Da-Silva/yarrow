from utils.style import Style


class Log:
    def __init__(self, source=None, path=None):
        self.source = source
        self.path = path
        self.pointer = "─"
        self.delimiter = "|"
        self.line_delemiter = "│"

    def __call__(self, kind, message, location=None, information=None, code=None):
        color_map = {
            "error": Style.RED,
            "warning": Style.YELLOW,
            "note": Style.BLUE,
            "debug": Style.PURPLE,
        }

        color = color_map.get(kind.lower(), Style.BLACK_LIGHT)
        output = f"[{color}{Style.BOLD}{kind.upper()}{Style.DEFAULT}] {message}"

        if code:
            output += f" {Style.BLACK_LIGHT}{code}{Style.DEFAULT}"

        if location and self.source:
            line = f"{Style.BLACK_LIGHT}"
            line += f"\n{self.delimiter}  location:"
            line += f"\n{self.delimiter}  {location[0]}{self.line_delemiter} {self.source.splitlines()[location[0] - 1]}"
            line += f"\n{self.delimiter}  {color}{' ' * (len(str(location[0])) + 2 + location[1]) + self.pointer * max(1, location[2] - location[1])}{Style.DEFAULT}"

            if information:
                line += f" {color}{information}{Style.DEFAULT}"

            output += line

        print(output)
