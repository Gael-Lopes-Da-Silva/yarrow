# Only one active borrow of a value is allowed.

main function do
	(1 2 3) xs mutable list<i32>
	xs borrow
	# ERROR: second overlapping borrow of xs
	xs borrow
end
