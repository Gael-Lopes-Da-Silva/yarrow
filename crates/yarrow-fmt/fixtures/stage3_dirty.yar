# Dirty Stage 3 fixture: CRLF, trailing spaces/tabs, no final newline.  
# Minimal program: print a line and exit.  

"std.io" io require	

main function do  
	"Hello, Yarrow!" io.write_line call	
end  