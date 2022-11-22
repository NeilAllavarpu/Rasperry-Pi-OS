/* Starting PC of the boot sequence */
__boot_addr = 0x80000;

SECTIONS {
    . =  __boot_addr;

    .text : {
        KEEP(*(.text._start))
        *(.text*)
    } : segment_code
    .rodata : ALIGN(8) { *(.rodata*) } : segment_code
    .data : { *(.data*) } : segment_data

    /* Align BSS + get start and end for zeroing purposes */
    .bss (NOLOAD) : ALIGN(16) {
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end = .;
    } : segment_data
    ASSERT(SIZEOF(.bss) > 0, "BSS should not be empty")


    .got : { *(.got*) }
    ASSERT(SIZEOF(.got) == 0, "Relocation support not expected")
}
