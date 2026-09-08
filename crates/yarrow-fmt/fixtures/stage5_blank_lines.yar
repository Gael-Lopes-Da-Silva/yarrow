# Dirty Stage 5 fixture: double blanks, missing item separators,
# and blanks after do / before end.

"std.io" io require


Point struct
	i32 x public
	i32 y public

end

Point implement
	distance public function
		reference<Point>
	do

		self const reference<Point>
		self.x self.x * self.y self.y * +
		return
	end with i32
end
helper function do

	1 2 +
	return
end with i32
main function do
	{x 3 y 4} p mutable Point
	p borrow
	p.distance call
	drop
end
