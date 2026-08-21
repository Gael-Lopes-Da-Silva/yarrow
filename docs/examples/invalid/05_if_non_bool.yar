# Stack / type error: if requires a bool on top, not an integer.

main function do
	42 if
		# ERROR: condition must be bool (e.g. after a comparison)
	end
end
