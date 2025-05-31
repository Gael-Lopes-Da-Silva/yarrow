local Instruction = {}
Instruction.__index = Instruction

function Instruction.new(name, content, token)
    local self = setmetatable({}, Instruction)
    self.name = name
    self.content = content
    self.token = token
    return self
end

return Instruction
