# Stage 10 gate: two independent syntax errors with parser recovery.

main function do
	# ERROR: set without a target
	set
	0
end

other function do
	# ERROR: move without a target variable
	move
	0
end
