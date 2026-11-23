do:
        JMP dq+2
        DW do, add, sub, dq
add:
        JMP add-2
sub:
        JMP do+2
dq:
        DW $-sub
