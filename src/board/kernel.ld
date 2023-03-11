GRANULE_SIZE = 64K;

/* Starting PC of the boot sequence */
__boot_addr = 0x80000;
__link_addr = 0xFFFFFFFFFE000000;
__init_stack_size = 0x1000;
__num_cores = 4;
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
        *(.data*)
        __data_end = .;
    } : segment_data
    .bss (NOLOAD) : ALIGN(16) {
        __bss_start = .;
        *(.bss*);
        __bss_end = .;
        . = ALIGN(GRANULE_SIZE * 2);
        __heap_start = .;
        __heap_size = 0x20000;
        . += __heap_size;

    } : segment_data
}
