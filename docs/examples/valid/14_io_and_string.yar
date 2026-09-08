# std.io and std.string wrappers over host print / string helpers.

"std.io" io require
"std.string" str require

main function do
	"hello" str.len call 5 == if
		"len ok" io.write_line call
	else
		"len bad" io.write_line call
	end

	"foo" "bar" "-" str.join call
		io.write_line call
		"ab" "cd" str.concat call
		io.write_line call
		"aa" "ab" str.compare call
		0 < if
		"cmp ok" io.write call
		io.newline call
	else
		"cmp bad" io.write_line call
	end

	42 io.write_int call
	io.newline call
	3.5 io.write_float call
	io.newline call
end
