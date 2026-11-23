; reasm: off
mov    rax,1   
; reasm: on
        mov rax, 1
; reasm: skip
mov    rax,   2
%if 0
  this is   not code
%endif
%define LONG 1 + \
     2
db 'unterminated
