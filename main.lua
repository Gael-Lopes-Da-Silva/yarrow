local Log = require("utils.log")
local Tokenizer = require("core.tokenizer")

local path = ""
local source = "function end"

local log = Log.new({source, path})
local tokenizer = Tokenizer.new({log})

local tokens = tokenizer:tokenize({source})
