local Token = {}
Token.__index = Token

function Token.new(kind, lexeme, location)
    local self = setmetatable({}, Token)
    self.kind = kind
    self.lexeme = lexeme
    self.location = location
    return self
end

return Token
