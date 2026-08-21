# Aliased require, bare require into current scope, and a local helper module.
# Resolve helpers relative to this file: helpers/greet.yar → "helpers.greet"

"std.io" io require
"std.math" math require
"helpers.greet" greet require

main function do
	"Ada" greet.hello call

	# Item-style / scoped std use
	16.0 math.sqrt call
	drop

	"modules ok" io.write_line call
end
