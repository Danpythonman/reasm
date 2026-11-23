%macro PRINT 1
            mov rsi, %1                ; arg
    %%done:
    %ifdef DEBUG
              call debug
    %else
              nop
    %endif
%endmacro
%rep 4
          nop
%endrep
        struc point
    .x:
            resd 1
    .y:
            resd 1
        endstruc
