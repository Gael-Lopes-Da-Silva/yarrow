# Yarrow Syntax Documentation

Yarrow is a stack-based programming language designed for flexibility and control, featuring a rich type system, modular code organization, and robust control flow constructs. This document outlines the syntax and semantics of Yarrow, providing examples and explanations for each feature.

## Comments

Comments in Yarrow are single-line and begin with the `#` symbol. They are used to annotate code and are ignored by the interpreter.

```yarrow
# This is a single-line comment
42 # Comments can follow code on the same line
```

**Notes**:
- Multiline comments are not supported to keep the syntax simple.
- Comments can appear anywhere in the source code and extend to the end of the line.

## Operators

Yarrow supports a variety of operators for arithmetic, logical, comparison, and bitwise operations. Operators are stack-based, meaning they consume operands from the stack in reverse order (e.g., `a b +` pops `b` and `a`, then pushes `a + b`).

### Arithmetic Operators
```yarrow
1 2 +   # Addition: 1 + 2 = 3
5 3 -   # Subtraction: 5 - 3 = 2
4 2 *   # Multiplication: 4 * 2 = 8
10 4 /  # Division: 10 / 4 = 2.5
10 3 // # Euclidean division: 10 // 3 = 3
10 3 %  # Remainder: 10 % 3 = 1
2 3 **  # Power: 2^3 = 8
```

### Logical Operators
```yarrow
true true and   # Logical AND: true
true false or   # Logical OR: true
true not        # Logical NOT: false
```

### Comparison Operators
```yarrow
1 1 ==    # Equal: true
1 2 !=    # Not equal: true
5 3 >     # Greater than: true
5 5 >=    # Greater than or equal: true
3 5 <     # Less than: true
3 3 <=    # Less than or equal: true
```

### Bitwise Operators
```yarrow
1 2 &     # Bitwise AND: 1 & 2 = 0
1 5 |     # Bitwise OR: 1 | 5 = 5
4 5 ^     # Bitwise XOR: 4 ^ 5 = 1
5 2 <<    # Left shift: 5 << 2 = 20
6 1 >>    # Right shift: 6 >> 1 = 3
5 ~       # Bitwise NOT: ~5 (depends on type)
```

**Example: Complex Expression**
To evaluate `(2 + 3) * 11 + 1`:
```yarrow
2 3 + 11 * 1 +  # Result: 56
```

**Operator Precedence**:
- Yarrow does not use traditional operator precedence; operations are evaluated strictly based on stack order.
- Parentheses are not needed for grouping since the stack dictates evaluation order.

**Notes**:
- Arithmetic and bitwise operators require numeric operands (e.g., `i32`, `f64`).
- Logical operators expect `bool` operands.
- Comparison operators return a `bool` value.
- Ensure sufficient operands are on the stack to avoid underflow errors.

## Types

Yarrow provides a comprehensive set of types for integers, floating-point numbers, and complex data structures, as well as C-compatible types for interoperability.

### Numeric Types
- **Signed Integers**: `i8`, `i16`, `i32`, `i64`, `i128`
- **Unsigned Integers**: `u8`, `u16`, `u32`, `u64`, `u128`
- **Floating-Point**: `f16`, `f32`, `f64`, `f128`
- **C-Compatible Integers**: `c_char`, `c_short`, `c_ushort`, `c_int`, `c_uint`, `c_long`, `c_ulong`, `c_longlong`, `c_ulonglong`
- **C-Compatible Floating-Point**: `c_double`, `c_longdouble`
- **Size Types**: `usize` (unsigned, architecture-dependent), `isize` (signed, architecture-dependent)

### Other Types
- **Boolean**: `bool` (`true` or `false`)
- **Void**: `void` (no value)
- **String**: `string` (sequence of characters)
- **Collections**: `array` (fixed-size), `vector` (dynamic-size), `hashmap` (key-value), `stack`, `queue`
- **Pointer**: `ptr` (memory address)
- **Type**: `type` (represents a type itself, used in metaprogramming)
- **Error**: `error` (for error handling)

**Example**:
```yarrow
42        # Inferred as u8 (smallest fitting integer type)
-900      # Inferred as i16
3.14      # Float, typically f64
"hello"   # String
true      # Bool
i32       # Type literal
```

**Notes**:
- Integer literals are automatically assigned the smallest fitting type (e.g., `42` as `u8` if within bounds).
- Floating-point literals default to `f64` unless specified.
- Types like `array` and `vector` require additional syntax for initialization (see Data Structures).

## Variables

Variables in Yarrow can be mutable, runtime constants, or compile-time constants. They are declared with a type and initialized with a value or expression.

### Syntax
```yarrow
# Mutable variable
myVar mutable i32 42 end

# Runtime constant
myConst const i32 100 end

# Compile-time constant
myStatic static i32 50 end  # Must be a constant expression
```

