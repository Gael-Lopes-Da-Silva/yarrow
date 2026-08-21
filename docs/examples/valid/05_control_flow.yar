# if/else, value match, while-style for, and iterable for.

"std.io" io require
"std.loop" loop require

main function do
	5 10 < if
		"less" io.write_line call
	else
		"not less" io.write_line call
	end

	85 score const i32
	score match
		dup 85 == case
			"exact" io.write_line call
		end

		dup 50 < case
			"under 50" io.write_line call
		end

		else
			"other" io.write_line call
		end
	end

	0 i mutable i32
	i 3 < for
		i dup 1 + set
	end

	[10 20 30] numbers static array<i32 3>
	0 sum mutable i32
	numbers for
		sum dup loop.value + set
	end

	"control ok" io.write_line call
end
