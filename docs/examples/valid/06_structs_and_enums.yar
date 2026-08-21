# Struct literal, implement block, borrow + method call, and enum match.

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

Color enum
	RED
	GREEN
	BLUE
end

main function do
	{x 3 y 4} p mutable Point
	p borrow
	p.distance call
	# Stack: [25]
	drop

	Color.GREEN c const Color
	c match
		dup Color.RED == case
			"red" io.write_line call
		end

		dup Color.GREEN == case
			"green" io.write_line call
		end

		else
			"other color" io.write_line call
		end
	end
end
