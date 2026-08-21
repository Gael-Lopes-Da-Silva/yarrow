# unwrap in a function that cannot return an error must not propagate failure.

Boom error
	FAIL
end

failing function do
	Boom.FAIL return
end with |void Boom|

main function do
	# ERROR: main cannot error here; unwrap cannot propagate
	failing call unwrap
end
