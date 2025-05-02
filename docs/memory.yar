# Memory Management in Yarrow: A Task Manager Example
# This program demonstrates the proposed memory management system for Yarrow,
# including stack ownership, explicit variable ownership, borrowing, region-based
# heap management, and compile-time checks.

"std.io" require io

# Error type for task operations
Error enum
    InvalidTag
    TaskFull
end

# Task struct: Represents a task with a name and dynamic list of tags
Task struct
    string name
    list[string] tags
end

# Task methods
Task implement
    add_tag function
        string tag
    do
        this.tags length 5 >= if
            error.TaskFull return
        end
        this.tags push tag call
        return
    end with void or Error

    print function do
        this.name " has tags: " +
        this.tags io.write_line call
        return
    end with void
end

# Helper function to create a task in a region
create_task function
    string name
    list[string] tags
do
    task {name=name tags=tags} mutable Task
    task move # Transfer ownership to caller
    return
end with Task or Error

# Main program demonstrating all memory management components
main function do
    # --- 1. Stack Ownership ---
    # Concept: Values on the stack are owned by the stack. Popping or scope exit drops them.
    # Implementation: Simple types (i32, bool) are copied; complex types (string, list) are moved.
    "temp" # Pushes string, owned by stack
    dup    # Creates a copy (for strings, this is a borrow, see Borrowing)
    io.write_line call # Prints "temp"
    pop    # Drops the borrowed copy
    pop    # Drops the original string

    # --- 2. Explicit Ownership for Variables ---
    # Concept: Variables are explicit owners, dropped when out of scope.
    # Implementation: mutable/const/static variables own values; reassigning drops old value.
    myName "Alice" mutable string
    myName "Bob" set # Drops "Alice", assigns "Bob"
    myName io.write_line call # Prints "Bob"
    # myName dropped at scope exit

    # --- 3. Borrowing via Stack Operations ---
    # Concept: Borrow values to create safe references, tracked by compiler.
    # Implementation: borrow operator creates &T or &mut T; dup on complex types borrows.
    myList (1 2 3) mutable list[i32]
    myList borrow # Pushes &list[i32]
    io.write_line call # Prints list
    release # Ends borrow
    myList push 4 call # Allowed because borrow ended
    myList io.write_line call # Prints [1, 2, 3, 4]

    # --- 4. Region-Based Heap Management ---
    # Concept: Heap data allocated in regions, freed as a unit.
    # Implementation: region/free_region for explicit regions; scope-based regions implicit.
    myRegion region
    defer myRegion free_region call # Ensure region is freed

    # Create a task in myRegion
    "Task1" ("urgent" "work") create_task call unwrap # Pushes Task or propagates Error
    task1 set # task1 owns the Task in myRegion

    # Add a tag
    "priority" task1.add_tag call handle
        match
            error.TaskFull case
                "Task full!" io.write_line call
            end
            else
                "Unknown error" io.write_line call
            end
        end
        # No fallback needed for void return
    end

    # Borrow and print task
    task1 borrow
    print call # Prints "Task1 has tags: [urgent, work, priority]"
    release

    # Move task to another variable (transfer ownership)
    task1 move task2 set # task1 no longer owns the Task
    task2 print call # Prints same as above
    # Region freed by defer, dropping task2’s Task

    # --- 5. Compile-Time Checks ---
    # Concept: Compiler ensures memory safety (no use-after-pop, no use-after-free).
    # Implementation: Tracks ownership, borrows, and lifetimes.
    # Example of invalid code (would be caught by compiler):
    # myList borrow
    # myList pop # Error: Cannot pop while borrowed
    # release
end
