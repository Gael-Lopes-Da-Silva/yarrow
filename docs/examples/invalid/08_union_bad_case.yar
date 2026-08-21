# Union match case type must be a member of the union.

Value union
	i32
	string
end

main function do
	42 v mutable Value
	v match
		# ERROR: bool is not a member of Value
		bool case
		end

		else
		end
	end
end
