# unsafe function + unsafe block, pointer<T> load/store, and std.mem.

"std.io" io require
"std.mem" mem require

Cell struct
	i32 value public
end

write_cell private unsafe function do
	unsafe
		32 mem.allocate cp mutable pointer<Cell>
		cp.value 7 set
		cp.value
		drop
		cp mem.free
	end
end

main function do
	unsafe
		16 mem.allocate p mutable pointer<i32>
		p 42 store
		p load
		drop
		p mem.free

		write_cell call
	end

	"unsafe ok" io.write_line call
end
