# Containers: list, array, hashmap literals and typed empty containers.

"std.io" io require
"std.list" list require

main function do
	[1 2 3] xs static array<i32 3>

	(10 20) ys mutable list<i32>
	ys 30 list.push_last call unwrap

	{"a" 1 "b" 2} m static hashmap<string i32>

	() empty mutable list<i32>
	{} map mutable hashmap<string i32>

	"containers ok" io.write_line call
end
