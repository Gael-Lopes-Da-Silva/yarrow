# Learn Yarrow in Y Minutes
# Yarrow is a stack-based language with a rich type system and modular design.
# Let's dive in with examples! Everything is evaluated on a stack.

# Modules: Import with require
"std.io" require io        # Import everything from io into a scope named io
# "std.math.sqrt" require  # Import a function into the main scope
# "std.math" require       # Would import everything from math into main scope

my_function function do
    # Arithmetic Operators: Stack-based, operands popped in reverse order
    1 2 +    # 3 (1 + 2)
    5 3 -    # 2 (5 - 3)
    4 2 *    # 8 (4 * 2)
    10 4 /   # 2.5 (10 / 4)
    10 3 //  # 3 (10 // 3)
    10 3 %   # 1 (10 % 3)
    2 3 ^    # 8 (2 ^ 3)
    # Current stack: [3, 2, 8, 2.5, 3, 1, 8]
    drop

    # Logical Operators: Work with bools
    true false and  # false
    true false or   # true
    true not        # false
    # Current stack: [false, true, false]
    drop

    # Comparison Operators: Return bool
    1 2 ==    # false
    1 2 !=    # true
    5 3 >     # true
    3 5 <     # true
    # Current stack: [false, true, true, true]
    drop

    # Bitwise Operators: For integers
    1 2 and      # 0
    1 5 or       # 5
    4 5 xor      # 1
    5 2 lshift   # 20
    5 2 rshift   # 1
    5 not        # -6
    # Current stack: [0, 5, 1, 20, 1, -6]
    drop

    # Types: Numeric, bool, string, and more
    # Integer literals get the smallest fitting type: positive -> smallest unsigned, negative -> smallest signed
    42        # u8 (smallest fitting integer)
    -900      # i16
    1_000     # u16
    0b100110  # u8 (38)
    0xAB12    # u16 (43890)
    3.14      # f16
    6_329.5   # f16
    "hello"   # string
    '\n'      # rune (char)
    true      # bool
    # Current stack: [42, -900, 1_000, 0b100110, 0xAB12, 3.14, 6_329.5, "hello", '\n', true]

    42 typeof # Pops the value and pushes its static type. Simple values are consumed freely; heap values arrive as borrows (from variable access or dup), which typeof releases, leaving the data owned. Reports the pointee type for references.
    # Current stack: [42, -900, 1_000, 0b100110, 0xAB12, 3.14, 6_329.5, "hello", '\n', true, u8]

    # Stack Manipulation: Control the stack
    drop         # [42, -900, 1_000, 0b100110, 0xAB12, 3.14, 6_329.5, "hello", '\n', true, u8] -> [] Remove all values on the stack and release all borrows
    42 dup       # [42] -> [42, 42] Copies for simple types, borrows for complex types
    1 2 swap     # [1, 2] -> [2, 1]
    1 2 3 rot    # [1, 2, 3] -> [2, 3, 1]
    1 2 3 unrot  # [1, 2, 3] -> [3, 1, 2]
    42 pop       # [42] -> [] Removes 42, also works with references, releasing the borrow

    # Container literals: Array, list, hashmap
    ()    # Empty list
    []    # Empty array
    {}    # Empty hashmap
    # Empty container literals carry no element type, so they need a typed context
    # (such as a variable declaration) to be usable

    drop

    # Variables: Mutable, const, or static
    42 myVar mutable i32       # Mutable, owns the value
    23 myVar set               # Update to 23, drops old value
    100 myConst const i32      # Runtime constant, owns the value
    50 myStatic static i32     # Compile-time constant, owned by program
    # A variable declaration pops a value of the same type from the stack and stores it under its name, out of the stack
    # Calling a variable pushes its value onto the stack (a copy for simple types, a borrow for complex types)
    myVar
    # Current stack: [23]
    myVar typeof
    # Current stack: [23, i32]
    drop

    # Functions: Defined with parameters and return types, can also be defined inside other functions, but can only be called in the body of said function
    # Parameters are copied onto the local stack in declaration order (first declared = deepest).
    # They are bound by `name const Type` declarations in the body, which pop from the top, so the last parameter binds first.
    add function
        i32
        i32 # The two values are copied into the local stack
    do
        # Current stack: [<i32>, <i32>]
        + # We add the two values from the stack
        # Current stack: [<i32>]
        return # Return the top value of the stack
    end with i32 # Return an i32 value

    3 4
    # Current stack: [3, 4]
    add call
    # Current stack: [7]
    drop
    # Current stack: []

    # Control Flow: If/else and match
    # For simplicity, there is no else-if/elif, check match for that
    5 10 < if # If accepts a boolean
        "less" io.write_line call
    else
        "not less" io.write_line call
    end

    # Also works with types
    myVar typeof i32 == if
        "is i32" io.write_line call
    else
        "is not i32" io.write_line call
    end

    85 score const i32
    score match
        # Current stack: [85]
        # Match runs the first case whose condition is true, otherwise the else block
        dup 85 == case # Cases accept a boolean
            "exact match" io.write_line call
        end

        dup 50 < case
            "under 50" io.write_line call
        end

        else
            "did not match everything above" io.write_line call
        end
    end

    # Loops: Conditional or iterable
    0 counter mutable i32
    counter 5 < for # Like a while loop
        counter dup 1 + set # Increment
        break       # Exit early
        # Continue  # Skips to the next iteration
    end

    # Array: Contain a declared number of values
    [10 20 30] numbers static array<i32 3> # If the size is not specified, it is inferred from the literal
    0 sum mutable i32

    numbers value for # Iterate through iterable data structures
        sum dup value + set
    end

    numbers value index for # To get the index
        sum dup index + set
    end

    # List: Dynamic arrays
    (43 54 65) myList static list<i32>

    # Hashmap: List with chosen keys
    # {k v} with literal keys is a hashmap literal; {field value} with identifier keys is a struct literal
    {"first" 4 "second" 5} myHashmap static hashmap<string i32>
