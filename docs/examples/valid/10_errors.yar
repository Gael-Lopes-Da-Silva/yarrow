# Custom error type, fallible return |T Err|, unwrap, and handle + fallback.

"std.io" io require

AppError error
	NOT_FOUND
	BAD_INPUT
end

lookup function
	i32
do
	key const i32
	key 0 == if
		AppError.NOT_FOUND return
	else
		key 10 *
		return
	end
end with |i32 AppError|

main function do
	5 lookup call unwrap
	drop

	0 lookup call handle
		match
			AppError.NOT_FOUND case
				"missing" io.write_line call
			end

			else
				"other error" io.write_line call
			end
		end

		-1 fallback
	end
	drop

	0 lookup call handle 0 fallback end
	drop
end with |void AppError|
