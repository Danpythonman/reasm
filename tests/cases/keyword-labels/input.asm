do:	jmp dq+2
	dw do, add, sub, dq
add:	jmp add-2
sub:	jmp do+2
dq:	dw $-sub
