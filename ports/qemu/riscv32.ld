ENTRY(_start)
MEMORY { RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 128K }
SECTIONS {
    .text : { KEEP(*(.init)) *(.text*) *(.rodata*) } > RAM
    .data : { __data_start = .; *(.data*) *(.sdata*) __data_end = .; } > RAM
    __data_load = __data_start;
    .bss (NOLOAD) : { __bss_start = .; *(.bss*) *(.sbss*) *(COMMON) . = ALIGN(4); __bss_end = .; } > RAM
    __stack_bottom = ORIGIN(RAM) + 96K;
    __stack_top = ORIGIN(RAM) + LENGTH(RAM);
    ASSERT(__bss_end <= __stack_bottom, "Program overlaps stack")
    /DISCARD/ : { *(.comment*) *(.eh_frame*) }
}
