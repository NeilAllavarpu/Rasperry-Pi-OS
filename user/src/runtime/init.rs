use core::{arch, ptr::NonNull};

use alloc::boxed::Box;

use crate::{os::syscalls, println};

/// The entry point of the program.
/// * Reads arguments off the stack and jumps into Rust code.
/// * Specifies the context address for the loading program to correctly invoke this program
#[naked]
#[no_mangle]
pub(super) unsafe extern "C" fn _start() {
    unsafe {
        arch::asm! {
            "adr x0, {ctx}",
            "mov x0, sp",
            "b {start}",
            ctx = sym super::exception::CONTEXT,
            start = sym start,
            options(noreturn)
        }
    }
}

static mut HEAP_MEM: [u8; 1 << 2] = [0; 1 << 2];

/// A wrapper around a pointer to read various values of different sizes in order from the pointer
struct PointerReader(NonNull<()>);

impl PointerReader {
    /// Reads a value from the current pointer, and offsets to the end of the value for further reads.
    /// # Safety
    /// This has the same safety concerns as a raw pointer read, as well as an pointer add - the pointer must always be valid and never go beyond the end of the valid region
    unsafe fn read<T: Copy>(&mut self) -> T {
        let typed_ptr = self.0.cast::<T>();
        // SAFETY: The caller upholds safety guarantees
        let value = unsafe { typed_ptr.read() };
        // SAFETY: The caller upholds safety guarantees
        let next_ptr = unsafe { typed_ptr.add(1) };
        self.0 = next_ptr.cast();
        value
    }

    /// Obtains a reference to a slice from the current pointer, and offsets to the end of the slice for further reads.
    /// # Safety
    /// This has the same safety concerns as `slice_from_raw_parts`, as well as a pointer add - the pointer must always be valid and never go beyond the end of the valid region
    unsafe fn read_slice<'slice, T>(&mut self, count: usize) -> &'slice [T] {
        let typed_ptr = self.0.cast::<T>();
        let value_ptr = NonNull::slice_from_raw_parts(typed_ptr, count);
        // SAFETY: The caller upholds safety guarantees
        let next_ptr = unsafe { typed_ptr.add(count) };
        self.0 = next_ptr.cast();
        // SAFETY: The caller upholds safety guarantees
        unsafe { value_ptr.as_ref() }
    }
}

/// The Rust entry point of the program. Initializes the runtime and then jumps to main
/// # Safety
/// * Should only be called once, upon program load.
/// * The arguments must be correct: `ttbr0_virtual` must be the virtual address of the base table for translation,
/// `argc` must be the number of arguments, `arglens` must be the length of those arguments as an array of `u16`s, and `argbytes` must be a pointer to the packed, concatenated contents of those arguments
/// * `main` must be a C-abi compatible label to invoke, and must be safe
unsafe extern "C" fn start(sp: Option<NonNull<u128>>) -> ! {
    extern "C" {
        fn main();
    }

    let sp = sp.expect("Arguments pointer should not be null");
    assert!(
        sp.is_aligned(),
        "Stack pointer should be aligned to 16 bytes because of stack alignment"
    );

    println!("What just happen? Why here? {:?}", sp);

    unsafe { crate::KERNEL_ALLOCATOR.set(&mut HEAP_MEM) };

    let mut reader = PointerReader(sp.cast());

    // SAFETY: The caller promises that the arguments region is safe
    let ttbr0_virtual = unsafe { reader.read::<usize>() };
    // SAFETY: The caller promises that the arguments region is safe
    let arg_count = unsafe { reader.read::<u16>() }.into();

    // SAFETY: The caller promises that the arguments region is safe
    let arg_lens = unsafe { reader.read_slice::<u16>(arg_count) };

    let args: Box<[&[u8]]> = arg_lens
        .iter()
        .scan(reader, |reader, &length| {
            // SAFETY: The caller promises that the arguments region is safe
            Some(unsafe { reader.read_slice::<u8>(length.into()) })
        })
        .collect();

    println!("ARGUMENTS: {ttbr0_virtual:X} {args:X?}");

    // SET UP VM THINGS
    // SAFETY: The caller/program promises to uphold safety
    unsafe { main() };
    syscalls::exit()
}
