.section .text
.global _context_switch
_context_switch:
    # Save context to stack
	stp	fp,  lr,  [sp,#-0x70]!
    mov fp,  sp
	stp	x27, x28, [sp, #0x60]
	stp	x25, x26, [sp, #0x50]
	stp	x23, x24, [sp, #0x40]
	stp	x21, x22, [sp, #0x30]
	stp	x19, x20, [sp, #0x20]

    mrs x4, daif
    stp x18, x4, [sp, #0x10]

    # Save old SP
    mrs x4, TPIDR_EL1
    mov x5, sp
    str x5, [x4, #8]
    # Load new SP
    msr TPIDR_EL1, x2
    ldr x5, [x2, #8]
    mov sp, x5

    # Restore context
    ldp x18, x19, [sp, #0x10]
    msr daif, x19

	ldp	x19, x20, [sp, #0x20]
	ldp	x21, x22, [sp, #0x30]
	ldp	x23, x24, [sp, #0x40]
	ldp	x25, x26, [sp, #0x50]
	ldp	x27, x28, [sp, #0x60]
	ldp	fp,  lr,  [sp],#0x70

    # Pass the previous thread as an argument to the callback
    mov x2, x4
    # UNSAFE: This makes assumptions about the passability of Rust closures
    br x3
