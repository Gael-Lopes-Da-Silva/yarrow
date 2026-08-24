# Branch join mismatch: if/else must leave the same number of values.

main function do
	true if
		42
	else
		# ERROR: else leaves nothing while then left i32
	end
	drop
end