### Assigning Values
Use the `set` keyword to update mutable variables:
```yarrow
myVar set 23 end  # Updates myVar to 23
```

**Example: Variable Usage**
```yarrow
counter mutable i32 0 end
counter set counter 1 + end  # Increment counter
```

**Notes**:
- Mutable variables (`mutable`) can be reassigned using `set`.
- Runtime constants (`const`) cannot be modified after initialization.
- Compile-time constants (`static`) must be resolvable at compile time (e.g., literals or constant expressions).
- Variables must be closed with `end`.
- The interpreter checks type bounds during assignment (e.g., `i32` bounds are `-2^31` to `2^31 - 1`).

## Functions

Functions in Yarrow are defined with the `function` keyword and support parameters, return types, and stack-based execution. The main entry point of a program is the `main` function.

### Syntax
```yarrow
# Basic function (returns void)
myFunc function do
    42  # Push 42 onto the local stack of the function
end

# Function with return type
myStringFunc function do
    "world" return  # Return a string to the main stack
end with string

# Function with parameters
add function i32 a i32 b do
    a b + return  # Add parameters and return result
end with i32

# Main entry point
main function do
    10 20 add call  # Calls add(10, 20)
end
```

### Calling Functions
Use the `call` keyword to invoke a function, which consumes the required number of arguments from the stack:
```yarrow
10 20 add call  # Pushes 30 (10 + 20) onto the stack
"hello" myStringFunc call  # Pushes "world"
```

**Example: Factorial Function**
```yarrow
factorial function i32 n do
    n 0 == if
        1 return
    else
        n n 1 - factorial call * return
    end
end with i32

main function do
    5 factorial call  # Computes 5! = 120
end
```

**Notes**:
- Functions default to returning `void` unless a `with` clause specifies a return type.
- The `return` keyword pushes a value onto the stack and exits the function.
- Parameters are declared as pairs of type and name (e.g., `i32 a`).
- Function calls consume arguments in the order they appear on the stack (last pushed is the last parameter).
- Recursive calls are supported, as shown in the factorial example.
- The `main` function is the program entry point and must return `void`.

## Control Flow

Yarrow supports conditional execution with `if`/`else` and pattern matching with `match`. Both constructs are stack-based and must be closed with `end`.

### If/Else Statements
```yarrow
# Simple if
5 10 < if
    "less"  # Executed if true
end

# If with else
x mutable i32 15 end
x 10 < if
    "less than 10"
else
    x 20 < if
        "between 10 and 20"
    else
        "20 or more"
    end
end
```

### Match Statements
The `match` construct evaluates a value against multiple cases, with an optional `else` branch:
```yarrow
status mutable i32 42 end
status match
    42 case
        "success"
    end
    43 case
        "error"
    end
    else
        "unknown"
    end
end
```

**Example: Grading System**
```yarrow
score mutable i32 85 end
score match
    dup 100 <= case
        "A"
    end
    dup 89 <= case
        "B"
    end
    else
        "C or below"
    end
end
```

**Notes**:
- `if` expects a `bool` value on the stack (e.g., from a comparison like `5 10 <`).
- `match` evaluates the top stack value against each case's condition.
- Case conditions can be single values (e.g., `42`) or expressions (e.g., `90 100 <=`).
- Both constructs must end with `end`, even for nested branches.
- An empty `else` branch in `match` is valid but does nothing.

## Loops

Yarrow uses the `while` keyword for loops, supporting both conditional loops and iteration over data structures.

### Conditional Loops
```yarrow
counter mutable i32 0 end
counter 5 < while
    counter set counter 1 + end
    break  # Exit the loop early
    continue  # Skip to the next iteration
end
```

### Iterating Over Data Structures
Iterate through arrays, vectors, or other iterables:
```yarrow
myArray array.i32 [1 2 3] end
i32 value in myArray while
    value  # Process each value
end

# With index
i32 value i32 index in myArray while
    value index +  # Use both value and index
end
```

**Example: Sum of Array**
```yarrow
numbers array.i32 [10 20 30] end
sum mutable i32 0 end
i32 value in numbers while
    sum set sum value + end
end
# sum is 60
```

**Notes**:
- `while` loops expect a `bool` condition on the stack for conditional loops.
- Iteration loops use the `in` keyword to bind values (and optionally indices) to variables.
- `break` exits the loop immediately, while `continue` skips to the next iteration.
- Loops must be closed with `end`.
- Ensure the iterable (e.g., `array`) is properly initialized to avoid runtime errors.

## Data Structures

Yarrow supports structs, enums, unions, and implementations for organizing and manipulating data.

### Structs
Structs define composite types with named fields:
```yarrow
Point struct
    i32 x
    i32 y
end

# Instantiate a struct
point Point mutable x: 10 y: 20 end
```

