GRANULE_SIZE = 64K;

/* Starting PC of the boot sequence */
__link_addr = 0xFFFFFFFFFE000000;

ENTRY(_start)
SECTIONS {
    .text __link_addr : {
        *(.text._start)
        *(.text* )
    } : segment_code
    .rodata : {
        *(.rodata*)
    } : segment_rodata
    .data : {
        *(.data*)
    } : segment_data
    .bss : {
        *(.bss*);
    } : segment_data
}

/* Start + end of sections */
__text_start = ADDR(.text);
__text_end = __text_start + SIZEOF(.text);

__rodata_start = ADDR(.rodata);
__rodata_end = __rodata_start + SIZEOF(.rodata);

__data_start = ADDR(.data);
__data_end = __data_start + SIZEOF(.data);

__bss_start = ADDR(.bss);
__bss_end = __bss_start + SIZEOF(.bss);
