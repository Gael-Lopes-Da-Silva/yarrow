local Cli = {}
Cli.__index = Cli

function Cli.new()
    local self = setmetatable({}, Cli)
    self.path = ""
    self.source = ""
    return self
end

function Cli:sanitize_source(source)
    return source:gsub("\t", ""):gsub("\r\n", "\n"):gsub("\r", "\n")
end

return Cli
