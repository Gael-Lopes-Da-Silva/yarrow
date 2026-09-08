# Console output. Thin wrappers over host `print_*` helpers.
write public function
	string
do
	@print
end

write_line public function
	string
do
	@print
	@print_newline
end

write_int public function
	i64
do
	@print_int
end

write_float public function
	f64
do
	@print_float
end

newline public function do
	@print_newline
end
