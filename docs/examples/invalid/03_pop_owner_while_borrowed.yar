# Popping / dropping an owner while a borrow of it is still on the stack.

main function do
	(1 2 3) xs mutable list<i32>
	xs borrow

	# ERROR: release the reference first (pop it), then the owner may be dropped
	xs pop
end
