# Aliased require, bare require into current scope, item import, and a local
# helper module. Resolve helpers relative to this file:
# helpers/greet.yar → "helpers.greet"

"std.io" io require
"std.math" math require
"std.math.sqrt" require
"helpers.greet" greet require

main function do
	"Ada" greet.hello call

	# Aliased module use
	16.0 math.sqrt call
	drop

	# Item import binds the function into the current scope
	25.0 sqrt call
	drop

	"modules ok" io.write_line call
end
