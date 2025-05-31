local Styles = require("utils.enums.styles")


local Log = {}
Log.__index = Log

function Log.new(args)
    local source = args[1] or args.source
    local path = args[2] or args.path

    local self = setmetatable({}, Log)
    self.source = source or nil
    self.path = path or nil
    self.pointer = "─"
    self.delimiter = "│"
    return self
end

function Log:print(args)
    local kind = args[1] or args.kind
    local message = args[2] or args.message
    local location = args[3] or args.location
    local information = args[4] or args.information
    local code = args[5] or args.code

    local color_map = {
        error = Styles.RED,
        warning = Styles.YELLOW,
        note = Styles.BLUE,
        debug = Styles.PURPLE
    }

    local color = color_map[kind:lower()] or Styles.BLACK_LIGHT
    local output = string.format("[%s%s%s%s] %s", color, Styles.BOLD, kind:upper(), Styles.DEFAULT, message)

    if code then
        output = output .. string.format(" %s%s%s", Styles.BLACK_LIGHT, code, Styles.DEFAULT)
    end

    if location and self.source then
        local lines = {}
        for line in string.gmatch(self.source, "[^\n]+") do
            table.insert(lines, line)
        end

        local line = string.format("%s\n%s  location:\n%s  %d%s %s\n%s  %s%s%s",
            Styles.BLACK_LIGHT,
            self.delimiter,
            self.delimiter,
            location[1],
            self.delimiter,
            lines[location[1]] or "",
            self.delimiter,
            color,
            string.rep(" ", #tostring(location[1]) + 2 + location[2]) ..
            string.rep(self.pointer, math.max(1, location[3] - location[2])),
            Styles.DEFAULT
        )

        if information then
            line = line .. string.format(" %s%s%s", color, information, Styles.DEFAULT)
        end

        output = output .. line
    end

    print(output)
end

return Log
