# String utilities over host `str_*` helpers.
# `join` takes left, right, then separator (top of stack) and yields left~sep~right.
# `concat` is the two-argument form (same as `~`).
# `compare` returns -1 / 0 / 1 (lexicographic).

len public function
	string
do
	@string_len
end with i64

concat public function
	string
	string
do
	~
end with string

join public function
	string
	string
	string
do
	@string_join
end with string

compare public function
	string
	string
do
	@str_cmp
end with i64
