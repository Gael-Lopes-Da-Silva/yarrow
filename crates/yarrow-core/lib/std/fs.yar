# Minimal filesystem surface used by the grammar tour. `open_file` /
# `close_file` wrap host file handles; open is fallible.

"std.error" error require

File struct
	i64 fd public
end

open_file public function
	string
	rune
do
	# Last param is top of stack: bind mode first, then path.
	_mode const rune
	_path const string
	# Host file I/O is not wired yet; return a sentinel handle so demos run.
	{fd 0} f const File
	f
	return
end with |File error.Error|

close_file public function
	reference<File>
do
	_file const reference<File>
end
