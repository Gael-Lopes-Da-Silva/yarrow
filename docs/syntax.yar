# Learn Yarrow in Y Minutes
# Yarrow is a stack-based language with a rich type system and modular design.
# Let's dive in with examples! Everything is evaluated on a stack.

# Comments: Single-line only with #
# This is a comment
42 # Can follow code

# Arithmetic Operators: Stack-based, operands popped in reverse order
1 2 +    # 3 (1 + 2)
5 3 -    # 2 (5 - 3)
4 2 *    # 8 (4 * 2)
10 4 /   # 2.5 (10 / 4)
10 3 //  # 3 (Euclidean division)
10 3 %   # 1 (remainder)
2 3 **   # 8 (2^3)

# Logical Operators: Work with bools
true false and  # false
true false or   # true
true not        # false

# Comparison Operators: Return bool
1 2 ==    # false
1 2 !=    # true
5 3 >     # true
3 5 <     # true

# Bitwise Operators: For integers
1 2 &     # 0 (1 & 2)
1 5 |     # 5 (1 | 5)
5 2 <<    # 20 (5 << 2)
5 2 >>    # 1 (5 >> 2)
5 ~       # -6 (~5)

# Types: Numeric, bool, string, and more
42        # u8 (smallest fitting integer)
-900      # i16
3.14      # f16
"hello"   # string
true      # bool

# Variables: Mutable, const, or static
myVar 42 mutable i32       # Mutable
myVar 23 set               # Update to 23
myConst 100 const i32      # Runtime constant
myStatic 50 static i32     # Compile-time constant

# Functions: Defined with parameters and return types
add function
    i32 a
    i32 b
do
    a ?65 # If a is not provided, push 65

    a b +
    return # Put into the main stack what's in the local function stack
end with i32

main function do
    10 20 add call  # Calls add(10, 20) -> 30
    {b=20} add call # Calls add(b: 20) where `a` default to 65 -> 85
end # Return void if not specified

# Control Flow: If/else and match
5 10 < if
    "less"
else
    "not less"
end

score 85 mutable i32
score match
    dup 100 <= case
        "A"
    end
    dup 85 == case
        "exact match"
    end
    dup 30 > dup 90 < and case
        "range match"
    end
    else
        "B or below"
    end
end

# Loops: Conditional or iterable
counter 0 mutable i32
counter 5 < while
    counter dup 1 + set
    break # Exit early
end

numbers [10 20 30] static array[i32]
sum 0 mutable i32
value in numbers while
    sum dup value + set end
end # sum = 60

# Structs: Composite types with methods
Point struct
    i32 x
    i32 y
end

Point implement
    distance function do
        this.x this.x *
        this.y this.y * +
        return
    end
end

point {x=5 y=20} mutable Point
point.x 10 set
point.distance call # 500 (10^2 + 20^2)

# Enums: Named values
Color enum
    RED    # 0
    GREEN  # 1
end

# Unions: Hold one type at a time
Value union
    i32
    string
end

val 42 mutable Value
val "hello" set

# List: dynamique arrays
myList (43 54 65) static list[i32]

# Hashmap: list with choosen keys
myHashmap {"first"=4 "second"=5} static hashmap[string i32]

# Modules: Import with require
"std.math.sqrt" require
16 sqrt call # 4.0

"std.io" require io
yarrow "Yarrow!" static string
"Hello, ${yarrow}" io.write_line call # there is string interpolation

# Stack Manipulation: Control the stack
42 dup    # [42, 42]
1 2 swap  # [2, 1]
1 2 3 rot # [2, 3, 1]
42 pop    # Remove 42
drop      # Remove all the value on the stack

# Defer: Run at scope exit
file 0 mutable pointer[i32]
file open_file call set

defer
    file close_file call
end

# Error Handling: Basic try/catch
risky_operation function do
    error.CustomError return
end with i32 or error # return a value of i32 or an error

main function do
    risky_operation call unwrap # Pushes i32 or propagates Error

    # If no error, i32 is on the stack
    io.write_line call
end

main function do
    # If you want to handle the error locally
    risky_operation call catch # Error is on the stack
        dup error.CustomError == if
            "Caught CustomError" io.write_line call
            0 # Push fallback value
        else
            error.OutOfMemory return # Propagate a different error
        end
    end

    # If no error, i32 is on the stack; if error handled, fallback value (0) is on stack
    io.write_line call
end

# Example Program: Putting it together
"std.io" require io

Person struct
    string name
    i32 age
end

Person implement
    greet function do
        name " says hello!" +
        return
    end
end

main function do
    person {name="Alice" age=30} mutable Person
    person.greet call # "Alice says hello!"
    i32 i in [1 2 3] while
        i person.age < if
            "Younger" io.write_line call
        end
    end
end

# That's Yarrow in a nutshell! Stack-based, typed, and modular.
