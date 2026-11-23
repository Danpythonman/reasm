        mov eax,[rbx+rcx*4-8]
        mov byte [rax],1
        lea rax,[rel msg]
        mov rax,[fs:0x28]
        mov rax,fs:[rbx]
        db "a,b",'c ; d',`e\`,f`
        dd 1+2,3 * 4
        add rsp,-8
        mov eax,[rbx+-1]
