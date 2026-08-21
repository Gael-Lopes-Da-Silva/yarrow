# unsafe function + unsafe block, pointer<T> load/store, and std.mem.

"std.io" io require
"std.mem" mem require

Cell struct
	i32 value public
end

write_cell private unsafe function do
	unsafe
		32 mem.allocate call cp mutable pointer<Cell>
		cp.value 7 set
		cp.value
		drop
		cp mem.free call
	end
end

main function do
	unsafe
		16 mem.allocate call p mutable pointer<i32>
		p 42 store
		p load
		drop
		p mem.free call

		write_cell call
	end

	"unsafe ok" io.write_line call
end
