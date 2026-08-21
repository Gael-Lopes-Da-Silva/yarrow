# Named union: store either member, match with Type case, autoderef in arms.

"std.io" io require

Value union
	i32
	string
end

main function do
	42 v mutable Value
	"hello" v set

	v match
		i32 case
			dup *
			pop
		end

		string case
			msg const reference<string>
			msg "!" ~
			io.write_line call
		end

		else
		end
	end
end
