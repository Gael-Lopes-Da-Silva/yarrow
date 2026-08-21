# Freeing a region while a borrow of an attached value is still live.

"std.region" region require

main function do
	region.create call myRegion const i64
	(1 2 3) xs mutable list<i32>
	xs myRegion region.put call

	# ERROR: borrow from put is still on the stack; release it before free
	myRegion region.free call
end
