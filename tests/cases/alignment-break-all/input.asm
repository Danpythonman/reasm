mov rax, 1 ; a
; standalone
movzx eax, al ; b
global x ; c
ret ; d
%if 1
nop ; e
%endif
x:
dec ecx ; f
