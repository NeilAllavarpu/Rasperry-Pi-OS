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
    .bss  : {
      *(.bss*)
    } : segment_data
.stack_sizes (INFO) :
  {
    KEEP(*(.stack_sizes));
  }
    ASSERT((SIZEOF(.eh_frame) == 0), "Should not have unwinding information")
}

__bss_start = ADDR(.bss);
__bss_end = __bss_start + SIZEOF(.bss);
