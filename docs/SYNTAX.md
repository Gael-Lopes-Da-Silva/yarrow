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
1 ~
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
# you can declare mutable variable like this
myMutableVariable mutable i32 45 end
myMutableVariable set 23 end # change the value with set

# you can define runtime constant like this
myConstant const i32 32 end

# you can define compile time constant like this
myConstant static i32 32 end # need to be a value known at compile time
```

## Functions
Functions are declared like this:
```py
my_function function do
    # ...
end # the function will return a type of void by default

# to return a different type than void you need to do like this
my_function function do
    "a simple string" return # push a string into the stack and return it
end with string # define return value of string

# functions can take parameters
my_params function
    i32 firstParam
    string secondParam
do
    # ...
end

# the main entrypoint of a program is main
main function do
    # ...
end

# To call a function use the call keyword
my_function call
43 "test" my_params call # will consume, on the stack, the exact number of declared parameters
```

## Control Flows
The language possess two common control flow:
```py
# if else statements are done like this
1 4 < if
    # ...
else
    4 5 > if
        # ...
    else
        # ...
    end
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
3 4 > while
    # ...
    break # to break out of the loop
    continue # to continue to the next iteration without doing what's bellow
end

# you can also iterate throught a iterable data structure like arrays
i32 theVal in myArray while
    # will loop until theVal has read all value of myArray
end

# it is also possible to get the index while iterating throught data structures
i32 value i32 index in myArray while
    # ...
end
```

## Data Structures
Here are the data structures you can find in Yarrow:
```py
# structs are declared like this
MyStruct struct
    i32 myVal
    i32 myOtherVal
end

# this is how you can implement a function to a struct
MyStruct implement
    the_function function do
    end
end

# functions declared in a struct are local and can be call like this
instanceOfMyStruct.the_function call

# enumerations are declared like this
myEnum enum
    FIRST # 0
    SECOND # 1
    LAST # 2
end

# unions are declared like this
myUnions union
    i32
    string
end
```

## Code Spliting
Code spliting or modularization is done like this:
```py
# Folders are like namespaces and files are a collections of function, struct, etc.
# Example: std is a folder with files like math.yar, file.yar, etc (yar being the file
# extension of Yarrow). Those files contains functions, struct, etc. like sqrt() or Vector2

"std.utils" require end # import everything from utils into current scope
"std.io" require io end # import everything from io into the io namespace

"path to the custom package" require end

"std.math.sqrt()" require end # import only one function into current scope
"std.math.Vector2" require end # same but with struct

"std.math{sqrt(),Vector2}" require end # import multiple things into current scope
"std.file{read_file(),Reader}" require end # also work with custom namespace
```
