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

# Types: Numeric, bool, string, and more
42        # u8 (smallest fitting integer)
-900      # i16
3.14      # f64 (default float)
"hello"   # string
true      # bool

# Variables: Mutable, const, or static
myVar mutable i32 42 end       # Mutable
myVar set 23 end               # Update to 23
myConst const i32 100 end      # Runtime constant
myStatic static i32 50 end     # Compile-time constant

# Functions: Defined with parameters and return types
add function i32 a i32 b do
    a b + return
end with i32

main function do
    10 20 add call  # Calls add(10, 20) -> 30
end

# Control Flow: If/else and match
5 10 < if
    "less"
else
    "not less"
end

score mutable i32 85 end
score match
    dup 100 <= case
        "A"
    end
    else
        "B or below"
    end
end

# Loops: Conditional or iterable
counter mutable i32 0 end
counter 5 < while
    counter set counter 1 + end
    break  # Exit early
end

numbers array.i32 [10 20 30] end
sum mutable i32 0 end
i32 value in numbers while
    sum set sum value + end
end  # sum = 60

# Structs: Composite types with methods
Point struct
    i32 x
    i32 y
end

Point implement
    distance function do
        x x * y y * + return
    end
end

point Point mutable x: 10 y: 20 end
point.distance call  # 500 (10^2 + 20^2)

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
val Value mutable i32 42 end
val set string "hello" end

# Modules: Import with require
"std.math.sqrt" require end
16 sqrt call  # 4.0

"std.io" require io end
io.println "Hello, Yarrow!" call

# Stack Manipulation: Control the stack
42 dup    # [42, 42]
1 2 swap  # [2, 1]
1 2 3 rot # [2, 3, 1]
42 drop   # Remove 42

# Defer: Run at scope exit
file mutable ptr 0 end
file set open_file call end
defer file close_file call end

# Error Handling: Basic try/catch
try
    risky_operation call
catch
    "Error occurred"
end

# Example Program: Putting it together
Person struct
    string name
    i32 age
end

Person implement
    greet function do
        name " says hello!" + return
    end
end

main function do
    person Person mutable name: "Alice" age: 30 end
    person.greet call  # "Alice says hello!"
    i32 i in array i32 [1 2 3] end while
        i person.age < if
            "Younger"
        end
    end
end

# That's Yarrow in a nutshell! Stack-based, typed, and modular.
