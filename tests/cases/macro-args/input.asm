%macro load 2
        mov %1, %2
%endmacro
%macro say 1-2+
        db %2
%endmacro
%macro label 1
%defstr name %1
        db name
%endmacro
%define pair(a,b) a+b

load    rax ,[ rbx+8 ]
load RAX,qword[rcx+rdx*4]
say 1,  "a" ,  "b"
label   a+b
        mov rax, pair( 1 ,2 )
load    {rdx}, 1
%define str(x) %str(x)
        db str([ RAX+1 ]), 0
        mov rax,[ RBX+1 ]
