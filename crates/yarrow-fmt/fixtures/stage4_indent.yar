# Dirty Stage 4 fixture: space indent and misaligned end/else.
# Nested function + if/else should become tab-indented with aligned ends.

"std.io" io require

demo function do
  add function
    i32
    i32 copy
  do
    +
    return
  end with i32

  3 4 add call
  drop
end

main function do
    5 10 < if
        "less" io.write_line call
      else
          "not less" io.write_line call
    end
end
