# std.fs: write a temp file, read it back, print confirmation.
"std.io" io require
"std.fs" fs require
"std.string" str require

main function do
	"/tmp/yarrow_fs_stage23.txt" 'w' fs.open_file call unwrap out const File

	out borrow
	"stage23-ok" fs.write_file call unwrap

	out borrow
	fs.close_file call

	"/tmp/yarrow_fs_stage23.txt" 'r' fs.open_file call unwrap inp const File

	inp borrow
	fs.read_file call unwrap got const string

	inp borrow fs.close_file call got "stage23-ok" str.compare call 0 == if
		"fs ok" io.write_line call
	else
		"fs bad" io.write_line call
	end
end with |void error.Error|
