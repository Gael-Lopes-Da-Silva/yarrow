# Syntax of Yarrow

## Comments
Comments are done using a `#` at the start of the comment:
```py
# this is a comment
```
For simplicity, multiline comments aren't supported.

## Operators
The language has most of the common operators we can find in other languages:
```py
1 1 + # adition
1 1 - # substration
1 1 * # multiplication
1 1 / # division
1 1 // # euclidian division
1 1 % # reminder
1 5 ** # power

true true and
true false or
true not
true false ==
true false !=

1 1 ==
1 5 !=
1 1 >
1 1 >=
1 1 <
1 1 <=

1 1 &
1 5 |
4 5 ^
5 6 <<
6 7 >>
```

For example, a basic math equation in Yarrow look like this:
`(2 + 3) * 11 + 1`
```py
2 3 + 11 * 1 +
```

## Types
Here is the list of the types you can find in Yarrow:
```
u8   i8
u16  i16
u32  i32
u64  i64
u128 i128
f16
f32
f64
f128
bool
void

string
array
vector
hashmap
stack
queue

ptr
usize isize
c_char
c_short
c_ushort
c_int
c_uint
c_long
c_ulong
c_longlong
c_ulonglong
c_double
c_longdouble
```

## Variables
Variable are declared like this:
```py
# by default variable are like runtime constant and cannot be changed after initialization
myVariable i32 32 end # we need to give a value

# you can declare mutable variable like this
mutable myMutableVariable i32 45 end
set myMutableVariable 23 # change the value with set

# you can define compile time constant like this
const myConstant i32 32 end # need to be a value known at compile time
```

## Functions
Functions are declared like this:
```py
function my_function do
    # ...
end # the function will return a type of void by default

# to return a different type than void you need to do like this
function my_function do
    "a simple string" return # push a string into the stack and return it
end with string # define return value of string

# functions can take parameters
function my_params
    i32 firstParam
    string secondParam
do
    # ...
end

# the main entrypoint of a program is main
function main do
    # ...
end
```

## Control Flows
The language possess two common control flow:
```py
# if else statements are done like this
1 4 < if
    # ...
else 4 5 > if
    # ...
else
    # ...
end

# switch case statements are done line this
myMutableVariable match
    43 case
        # ...
    end

    53 54 case
        # ...
    end

    else
        # ...
    end
end
```

## Loops
Loops in Yarrow only use the `while` keyword:
```py
while 3 4 >
    # ...
    break # to break out of the loop
    continue # to continue to the next iteration without doing what's bellow
end

# you can also iterate throught a iterable data structure like arrays
while i32 theVal in myArray
    # will loop until theVal has read all value of myArray
end

# it is also possible to get the index while iterating throught data structures
while i32 value i32 index in myArray
    # ...
end
```

## Data Structures
```py
```
