; top
foo:
        mov rax, 1            ; a
        movzx eax, byte [rbx] ; b
        ; inside

        ret         ; c
        mov rax, [a_really_long_symbol_name_that_goes_past_max_column_here] ; d
        nop         ; e
        ; end
