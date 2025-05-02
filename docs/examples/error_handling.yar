"std.io" require io

Error enum
    InvalidAge
    NameTooLong
end

Person struct
    string name
    i32 age
end

Person implement
    greet function do
        this.name dup length 10 > if
            error.NameTooLong return
        end
        this.name " says hello!" +
        return
    end with string | Error
end

main function do
    person {name="Alice" age=30} mutable Person
    person.greet call handle
        match
            error.NameTooLong case
                "Name too long!" io.write_line call
            end
            else
                "Unknown error" io.write_line call
            end
        end
        "Fallback greeting" # Fallback if error
    end
    io.write_line call # Prints greeting or fallback

    i32 i in [1 2 3] while
        i person.age < if
            "Younger" io.write_line call
        end
    end
end
