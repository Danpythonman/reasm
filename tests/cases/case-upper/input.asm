%define value 1
%ifdef value
global main
%endif
section .text
main: lock xadd dword [rax], ecx
mov rax, [rel rip_label]
lea rax, [$rax]
times 2 db 0
foo equ 1
call MyFunc
[bits 64]
