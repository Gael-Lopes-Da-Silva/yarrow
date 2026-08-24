# Stage 10 gate: two independent errors in one compile.

"std.list" list require

bad_move function do
	(1 2 3) xs mutable list<i32>
	() ys mutable list<i32>
	xs ys move

	# ERROR: use after move
	xs list.len call
end

main function do
	# ERROR: if condition is not bool
	1 if
		0
	end
end
