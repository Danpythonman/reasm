%define SYS_WRITE 1
        global _start

section .text
; Starting point
_start:
        mov rax, 1                     ; write(fd, buf, len)
        mov rdi, 1
.loop:
        dec rcx
        jnz .loop

section .data
msg:
        db "Hello world!", 10
msglen  equ $-msg