end # Return void if not specified

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
    end with i32
end

struct_function function do
    {x 5 y 20} point mutable Point
    10 point.x set
    # Here we need to pass a reference, see memory management below
    point borrow         # Pushes reference<Point>
    point.distance call  # 500 (10^2 + 20^2)
    # Current stack: [500]
end

# Enums: Named values
Color enum
    RED    # 0
    GREEN  # 1
    BLUE   # 2
    # PURPLE 32 # An explicit value: the next member would continue from 33
    # YELLOW 0b101101 # Also works
end

enum_function function do
    Color.RED myColor const Color
    myColor match
        dup Color.RED == case
            "the color is red" io.write_line call
        end

        dup Color.GREEN == case
            "the color is green" io.write_line call
        end

        else
            "the color is not matched" io.write_line call
        end
    end
end

# Unions: Hold one type at a time
MyUnion union
    i32
    string
end

union_function function do
    42 val mutable MyUnion  # Can be initialized either with a string or an i32
    "Myself" val set        # String is a valid type of the union

    val typeof
    # Current stack: [MyUnion]
    drop

    # On a union subject, match dispatches on the active member's type: cases are `Type case`.
    # Each branch receives the member as a reference<Type> that auto-derefs on read, so it
    # behaves like a plain value for read operations (arithmetic, comparison, concatenation...).
    # The borrow is released at the end of the match, leaving the union untouched.
    # Member types must be distinct, and a case type must be one of them.
    val match
        i32 case
            # Current stack: [reference<i32>]
            dup * # Auto-deref: 42 * 42, the reference reads as its value
            # Current stack: [1764]
            drop
        end

        string case
            # Current stack: [reference<string>]
            greeting const reference<string> # Bind the member as a reference
            greeting " says hello!" +        # Auto-deref: concatenation reads through it
            # Current stack: ["Myself says hello!"]
            drop
        end

        else # Optional if all types are covered
        end
    end
end

# Defer: Run at scope exit
defer_function function do
    "std.fs" require fs # Import only for this function scope

    "myfile.txt" 'r' fs.open_file call unwrap # Will return a reference to the file
    file const reference<File>
    # file.read_line call # To get a list of lines
    defer # Defer body is executed in reverse
        # Would be last to execute
        file fs.close_file call
        # Would be first to execute
    end
