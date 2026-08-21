# List operations for `list<i32>`.
push_last public function
	list<i32>
	i32
do
	@list_push
end

len public function
	list<i32>
do
	@list_len
end with i64

get public function
	list<i32>
	i64
do
	@list_get
end with i32

put public function
	list<i32>
	i64
	i32
do
	@list_set
end
