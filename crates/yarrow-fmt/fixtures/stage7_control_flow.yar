# Dirty Stage 7 fixture: jammed if/match/for, split defer, loose handle.

"std.io" io require

AppError error NOT_FOUND BAD_INPUT end

lookup function i32 do
key const i32
key 0 == if
AppError.NOT_FOUND return
else
key 10 * return
end
end with |i32 AppError|

main function do
5 10 < if
"less" io.write_line call
else
"not less" io.write_line call
end

85 score const i32
score match
dup 85 == case
"exact" io.write_line call
end
dup 50 < case
"under 50" io.write_line call
end
else
"other" io.write_line call
end
end

0 i mutable i32
i 3 < for
i 1 + i set
end

defer "bye" io.write_line call end

0 lookup call handle
match
AppError.NOT_FOUND case
"missing" io.write_line call
end
else
"other" io.write_line call
end
end
-1 fallback
end
drop

0 lookup call handle 0 fallback end
drop

unsafe
1 drop
end
end
