GRANULE_SIZE = 64K;

/* Starting PC of the boot sequence */
__boot_addr = 0x80000;
__link_addr = 0xFFFFFFFFFE000000;
SECTIONS {
    . =  __link_addr + __boot_addr;
    .text : {
        __text_start = .;
        KEEP(*(.text._start))
        *(.text*)
        __text_end = .;
    } : segment_code
    .rodata : ALIGN(GRANULE_SIZE) {
        __rodata_start = .;
        *(.rodata*)
        __rodata_end = .;
    } : segment_rodata
    .data : ALIGN(GRANULE_SIZE) {
        __data_start = .;
        /* TODO: Figure out a safe way to dynamically reference this physical address */
        ASSERT(. - __link_addr == 0xA0000, "Translation table is shifted");
        KEEP(*(.data.table))
        . += 4096;
        *(.data*)
    } : segment_data
    .bss (NOLOAD) : ALIGN(16) {
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end = .;
    } : segment_data
    __data_end = .;
    . = ALIGN(GRANULE_SIZE * 2);
    __heap_start = .;
    __heap_size = 0x20000;
    . += __heap_size;

    ASSERT(SIZEOF(.bss) > 0, "BSS should not be empty")

    .got : { *(.got*) }
    ASSERT(SIZEOF(.got) == 0, "Relocation support not expected")
}
