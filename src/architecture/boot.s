.section .text._start
.global _start
_start:
    # Get start and end of BSS
	adrp x1, __bss_start
    add x1, x1, #:lo12:__bss_start
	adrp x2, __bss_end
    add x2, x2, #:lo12:__bss_end

    # Zero BSS
clear_bss:
	stp xzr, xzr, [x1], #16
	cmp x1, x2
	b.ne clear_bss

    // Run init sequence
    mov sp, 0x80000
    b init

.global _per_core_init
_per_core_init:
    # Get core ID
    mrs	x0, mpidr_el1
	and	x0, x0, #0b11

    # Pick an appropriate stack pointer
    # Note: the core ID should never be 0
    # since that core runs the main init sequence
    lsl x0, x0, #16
    mov sp, x0
    b per_core_init
