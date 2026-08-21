# Declaration type mismatch that cannot coerce (string into i32).

main function do
	# ERROR: string does not coerce to i32
	"nope" n mutable i32
end
