# Use after move: source must not be used once ownership transferred.

"std.io" io require
"std.list" list require

main function do
	(1 2 3) a mutable list<i32>
	() b mutable list<i32>
	a b move

	# ERROR: a no longer owns the list
	a 4 list.push_last call unwrap
end
