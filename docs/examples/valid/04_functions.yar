# Nested function, parameter move vs copy, and call.

"std.io" io require

demo function do
	add function
		i32
		i32 copy
	do
		+
		return
	end with i32

	3 4 add call
	# Stack: [7]
	drop

	"add ok" io.write_line call
end

main function do
	demo call
end
