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
    } : segment_code
    .rodata : ALIGN(GRANULE_SIZE) {
        __text_end = .;
        __rodata_start = .;
        *(.rodata*)
    } : segment_rodata
    .data : ALIGN(GRANULE_SIZE) {
        __rodata_end = .;
        __data_start = .;
        *(.data*)
    } : segment_data
    .bss (NOLOAD) : ALIGN(16) {
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end = .;
    } : segment_data
    __init_stacks = .;
    . += __init_stack_size * __num_cores;
    . = ALIGN(GRANULE_SIZE * 2);
    __heap_start = .;
    __heap_size = 0x20000;
    . += __heap_size;
    __data_end = .;


    ASSERT(SIZEOF(.bss) > 0, "BSS should not be empty")

    .got : { *(.got*) }
    ASSERT(SIZEOF(.got) == 0, "Relocation support not expected")
}
