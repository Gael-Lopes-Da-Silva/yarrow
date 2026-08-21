# Calling an unsafe function outside an unsafe block.

"std.mem" mem require

touch private unsafe function do
	unsafe
		8 mem.allocate call
		mem.free call
	end
end

main function do
	# ERROR: unsafe function requires an enclosing unsafe ... end
	touch call
end
