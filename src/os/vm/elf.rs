//! ELF loading capabilities

use super::AddressSpace;
use bitfield_struct::bitfield;
use core::cmp::Ordering;
use core::mem;
use core::ptr::NonNull;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

/// ELF's register width
#[derive(Debug, FromPrimitive)]
enum BitVersion {
    Bit32 = 1,
    Bit64 = 2,
}

/// ELF endiannes
#[derive(Debug, FromPrimitive)]
enum Endian {
    Little = 1,
    Big = 2,
}

/// ELF ABI
#[derive(Debug, FromPrimitive)]
enum Abi {
    SystemV = 0,
}

/// Type of ELF file
#[derive(Debug, FromPrimitive)]
enum ObjectFile {
    Unknown = 0,
    Relocatable = 1,
    Executable = 2,
    Shared = 3,
    Core = 4,
}

/// ELF ISA
#[derive(Debug, FromPrimitive)]
enum Isa {
    AArch64 = 0xB7,
}

/// Type of program header
#[derive(Debug, FromPrimitive)]
enum ProgramHeaderType {
    Load = 1,
    Phdr = 6,
    GNUStack = 0x6474_E551,
}

/// The complete 64-bit ELF header
#[repr(C)]
struct ElfHeader {
    /// Magic header; should be equal to the `MAGIC` constant in a valid ELF
    magic: [u8; 4],
    /// Register width
    bit_version: u8,
    /// Endianness
    endian: u8,
    /// Version of the header - should be 1
    header_version: u8,
    /// ABI
    abi: u8,
    /// Extra information describing the ABI, if necessary
    abi_version_extra: u8,
    /// Padding
    __: [u8; 7],
    /// Type of ELF file
    obj_file: u16,
    /// ISA
    isa: u16,
    /// Version of ELF used
    elf_version: u32,
    /// Entry point of the executable
    entry: u64,
    /// Offset of the program headers from the start of the ELF
    program_header_offset: u64,
    /// Offset of the section headers from the start of the ELF
    section_header_offset: u64,
    flags: u32,
    /// Size of this header, in bytes
    elf_header_size: u16,
    /// Size of program headers, in bytes
    program_header_entry_size: u16,
    /// Number of program headers
    program_header_entry_count: u16,
    /// Size of section headers, in bytes
    section_header_entry_size: u16,
    /// Number of section headers
    section_header_entry_count: u16,
    section_header_names_index: u16,
}

impl ElfHeader {
    const MAGIC: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46];
}

#[bitfield(u32)]
struct ProgramHeaderFlags {
    executable: bool,
    writeable: bool,
    readable: bool,
    #[bits(29)]
    __: u32,
}

/// ELF Program Headers, 64 bit version
#[repr(C)]
struct ProgramHeader {
    /// Type of program header
    p_type: u32,
    /// Flags associated with this section
    flags: ProgramHeaderFlags,
    /// Offset of this section, in bytes, in the ELF
    offset: u64,
    /// Virtual address to map the section to
    va: u64,
    /// Ignored
    _pa: u64,
    /// Size of the section, in bytes, in the ELF
    filesz: u64,
    /// Size of the virtual addressing range, in bytes, of this section. Bytes beyond the size
    /// covered by `filesz` are filled in with zeroes
    memsz: u64,
    /// Alignment of this section
    align: u64,
}

