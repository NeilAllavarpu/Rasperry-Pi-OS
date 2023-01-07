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
    movk x0, #0x8380, LSL #32
    movk x0, #0xA000, LSL #16
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

    # Set EL1's translation table + propertires
    # TODO: Don't hardcode this
    mov x0, #0xA0000
    msr TTBR0_EL1, x0

    mov	w10, #0xf0
    msr	MAIR_EL1, x10

    mov x9, #0x7527
    movk x9, #0x191, lsl #32
    movk x9, #0x118, lsl #48
    msr TCR_EL1, x9
    mov	x9, #0x1005
    msr SCTLR_EL1, x9

    # Run init sequence
    # This is a context synchronizing event, so no explicit barrier is necessary
    eret
.section .data.table
.balign 4096, 4096
.global KERNEL_TABLE
KERNEL_TABLE:
.dword 0x00603
.dword 0x10603
.dword 0x20603
.dword 0x30603
.dword 0x40603
.dword 0x50603
.dword 0x60603
.dword 0x70603
.dword 0x80603
.dword 0x90603
.dword 0xA0603
.dword 0xB0603
.dword 0xC0603
.dword 0xD0603
.dword 0xE0603
.dword 0xF0603
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
.dword 0
