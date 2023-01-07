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
.global _per_core_init
_per_core_init:
    # Disable interrupts
    msr DAIFSET, #0b1111

    # Get core ID
    mrs	x0, MPIDR_EL1
	and	x0, x0, #0b11

    # Pick an appropriate stack pointer
    add x0, x0, #1
    lsl x0, x0, #16
    msr SP_EL1, x0

    # Disable hypervisor controls
    movz x0, #0x0120, LSL #48
    movz x1, #0x8380, LSL #32
    movz x2, #0xA000, LSL #16
    add x0, x0, x1
    add x0, x0, x2
    msr HCR_EL2, x0

    # NOTE: Check EVNTIS bit, if intended to use
    mov x0, #0x00003
    msr CNTHCTL_EL2, x0

    msr CNTVOFF_EL2, xzr

    adrp x0, init
    add x0, x0, #:lo12:init
    msr ELR_EL2, x0

    # Keep interrupts disabled in EL1, switch to the SP_EL1 stack
    mov x0, #0x3C5
    msr SPSR_EL2, x0

    # Run init sequence
    eret
