
    .section .init
    .globl _start
_start:
    la  sp, __stack_top

    /* Zero .bss */
    la  a0, __bss_start__
    la  a1, __bss_end__
1:
    beq a0, a1, 2f
    sw  zero, 0(a0)
    addi a0, a0, 4
    blt a0, a1, 1b
2:
    /* Jump to C main */
    call main

3:
    wfi
    j 3b
