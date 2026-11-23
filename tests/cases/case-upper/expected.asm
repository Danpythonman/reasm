%DEFINE value 1
%IFDEF value
        GLOBAL main
%ENDIF
SECTION .text
main:
        LOCK XADD DWORD [RAX], ECX
        MOV RAX, [REL rip_label]
        LEA RAX, [$rax]
        TIMES 2 DB 0
foo     EQU 1
        CALL MyFunc
        [BITS 64]