/// Loads the given ELF file into the given address space, and returns the entry point for the ELF.
///
/// Returns `None` if an error occurs while loading the ELF
#[expect(clippy::module_name_repetitions, reason = "Name is not final")]
#[inline]
pub fn load_elf<const PAGE_BITS: u8, const ADDRESS_BITS: u8>(
    address_space: &mut AddressSpace<PAGE_BITS, ADDRESS_BITS>,
    elf: &[u8],
    elf_pa: u64,
) -> Option<(u64, u64, u64)>
where
    [(); 1 << (ADDRESS_BITS - PAGE_BITS)]: Sized,
{
    let page_mask = (1 << PAGE_BITS) - 1;
    const fn page_round_up(addr: u64, page_size: u8) -> u64 {
        let page_mask = (1 << page_size) - 1;
        (addr + page_mask) & !page_mask
    }

    if elf.len() < mem::size_of::<ElfHeader>() {
        return None;
    }

    // SAFETY: We have verified above that the header has enough space
    let header_ptr = NonNull::from(unsafe { elf.get_unchecked(0) }).cast::<ElfHeader>();
    if !header_ptr.as_ptr().is_aligned() {
        return None;
    }

    // SAFETY: A `ElfHeader` can be represented by any arbitrary bytes of sufficient size,
    // the lifetime is bound by this function which the underlying buffer is guaranteed to last
    // for, and the alignment is verified above
    let header = unsafe { header_ptr.as_ref() };

    // 0x7F followed by ELF
    if header.magic != ElfHeader::MAGIC {
        return None;
    }

    // Program header sizes should match
    if usize::try_from(header.program_header_entry_size).ok()? != mem::size_of::<ProgramHeader>() {
        return None;
    }

    let mut bss_start = None;
    let mut bss_end = None;

    match FromPrimitive::from_u8(header.bit_version)? {
        BitVersion::Bit32 => todo!("Implement 32-bit ELF loading"),
        BitVersion::Bit64 => {
            let offset = usize::try_from(header.program_header_offset).ok()?;
            let num_headers = usize::try_from(header.program_header_entry_count).ok()?;

            if elf.len()
                < mem::size_of::<ProgramHeader>()
                    .checked_mul(num_headers)
                    .and_then(|x| x.checked_add(offset))?
            {
                return None;
            }

            let prog_headers_ptr =
                // SAFETY: We have checked above that there is enough space for the program headers
                NonNull::from(unsafe { elf.get_unchecked(offset) }).cast::<ProgramHeader>();

            if !prog_headers_ptr.as_ptr().is_aligned() {
                return None;
            }

            let prog_headers =
                // SAFETY: we have checked above for sufficient size and proper alignment, program
                // headers can be constructed from arbitrary bits, the memory is not mutated, and the size does not
                // overflow
                unsafe { NonNull::slice_from_raw_parts(prog_headers_ptr, num_headers).as_ref() };

            for header in prog_headers {
                // ELF files are specified to have the same offset from a page in both the file and in
                // memory
                if header.offset & page_mask != header.va & page_mask {
                    return None;
                }

                match FromPrimitive::from_u32(header.p_type)? {
                    ProgramHeaderType::Load => {
                        let virtual_start = header.va & !page_mask;
                        let virtual_backed_range = page_round_up(
                            // SAFETY: From above's masking, `virtual_start <= header.va`
                            unsafe { header.va.unchecked_sub(virtual_start) }
                                .checked_add(header.filesz)?,
                            PAGE_BITS,
                        );
                        match header.filesz.cmp(&header.memsz) {
                            Ordering::Equal | Ordering::Less => {
                                let physical_start =
                                    elf_pa.checked_add(header.offset)? & !page_mask;
                                // SAFETY: The physical and virtual starts are properly aligned by masking
                                unsafe {
                                    address_space.map_range(
                                        virtual_start,
                                        physical_start,
                                        virtual_backed_range,
                                        header.flags.writeable(),
                                        header.flags.executable(),
                                        false,
                                    );
                                }
                                if header.memsz > header.filesz {
                                    assert!(bss_start.is_none());
                                    assert!(bss_end.is_none());
                                    bss_start = Some(header.va + header.filesz);
                                    bss_end = Some(header.va + header.memsz);
                                }
                            }
                            /*Ordering::Less => {
                                let virtual_range = page_round_up(
                                    header.va + header.memsz - virtual_start,
                                    PAGE_BITS,
                                );
                                if virtual_range == virtual_backed_range {
                                    let new_frame = (0x2_0000 as *mut ());
                                    elf.get_mut(
                                        usize::try_from(header.offset + header.filesz).ok()?
                                            ..usize::try_from(header.offset + header.memsz).ok()?,
                                    )?
                                    .fill(0);
                                } else {
                                    todo!("Handle filesz < memsz");
                                }
                            }*/
                            Ordering::Greater => {
                                // Invalid ELF - memsz shouldn't be smaller than filesz
                                return None;
                            }
                        }
                    }
                    ProgramHeaderType::GNUStack | ProgramHeaderType::Phdr => {}
                }
            }

            Some((header.entry, bss_start.unwrap_or(0), bss_end.unwrap_or(0)))
        }
    }
}
