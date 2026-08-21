# Mutating (or consuming) a value while a borrow is still live.

"std.list" list require

main function do
	(1 2 3) xs mutable list<i32>
	xs borrow

	# ERROR: xs is borrowed; cannot mutate until the reference is released
	xs 4 list.push_last call unwrap
end
