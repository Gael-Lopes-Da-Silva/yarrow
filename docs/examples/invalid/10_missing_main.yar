# Programs must define main.

"std.io" io require

# ERROR: missing main function
helper function do
	"no entry" io.write_line call
end
