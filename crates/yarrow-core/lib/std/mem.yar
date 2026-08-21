# Manual memory management: raw allocation, deallocation and raw 64-bit
# word access. Every function here is unsafe: callers must be inside an
# `unsafe` block. The implementation wraps the compiler-level primitives
# (`@alloc`/`@free`/`@load`/`@store`) inside explicit `unsafe` regions.

allocate public unsafe function
	i64
do
	unsafe
		@alloc
	end
end with i64

free public unsafe function
	i64
do
	unsafe
		@free
	end
end

load public unsafe function
	i64
do
	unsafe
		@load
	end
end with i64

store public unsafe function
	i64
	i64
do
	unsafe
		@store
	end
end
