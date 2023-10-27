/* Starting PC of the boot sequence */
__text_start = 0x4000;

SECTIONS {
    .text __text_start : {
        *(.init)
        *(.text*)
    } : segment_code
    .data : {
      *(.rodata*)
      *(.data*)
      __data_end = .;
    } : segment_data
}

ASSERT(SIZEOF(.bss) == 0, "BSS not implemented");