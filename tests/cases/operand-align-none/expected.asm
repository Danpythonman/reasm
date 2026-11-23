        mov rax, 1
        movzx eax, al
        vpbroadcastd zmm0{k1}{z}, eax

        cmovne rax, rbx
