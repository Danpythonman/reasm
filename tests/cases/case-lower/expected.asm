%define VALUE 1
        global MAIN
section .TEXT
MAIN:
        mov rax, qword [rbx]
        db 'ABC'
%macro PUSHALL 0
        push rax
%endmacro
