// Creates an exception handler that calls the given handler
.macro EXCEPTION_HANDLER handler
    // Save all registers that are not necessarily preserved by the C ABI
    // Calling the handler will preserve the remaining registers
    stp    x0, x1, [sp, #-0xA0]! // This also allocates stack space for the context
    stp    x2, x3, [sp, #0x10]
    stp    x4, x5, [sp, #0x20]
    stp    x6, x7, [sp, #0x30]
    stp    x8, x9, [sp, #0x40]
    stp    x10, x11, [sp, #0x50]
    stp    x12, x13, [sp, #0x60]
    stp    x14, x15, [sp, #0x70]
    stp    x16, x17, [sp, #0x80]
    stp    x18, lr, [sp, #0x90]

    bl    \handler

    // Upon end of handler, return from the exception
    // NOTE: the exception vector is limited to 32 instructions per handler, so
    // this must be very short to fit in

    // Restore everything in reverse order that it was saved
    ldp    x18, lr, [sp, #0x90]
    ldp    x16, x17, [sp, #0x80]
    ldp    x14, x15, [sp, #0x70]
    ldp    x12, x13, [sp, #0x60]
    ldp    x10, x11, [sp, #0x50]
    ldp    x8, x9, [sp, #0x40]
    ldp    x6, x7, [sp, #0x30]
    ldp    x4, x5, [sp, #0x20]
    ldp    x2, x3, [sp, #0x10]
    ldp    x0, x1, [sp], #0xA0 // This also restores the stack pointer
    eret
.endm

.section .text
// Alignment for VBAR
.balign 0x800
.global _exception_vector
_exception_vector:
// The first 4 are taken if we use SP_EL0 at the current exception level, which we should never use
.balign 0x80
    b {from_sp_el0}
.balign 0x80
    b {from_sp_el0}
.balign 0x80
    b {from_sp_el0}
.balign 0x80
    b {from_sp_el0}
// These are taken if we use SP_EL1 and are in EL1
.balign 0x80
    EXCEPTION_HANDLER {synchronous}
.balign 0x80
    EXCEPTION_HANDLER {irq} // IRQs taken while in EL1
.balign 0x80
    b {fiq} // FIQs should never be enabled for any peripheral
.balign 0x80
    b {serror} // Can SErrors occur?
// These are taken if we were in AArch64 EL0
.balign 0x80 // Synchronous exception: only an SVC should be able to return, and it should have already formatted the registers accordingly
    // Save all registers that are not necessarily preserved by the C ABI
    // Calling the handler will preserve the remaining registers
    stp    x18, lr, [sp, #-0xA0]! // This also allocates stack space for the context
    mrs x18, ESR_EL1
    lsr w18, w18, 26
    cmp w18, {SVC_CODE}
    b.ne 0f
    bl {svc}
    b 1f
    0: stp    x2, x3, [sp, #0x10]
    stp    x4, x5, [sp, #0x20]
    stp    x6, x7, [sp, #0x30]
    stp    x8, x9, [sp, #0x40]
    stp    x10, x11, [sp, #0x50]
    stp    x12, x13, [sp, #0x60]
    stp    x14, x15, [sp, #0x70]
    stp    x16, x17, [sp, #0x80]
    stp    x0, x1, [sp, #0x90]

    bl    {synchronous}

    // Upon end of handler, return from the exception
    // NOTE: the exception vector is limited to 32 instructions per handler, so
    // this must be very short to fit in

    // Restore everything in reverse order that it was saved
    ldp    x0, x1, [sp, #0x90]
    ldp    x16, x17, [sp, #0x80]
    ldp    x14, x15, [sp, #0x70]
    ldp    x12, x13, [sp, #0x60]
    ldp    x10, x11, [sp, #0x50]
    ldp    x8, x9, [sp, #0x40]
    ldp    x6, x7, [sp, #0x30]
    ldp    x4, x5, [sp, #0x20]
    ldp    x2, x3, [sp, #0x10]
    1:ldp    x18, lr, [sp], #0xA0 // This also restores the stack pointer
    eret
.balign 0x80
    EXCEPTION_HANDLER {irq} // IRQs taken while in EL0
.balign 0x80
    b {fiq} // FIQs should never be enabled for any peripheral
.balign 0x80
    b {serror} // Can SErrors occur?
// These are taken if we were in AArch32 EL0 - currently not supported
.balign 0x80
    b {aarch32}
.balign 0x80
    b {aarch32}
.balign 0x80
    b {aarch32}
.balign 0x80
    b {aarch32}
