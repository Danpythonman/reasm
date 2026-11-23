    {evex}vpmulhrsw xm17, xm23, [cq+64*15]
    {vex} vpaddd ymm0, ymm1, ymm2
    lock   add [rax], 1
obmc_masks:
pw_512:         times 2 dw 512
