ENTRY(_start)
/* STM32F407: 1 MiB flash at 0x08000000, 128 KiB contiguous SRAM at 0x20000000. */
MEMORY { FLASH (rx) : ORIGIN = 0x08000000, LENGTH = 1024K
         RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 128K }
SECTIONS {
    .vectors : { KEEP(*(.vectors)) } > FLASH
    .text : { *(.text*) *(.rodata*) } > FLASH
    .ARM.exidx : { *(.ARM.exidx*) } > FLASH
    .data : { __data_start = .; *(.data*) __data_end = .; } > RAM AT> FLASH
    __data_load = LOADADDR(.data);
    .bss (NOLOAD) : { __bss_start = .; *(.bss*) *(COMMON) . = ALIGN(4); __bss_end = .; } > RAM
    __stack_bottom = ORIGIN(RAM) + 96K;
    __stack_top = ORIGIN(RAM) + LENGTH(RAM);
    ASSERT(__bss_end <= __stack_bottom, "Static RAM overlaps stack")
    /DISCARD/ : { *(.comment*) *(.eh_frame*) }
}
