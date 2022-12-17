// Creates an exception handler that calls the given handler
.macro EXCEPTION_HANDLER handler
	// Save all registers that are not necessarily preserved by the C ABI
    // Calling the handler will preserve the remaining registers
	stp	x0, x1, [sp, #-0xA0]! // This also allocates stack space for the context
	stp	x2, x3, [sp, #0x10]
	stp	x4, x5, [sp, #0x20]
	stp	x6, x7, [sp, #0x30]
	stp	x8, x9, [sp, #0x40]
	stp	x10, x11, [sp, #0x50]
	stp	x12, x13, [sp, #0x60]
	stp	x14, x15, [sp, #0x70]
	stp	x16, x17, [sp, #0x80]
	stp	x18, lr, [sp, #0x90]

	bl	\handler

	// Upon end of handler, return from the exception
    // NOTE: the exception vector is limited to 32 instructions per handler, so
    // this must be very short to fit in
    // (Currently, this uses 22 instructions in total)

	// Restore everything in reverse order that it was saved
	ldp	x18, lr, [sp, #0x90]
	ldp	x16, x17, [sp, #0x80]
	ldp	x14, x15, [sp, #0x70]
	ldp	x12, x13, [sp, #0x60]
	ldp	x10, x11, [sp, #0x50]
	ldp	x8, x9, [sp, #0x40]
	ldp	x6, x7, [sp, #0x30]
	ldp	x4, x5, [sp, #0x20]
	ldp	x2, x3, [sp, #0x10]
	ldp	x0, x1, [sp], #0xA0 // This also restores the stack pointer
	eret
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
