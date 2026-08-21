# Map operations for `hashmap<i64 i32>`. `get` pushes the value and a found
# flag (like the `@map_get` builtin).
len public function
	hashmap<i64 i32>
do
	@map_len
end with i64

get public function
	hashmap<i64 i32>
	i64
do
	@map_get
end with i32 bool

put public function
	hashmap<i64 i32>
	i64
	i32
do
	@map_set
end
