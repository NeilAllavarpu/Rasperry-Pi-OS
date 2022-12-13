.section .text._start
.global _start
_start:
    # Disable interrupts
    msr DAIFSET, #0b1111

    # Get start and end of BSS
	adrp x0, __bss_start
    add x0, x0, #:lo12:__bss_start
	adrp x1, __bss_end
    add x1, x1, #:lo12:__bss_end

1:  # Zero BSS
	stp xzr, xzr, [x0], #16
	cmp x0, x1
	b.ne 1b

    # For logging purposes, make it appear as if the thread ID is 0
    mov x0, #0
    str xzr, [x0]

    # Run init sequence
    mov sp, 0x80000
    b el2_init

.global _per_core_init
_per_core_init:
    # Disable interrupts
    msr DAIFSET, #0b1111

    # Get core ID
    mrs	x0, MPIDR_EL1
	and	x0, x0, #0b11

    # Pick an appropriate stack pointer
    # Note: the core ID should never be 0
    # since that core runs the main init sequence
    lsl x0, x0, #16
    mov sp, x0

    # Run init sequence
    b el2_init
