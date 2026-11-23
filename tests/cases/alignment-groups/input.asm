foo:
    mov rax, 1 ; a
.x: movzx eax, al ; b
    cmovne rax, [rbx+rcx*8+some_long_offset_name] ; c

    nop ; d
    vpxor xmm0, xmm0 ; e
