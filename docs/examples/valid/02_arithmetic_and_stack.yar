# Literals, arithmetic, comparisons, and stack words.
# Demonstrates postfix evaluation and drop / dup / swap.

"std.io" io require

main function do
	# [left, right] then op → result
	10 3 +
	10 3 -
	10 3 *
	10 4 /
	10 3 //
	10 3 %
	2 3 ^
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

	42 dup
	1 2 swap
	1 2 3 rot
	drop

	"stack ok" io.write_line call
end
