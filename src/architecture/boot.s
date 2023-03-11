.section .text._start
.global _start
_start:
    # Disable interrupts
    msr DAIFSET, #0b1111

    # Zero BSS
    adr x0, __bss_start
    adr x1, __bss_end
1:
	stp xzr, xzr, [x0], #0x10
	cmp x0, x1
	b.lt 1b

    # Set up the kernel translation table
    adr x8, KERNEL_TABLE

    # Map the kernel text as read + execute
    adr x0, __text_start
    adr x1, __text_end
    mov x2, #0x0683 // Disable write permissions
    movk x2, #0x0040, LSL #48 // Disable unprivileged execute
    bl map_region

    # Map the kernel rodata as read only
    adr x0, __rodata_start
    adr x1, __rodata_end
    movk x2, #0x0060, LSL #48 // Disable any execution
    bl map_region

    # Map the kernel data + bss as read-write
    adr x0, __data_start
    adr x1, __bss_end
    movk x2, #0x0603 // Allow read-write permissions, disable execution
    bl map_region

    # Wake up other cores
    adr x0, 1f
    mov x1, #0xE0
    stp x0, x0, [x1, #0]
    str x0, [x1, #0x10]
    dsb ISHST // Ensure that writes are completed before continuing
    sev
1:
    # Disable interrupts
    msr DAIFSET, #0b1111

    # X8 stores the offset to mask to get to the virtual address space
    movn x8, #0xFFFF
    movk x8, #0xFE00, LSL #16

    # Get core ID
    mrs	x0, MPIDR_EL1
	and	x0, x0, #0b11

    # Pick an appropriate stack pointer
    adr x1, INIT_STACKS
    add x0, x0, #1
    add x0, x1, x0, LSL #12 // Index appropriately into the stack array
    orr x0, x0, x8 // Convert to higher address
    msr SP_EL1, x0

    # Disable hypervisor controls
    movz x0, #0x0120, LSL #48
    movk x0, #0x8380, LSL #32
    movk x0, #0xA000, LSL #16
    msr HCR_EL2, x0

    # NOTE: Check EVNTIS bit, if intended to use
    mov x0, #0x00003
    msr CNTHCTL_EL2, x0 // Clear timer hypervisor controls
    msr CNTVOFF_EL2, xzr // Zero virtual offset

    # Jump into the Rust `init` upon `eret`
    adr x0, init
    orr x0, x0, x8
    msr ELR_EL2, x0

    # Keep interrupts disabled in EL1, switch to the SP_EL1 stack
    mov x0, #0x3C5
    msr SPSR_EL2, x0

    # Set EL1's translation table + properties
    adr x0, KERNEL_TABLE
    msr TTBR1_EL1, x0

    mov	w10, #0xff
    msr	MAIR_EL1, x10

    ldr x9, =0x011801D1F527F527
    msr TCR_EL1, x9
    mov	x9, #0x1005
    msr SCTLR_EL1, x9

    # Clear the FP, LR to indicate end of call stack
    mov fp, #0
    mov LR, #0

    # Run init sequence
    # This is a context synchronizing event, so no explicit barrier is necessary
    eret

# Maps a given region with the specified permissions
# x0: Start of region, aligned to a page boundary
# x1: Exclusive end of region
# x2: Permissions mask
# x8: Base address of the kernel table
map_region:
    orr x3, x0, x2 // Apply permissions
    lsr x4, x0, #16 // Convert address to a page number
    str x3, [x8, x4, LSL #3] // Store to appropriate entry

    add x0, x0, #0x10000 // Go to next page
    cmp x0, x1 // Loop if there are more pages to map
    b.lt map_region

    ret
