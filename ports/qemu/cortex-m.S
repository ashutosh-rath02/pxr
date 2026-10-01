.syntax unified
.cpu cortex-m3
.thumb
.section .vectors,"a"
.word __stack_top
.word _start
.rept 13
.word trap
.endr
.word timer_interrupt
.section .text._start,"ax"
.global _start
.thumb_func
_start:
    ldr r0, =__bss_start
    ldr r1, =__bss_end
    movs r2, #0
1:  cmp r0, r1
    bhs 2f
    str r2, [r0], #4
    b 1b
2:  ldr r0, =__data_start
    ldr r1, =__data_end
    ldr r2, =__data_load
3:  cmp r0, r1
    bhs 4f
    ldr r3, [r2], #4
    str r3, [r0], #4
    b 3b
4:  ldr r0, =__stack_bottom
    ldr r1, =__stack_top
    ldr r2, =0xa5a5a5a5
5:  cmp r0, r1
    bhs 6f
    str r2, [r0], #4
    b 5b
6:  bl test_main
    b trap
.thumb_func
trap:
    movs r0, #1
    bl platform_exit
    b trap
