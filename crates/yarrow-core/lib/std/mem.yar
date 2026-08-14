# Manual memory management: raw allocation, deallocation and raw 64-bit
# word access. Every function here is unsafe: callers must be inside an
# `unsafe` block. The implementation wraps the compiler-level primitives
# (`@alloc`/`@free`/`@load`/`@store`) inside explicit `unsafe` regions.

alloc unsafe function
    i64
do
    unsafe
        @alloc
    end
end with i64

free unsafe function
    i64
do
    unsafe
        @free
    end
end

load unsafe function
    i64
do
    unsafe
        @load
    end
end with i64

store unsafe function
    i64
    i64
do
    unsafe
        @store
    end
end