end

# Memory Management: Stack-based ownership and regions
# Yarrow manages memory using stack ownership, explicit variable ownership,
# borrowing, region-based heap management, and compile-time checks.
memory_function function do
    "std.list" require list      # Import functions like list_push
    "std.region" require region  # Import functions like make_region, put_region or free_region

    # Stack Ownership: Stack owns temporary values, dropped when popped
    "temp"  # Pushes string, owned by stack
    pop     # Drops string, freeing memory

    # Variable Ownership: Variables own values, dropped at scope exit
    "hello" myStr mutable string
    "world" myStr set # Drops "hello", assigns "world"
    # myStr dropped at scope exit

    # Borrowing: Create safe references with borrow operator
    # There can only be one borrow of a value but it can move
    (1 2 3) myList mutable list<i32>
    myList borrow # Pushes reference<list<i32>>
    # Use the reference<list<i32>>
    pop # Ends borrow by popping the reference from the stack
    myList 4 list.list_push call unwrap # Allowed after release
    () myList2 const list<i32>
    myList myList2 move        # Transfer the ownership of the data from myList to myList2
    # myList 4 list_push call  # Compile time error because does not own the value anymore
    myList2 4 list.list_push call unwrap # Allowed after move

    # Compile-Time Checks: Prevent use-after-pop, use-after-free
    # myList2 borrow
    # myList2 pop # Error: Cannot pop while borrowed, need to pop reference before to release borrow

    # Regions: Heap data allocated in regions, freed as a unit
    myRegion region.make_region call
    (1 2 3) myListRegion mutable list<i32>
    myListRegion myRegion region.put_region call
    myRegion region.free_region call # Would also work in a defer
    # Region freed, dropping myListRegion
end

# Error Handling: Errors as values with unwrap and handle
error_function function do
    risky_operation function do
        error.CustomError # create new error value
        return
    end with i32 or error

    # risky_operation call unwrap # Pushes the i32 on success; on error, propagates error.CustomError (returned from this function since it can return an error). In a function that cannot error, unwrap would crash the program instead.
    # io.write_line call
    risky_operation call handle
        match
            error.CustomError == case
                "Caught CustomError" io.write_line call
            end

            else
                "Unknown error" io.write_line call
            end
        end

        "An error happened" fallback # Fallback value to push on the stack, should risky_operation return an error
    end
    io.write_line call

    risky_operation call handle "An error happened" fallback end # If error, push string on stack instead
    io.write_line call
end with void or error

# Example Program: Putting it together
Person struct
    string name
    list<i32> scores
end

Person implement
    add_score function
        reference<Person> # The reference needs to point to a mutable value
        i32
    do
        "std.list" require list # Also works in a struct implementation function

        # Current stack: [reference<Person>, <i32>]
        score const i32
        self const reference<Person>

        self.scores score list.list_push call unwrap
        return
    end with void or error

    greet function
        reference<Person>
    do
        self const reference<Person>

        self.name " says hello!" +
        return
    end with string or error
end

example_function function do
    "std.region" require region

    myRegion region.make_region call
    defer myRegion region.free_region call end

    {name "Alice" scores (10 20)} person mutable Person # Struct literal (identifier keys), not to be confused with hashmap literals {k v}
    person myRegion region.put_region call

    person borrow             # Put a reference<Person> on the stack
    person.greet call unwrap  # Use the reference<Person> to call greet and release borrow
    io.write_line call        # Prints "Alice says hello!"

    person borrow
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

    [12 27 36] _ index for # Only get the index, discard the value
        index 30 < if
            "Younger" io.write_line call
        end
    end
    # Region freed, dropping person
end

# Entry point of the program, always required
main function do
    my_function call # Use the call keyword to call a function
    struct_function call
    enum_function call
    union_function call
    defer_function call
    memory_function call
    error_function call
    example_function call

    "Hello, Yarrow!" io.write_line call # Here's how to write a line
end # May or may not return something

# That's Yarrow in a nutshell! Stack-based, typed, modular, and memory-safe.
