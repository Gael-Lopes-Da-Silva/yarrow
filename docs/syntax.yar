# Learn Yarrow in Y Minutes
# Yarrow is a stack-based language with a rich type system and modular design.
# Let's dive in with examples! Everything is evaluated on a stack.

# Arithmetic Operators: Stack-based, operands popped in reverse order
1 2 +    # 3 (1 + 2)
5 3 -    # 2 (5 - 3)
4 2 *    # 8 (4 * 2)
10 4 /   # 2.5 (10 / 4)
10 3 //  # 3 (10 // 3)
10 3 %   # 1 (10 % 3)
2 3 ^    # 8 (2 ^ 3)

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
1 2 and      # 0
1 5 or       # 5
4 5 xor      # 1
5 2 lshift   # 20
5 2 rshift   # 1
5 not        # -6

# Types: Numeric, bool, string, and more
42        # u8 (smallest fitting integer)
-900      # i16
3.14      # f16
"hello"   # string
'\n'      # rune (char)
true      # bool

# Stack Manipulation: Control the stack
42 dup    # [42] -> [42, 42] For simple types, borrows for complex types
1 2 swap  # [1, 2] -> [2, 1]
1 2 3 rot # [1, 2, 3] -> [2, 3, 1]
42 pop    # [42] -> [] Remove 42 also work with reference, releasing borrows
drop      # [2, 3] -> [] Remove all values on the stack and release all borrows

# Variables: Mutable, const, or static
42 myVar mutable i32       # Mutable, owns the value
23 myVar set               # Update to 23, drops old value
100 myConst const i32      # Runtime constant, owns the value
50 myStatic static i32     # Compile-time constant, owned by program

# Functions: Defined with parameters and return types
add function
    i32
    i32 # The two value are copied into the local stack
do
    + # We add the two value from the stack
    return # Return the top value of the stack
end with i32

main function do
    10 20 add call  # Calls add(10, 20) -> 30
end # Return void if not specified

# Control Flow: If/else and match
# For simplicity, there is no else if/elif, check match for that
5 10 < if
    "less"
else
    "not less"
end

score 85 mutable i32
score match
    dup 100 <= case # Cases accept a boolean
        "A"
    end
    dup 85 == case
        "exact match"
    end
    dup 30 > over 90 < and case
        "range match"
    end
    else
        "B or below"
    end
end

# Loops: Conditional or iterable
0 counter mutable i32
counter 5 < while
    counter dup 1 + set # Incrementation
    break # Exit early
end

[10 20 30] numbers static array<i32 3> # If size not specified, will infer it
0 sum mutable i32

numbers value for # Iterate throught iterable data structures
    sum dup value + set
end

# Structs: Composite types with methods
Point struct
    i32 x
    i32 y
end

Point implement
    distance function
        reference<Point>
    do
        self const reference<Point>
        self.x self.x * self.y self.y * +
        return
    end
end

{x 5 y 20} point mutable Point
10 point.x set
# Here we need to pass a reference, see memory management bellow
point @borrow call point.distance call # 500 (10^2 + 20^2)

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

42 val mutable Value
"hello" val set

# Modules: Import with require
"std.math.sqrt" require
16 sqrt call # 4.0

"std.io" require io
"Hello, Yarrow!" io.write_line call

# List: Dynamic arrays
(43 54 65) myList static list<i32>

# Hashmap: List with chosen keys
{"first" 4 "second" 5} myHashmap static hashmap<string i32>

# Defer: Run at scope exit
"myfile.txt" 'r' open_file call file mutable pointer<i32>
defer file close_file call end # Defer body is executed in reverse

# Memory Management: Stack-based ownership and regions
# Yarrow manages memory using stack ownership, explicit variable ownership,
# borrowing, region-based heap management, and compile-time checks.

# Stack Ownership: Stack owns temporary values, dropped when popped
"temp" # Pushes string, owned by stack
pop    # Drops string, freeing memory

# Variable Ownership: Variables own values, dropped at scope exit
main function do
    "hello" myStr mutable string
    "world" myStr set # Drops "hello", assigns "world"
end # myStr dropped at scope exit

# Borrowing: Create safe references with borrow operator
# There can only be one borrow of a value but it can move
(1 2 3) myList mutable list<i32>
myList @borrow call # Pushes reference<list<i32>>
# Use the reference<list<i32>>
pop # Ends borrow by popping the reference from the stack
myList 4 @list_push call # Allowed after release
0 myList2 const list<i32>
myList myList2 @move call # Transfer the ownership of the data from myList to myList2
myList 4 @list_push call # Compile time error because does not own the value anymore

# Regions: Heap data allocated in regions, freed as a unit
myRegion @make_region
defer myRegion @free_region call end
(1 2 3) myList mutable list<i32>
myList myRegion @put_region call
# Region freed, dropping myList

# Compile-Time Checks: Prevent use-after-pop, use-after-free
myList @borrow call
myList pop # Error: Cannot pop while borrowed, need to pop reference before to release borrow

# Error Handling: Errors as values with unwrap and handle
risky_operation function do
    error.CustomError return # create new error value
end with i32 or error

main function do
    risky_operation call unwrap # Pushes i32 or propagates error.CustomError
    # will crash the program and throw CustomError
    io.write_line call
end

main function do
    risky_operation call handle
        match
            error.CustomError == case
                "Caught CustomError" io.write_line call
            end
            else
                "Unknown error" io.write_line call
            end
        end
        0 # Fallback value
    end
    io.write_line call
end

main function do
    risky_operation call handle 0 end # If error, push 0
    io.write_line call
end

# Example Program: Putting it together
"std.io" require io

Person struct
    string name
    list<i32> scores
end

Person implement
    add_score function
        reference<Person> # The reference need to point to mutable value
        i32
    do
        self const reference<Person>
        score const i32

        self.scores score @list_push call unwrap
        return
    end with void or Error

    greet function
        reference<Person>
    do
        self const reference<Person>

        self.name "says hello!" ' ' @string_join call unwrap
        return
    end with string or Error
end

main function do
    myRegion @make_region
    defer myRegion @free_region call end

    {name "Alice" scores (10 20)} person mutable Person
    person myRegion @put_region call

    person @borrow call # Put a reference<Person> on the stack
    person.greet call unwrap # Use the reference<Person> to call greet and release borrow
    io.write_line call # Prints "Alice says hello!"

    person @borrow call
    30 person.add_score call handle # Consume the reference and the score
        match
            error.OutOfMemory == case
                "No memory" io.write_line call
            end
            else
                "Unknown error" io.write_line call
            end
        end
    end

    [12 27 36] i for
        i 30 < if
            "Younger" io.write_line call
        end
    end
    # Region freed, dropping person2
end

# That's Yarrow in a nutshell! Stack-based, typed, modular, and memory-safe.
