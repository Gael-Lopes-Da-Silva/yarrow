# File summary with deliberate dirty spacing on the next line.
#No space after hash.
#  Extra spaces after hash.

"std.io" io require

Point struct
	i32 x public
	i32 y          # private by default
end

# Literal u8 coerces to i32 at the declaration site.
main function do
	42 answer mutable i32#jammed trailing
	5 10 < if
		"less" io.write_line call   # condition already on the stack
	end
end
