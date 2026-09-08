# Literals, arithmetic, comparisons, and stack words.
# Demonstrates postfix evaluation and drop / dup / swap.
"std.io" io require

main function do
	10 3 +
	10 3 -
	10 3 *
	10 4 /
	10 3 //
	10 3 %
	2 3 ^
	drop
	"hello" " world" ~
	drop
	true false and
	true false or
	true not
	drop
	1 2 ==
	5 3 >
	drop
	1 2 and
	5 2 lshift
	drop
	42 42
	2 1
	2 3 1
	drop
	"stack ok" io.write_line call
end