### Implementations
Add functions to structs for behavior:
```yarrow
Point implement
    distance function do
        x x * y y * +  # Approximate distance (sqrt not applied)
    end
end

# Call struct method
point.distance call  # Computes x^2 + y^2
```

### Enums
Enums define a set of named values with optional explicit values:
```yarrow
Color enum
    RED    # 0
    GREEN  # 1
    BLUE   # 2
end

# With explicit values
Status enum
    OK 200
    ERROR 500
end
```

### Unions
Unions allow a variable to hold one of several types:
```yarrow
Value union
    i32
    string
end

# Use a union
val Value mutable i32 42 end
val set string "hello" end
```

**Example: Struct with Methods**
```yarrow
Rectangle struct
    i32 width
    i32 height
end

Rectangle implement
    area function do
        width height * return
    end
end

rect Rectangle mutable width: 5 height: 10 end
rect.area call  # Returns 50
```

**Notes**:
- Struct fields are declared as type-name pairs.
- Struct methods are called using dot notation (`instance.method call`).
- Enums automatically assign sequential integers starting from 0 unless specified.
- Unions store only one value at a time, with type checking at runtime.
- All data structure declarations must end with `end`.

## Code Splitting and Modularization

Yarrow supports modular code organization through the `require` keyword, allowing imports of functions, structs, and other definitions from files or namespaces.

### Syntax
```yarrow
# Import entire module
"std.utils" require end  # Imports all definitions from std/utils.yar

# Import into a namespace
"std.io" require io end  # Imports std/io.yar into the io namespace

# Import specific definitions
"std.math.sqrt" require end  # Imports sqrt function
"std.math.Vector2" require end  # Imports Vector2 struct

# Import multiple definitions
"std.math{sqrt,Vector2}" require end  # Imports sqrt and Vector2

# Import from custom package
"myapp.utils" require end  # Imports from myapp/utils.yar
```

**Example: Using Standard Library**
```yarrow
"std.math.sqrt" require end
16 sqrt call  # Returns 4.0

"std.io" require io end
io.println "Hello, Yarrow!" call
```

**Notes**:
- Modules are organized in a folder structure (e.g., `std/math.yar`).
- File extensions are typically `.yar` but omitted in `require` statements.
- Namespaces are created using dot notation (e.g., `std.math`).
- Specific imports reduce namespace pollution and improve clarity.
- The `end` keyword is required to close `require` statements.
- Circular imports are not supported and will cause errors.

## Additional Features

### Stack Manipulation
Yarrow provides stack manipulation operations for advanced control:
```yarrow
42 dup   # Duplicates the top value: [42, 42]
1 2 swap # Swaps the top two values: [2, 1]
1 2 3 rot # Rotates the top three values: [2, 3, 1]
42 drop   # Removes the top value
pop       # Removes the top value (synonym for drop)
over      # Copies the second-to-top value to the top
```

### Defer Statements
The `defer` keyword schedules code to run at the end of the current scope:
```yarrow
file mutable ptr 0 end
file set open_file call end
defer file close_file call end  # Ensures file is closed when scope exits
```

### Error Handling
Yarrow supports basic error handling with `try` and `catch`:
```yarrow
try
    risky_operation call
catch
    "Error occurred"  # Handle the error
end
```

**Notes**:
- Stack operations are essential for managing operands in complex expressions.
- `defer` is useful for resource cleanup (e.g., closing files).
- Error handling is currently limited and may be expanded in future versions.

## Example Program

A complete Yarrow program demonstrating multiple features:
```yarrow
# Define a struct
Person struct
    string name
    i32 age
end

# Implement a method
Person implement
    greet function do
        name " says hello!" + return
    end
end

# Main function
main function do
    # Create a person
    person Person mutable name: "Alice" age: 30 end

    # Call method
    person.greet call

    # Loop through ages
    i32 i in array i32 [1, 2, 3] end while
        i person.age < if
            "Younger"
        else
            "Older"
        end
    end
end
```

## Best Practices
- **Use Descriptive Names**: Choose clear names for variables, functions, and structs (e.g., `calculateSum` instead of `f`).
- **Leverage Static Constants**: Use `static` for compile-time constants to optimize performance.
- **Modularize Code**: Organize code into modules using `require` to improve maintainability.
- **Check Stack State**: Ensure the stack has the correct number and type of operands before operations.
- **Close Blocks**: Always use `end` to close constructs like `function`, `if`, and `while`.

## Limitations
- Multiline comments are not supported.
- The interpreter currently implements only a subset of instructions (e.g., arithmetic and stack operations).
- Advanced error handling and debugging tools are under development.
- The `compiler` module is not yet implemented, limiting the language to interpretation.

For further details or to contribute to Yarrow's development, refer to the interpreter source code or contact the maintainers.
