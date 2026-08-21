# Local module imported by valid/12_modules.yar as "helpers.greet".

hello public function
	string
do
	"std.io" io require
	name const string
	"Hello, " name ~ "!" ~
	io.write_line call
end
