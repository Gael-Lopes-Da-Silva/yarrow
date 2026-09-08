# File summary stays at top.

# Stays with helper.
helper function do
	1 drop
end

main function do
	"inner" require
	0 drop
end

# Stays with Point.
Point implement
	noop public function do
	end
end

api public function do
	2 drop
end

Color enum
	RED
	GREEN
end

"helpers.greet" greet require
"std.io" io require

Point struct
	i32 x public
end
