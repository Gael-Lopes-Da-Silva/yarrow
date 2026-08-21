# Square root over f64, computed in pure Yarrow by Newton's method
# (`x_{n+1} = (g + n/g) / 2`), 32 fixed iterations. The parameter is bound
# to a variable first so it is read back through the variable instead of
# staying on the stack across the loop.
sqrt public function
	f64
do
	n mutable f64
	n 0.0 == if
		0.0
	else
		n 2.0 / guess mutable f64
		0 i mutable i64
		i 32 < for
			n guess / guess + 2.0 / next mutable f64
			next guess set
			i 1 + i set
		end
		guess
	end
end with f64
