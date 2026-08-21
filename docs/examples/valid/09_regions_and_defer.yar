# Region batch free and defer at scope exit (reverse registration order).

"std.io" io require
"std.region" region require

Person struct
	string name public
end

main function do
	myRegion region.create call
	defer myRegion region.free call end

	{name "Ada"} person mutable Person
	person myRegion region.put call

	defer
		"leaving scope" io.write_line call
	end

	"in scope" io.write_line call
end
