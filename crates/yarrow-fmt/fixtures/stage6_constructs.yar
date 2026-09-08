# Dirty Stage 6 fixture: jammed require, inline struct fields,
# params on one line, split end/with, jammed bindings.

"std.io"io require
"std.list"list require

Point struct i32 x public i32 y public end

Color enum RED GREEN BLUE end

Value union i32 string end

add function i32 i32 copy do
+
return
end
with i32

Point implement
distance public function reference<Point> do
self const reference<Point>
self.x self.x * self.y self.y * +
return
end with i32
end

main function do
{x 3 y 4}p mutable Point
[10 20 30]numbers static array<i32 3>
(10 20)ys mutable list<i32>
3
4
add call
drop
"ok"io.write_line call
end
