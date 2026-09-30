.section .init,"ax"
.global _start
.option norelax
_start:
    la sp, __stack_top
    la t0, __bss_start
    la t1, __bss_end
1:  bgeu t0, t1, 2f
    sw zero, 0(t0)
    addi t0, t0, 4
    j 1b
2:  la t0, __stack_bottom
    la t1, __stack_top
    li t2, 0xa5a5a5a5
3:  bgeu t0, t1, 4f
    sw t2, 0(t0)
    addi t0, t0, 4
    j 3b
4:  la t0, trap
    csrw mtvec, t0
    call test_main
    j trap
.balign 4
trap:
    li a0, 1
    call platform_exit
    j trap
