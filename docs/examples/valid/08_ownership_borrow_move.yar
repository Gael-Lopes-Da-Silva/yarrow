# Stack ownership, variable ownership, borrow release, and move.

"std.io" io require
"std.list" list require

main function do
	# Stack owns the temporary until pop.
	"temp"
	pop

	"hello" s mutable string
	"world" s set

	(1 2 3) xs mutable list<i32>
	xs borrow
	pop
	xs 4 list.push_last call unwrap

	() ys mutable list<i32>
	xs ys move
	ys 5 list.push_last call unwrap

	"ownership ok" io.write_line call
end
