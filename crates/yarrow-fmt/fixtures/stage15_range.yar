# Dirty Stage 15 fixture: messy `lookup` between tidy neighbors.
# Range-format should expand to the `lookup` item only.

"std.io" io require

AppError error
	NOT_FOUND
	BAD_INPUT
end

# Messy helper body / jammed keywords.
lookup function i32 do
key const i32
key 0 == if
AppError.NOT_FOUND return
else
key 10 * return
end
end with |i32 AppError|

main function do
	5 10 < if
		"less" io.write_line call
	else
		"not less" io.write_line call
	end
	0 drop
end
