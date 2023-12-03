/* Starting PC of the boot sequence */
__link_addr = 0x1000;
__text_start = __link_addr;

SECTIONS {
    .text __link_addr : {
        *(.init)
        *(.text*)
    } : segment_code
    .data : {
      *(.rodata*)
      *(.data*)
    } : segment_data
    .bss (NOLOAD) : {
      *(.bss*)
    } : segment_data
    ASSERT((SIZEOF(.eh_frame) == 0), "Should not have unwinding information")
}

__bss_start = ADDR(.bss);
__bss_end = __bss_start + SIZEOF(.bss);
__elf_start = ADDR(.data) + SIZEOF(.data);
