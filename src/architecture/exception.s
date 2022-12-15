// Creates an exception handler that calls the given handler
.macro EXCEPTION_HANDLER handler
	// Allocate stack space for the context
	sub	sp, sp, #0x110

	// Save all general purpose registers
	stp	x0, x1, [sp, #0x0]
	stp	x2, x3, [sp, #0x10]
	stp	x4, x5, [sp, #0x20]
	stp	x6, x7, [sp, #0x30]
	stp	x8, x9, [sp, #0x40]
	stp	x10, x11, [sp, #0x50]
	stp	x12, x13, [sp, #0x60]
	stp	x14, x15, [sp, #0x70]
	stp	x16, x17, [sp, #0x80]
	stp	x18, x19, [sp, #0x90]
	stp	x20, x21, [sp, #0xA0]
	stp	x22, x23, [sp, #0xB0]
	stp	x24, x25, [sp, #0xC0]
	stp	x26, x27, [sp, #0xD0]
	stp	x28, x29, [sp, #0xE0]

	str	lr, [sp, #0xF0]

	bl	\handler

	// Upon end of handler, return from the exception
    // Because the exception vector is limited to 32 instructions per handler,
    // this must be a branch outside of the exception vector
    // (Currently, this uses 24 instructions)
	b	exception_return
.endm
.section .text
// Alignment for VBAR
.balign 0x800
.global _exception_vector
_exception_vector:
.balign 0x80
    EXCEPTION_HANDLER handle_curr_el0_sync
.balign 0x80
	EXCEPTION_HANDLER handle_curr_el0_irq
.balign 0x80
    EXCEPTION_HANDLER handle_curr_el0_fiq
.balign 0x80
	EXCEPTION_HANDLER handle_curr_el0_other
.balign 0x80
	EXCEPTION_HANDLER handle_curr_elx_sync
.balign 0x80
	EXCEPTION_HANDLER handle_curr_elx_irq
.balign 0x80
	EXCEPTION_HANDLER handle_curr_elx_fiq
.balign 0x80
	EXCEPTION_HANDLER handle_curr_elx_other
.balign 0x80
	EXCEPTION_HANDLER handle_lower_el_sync_64
.balign 0x80
	EXCEPTION_HANDLER handle_lower_el_irq_64
.balign 0x80
	EXCEPTION_HANDLER handle_lower_el_fiq_64
.balign 0x80
	EXCEPTION_HANDLER handle_lower_el_other_64
.balign 0x80
	EXCEPTION_HANDLER handle_lower_el_sync_32
.balign 0x80
	EXCEPTION_HANDLER handle_lower_el_irq_32
.balign 0x80
	EXCEPTION_HANDLER handle_lower_el_fiq_32
.balign 0x80
	EXCEPTION_HANDLER handle_lower_el_other_32
exception_return:
    // Restore everything in reverse order that it was saved
	ldr	lr, [sp, #0xF0]

	ldp	x28, x29, [sp, #0xE0]
	ldp	x26, x27, [sp, #0xD0]
	ldp	x24, x25, [sp, #0xC0]
	ldp	x22, x23, [sp, #0xB0]
	ldp	x20, x21, [sp, #0xA0]
	ldp	x18, x19, [sp, #0x90]
	ldp	x16, x17, [sp, #0x80]
	ldp	x14, x15, [sp, #0x70]
	ldp	x12, x13, [sp, #0x60]
	ldp	x10, x11, [sp, #0x50]
	ldp	x8, x9, [sp, #0x40]
	ldp	x6, x7, [sp, #0x30]
	ldp	x4, x5, [sp, #0x20]
	ldp	x2, x3, [sp, #0x10]
	ldp	x0, x1, [sp, #0x0]

	add	sp, sp, #0x110
	eret
