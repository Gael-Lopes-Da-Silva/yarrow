# List operations for `list<i32>`.
push function
    list<i32>
    i32
do
    @list_push
end

len function
    list<i32>
do
    @list_len
end with i64

get function
    list<i32>
    i64
do
    @list_get
end with i32

put function
    list<i32>
    i64
    i32
do
    @list_set
end
