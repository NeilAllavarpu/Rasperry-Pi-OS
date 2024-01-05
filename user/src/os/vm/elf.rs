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

#[derive(Debug)]
pub enum ElfLoadError {
    UnexpectedEoF,
    Alignment,
    Magic,
    HeaderSize,
    BitVersion,
    HeaderType,
    MemSz,
}

/// Loads the given ELF file into the given address space, and returns the entry point for the ELF.
///
/// Returns `None` if an error occurs while loading the ELF
#[expect(clippy::module_name_repetitions, reason = "Name is not final")]
#[inline]
#[allow(panic_in_result_fn)]
#[allow(clippy::too_many_lines)]
pub fn load_elf<const PAGE_BITS: u8, const ADDRESS_BITS: u8>(
    address_space: &mut AddressSpace<PAGE_BITS, ADDRESS_BITS>,
    elf: &[u64],
    elf_pa: u64,
) -> Result<(u64, u64, u64, u64), ElfLoadError>
where
    [(); 1 << (ADDRESS_BITS - PAGE_BITS)]: Sized,
{
    const fn page_round_up(addr: u64, page_size: u8) -> u64 {
        let page_mask = (1 << page_size) - 1;
        (addr + page_mask) & !page_mask
    }

    const fn page_round_down(addr: u64, page_size: u8) -> u64 {
        let page_mask = (1 << page_size) - 1;
        (addr) & !page_mask
    }
    let page_mask = (1 << PAGE_BITS) - 1;
    let elf_len = mem::size_of_val(elf);

    if elf_len < mem::size_of::<ElfHeader>() {
        return Err(ElfLoadError::UnexpectedEoF);
    }

    // SAFETY: We have verified above that the header has enough space
    let header_ptr = NonNull::from(unsafe { elf.get_unchecked(0) }).cast::<ElfHeader>();
    if !header_ptr.as_ptr().is_aligned() {
        return Err(ElfLoadError::Alignment);
    }

    // SAFETY: A `ElfHeader` can be represented by any arbitrary bytes of sufficient size,
    // the lifetime is bound by this function which the underlying buffer is guaranteed to last
    // for, and the alignment is verified above
    let header = unsafe { header_ptr.as_ref() };

    // 0x7F followed by ELF
    if header.magic != ElfHeader::MAGIC {
        return Err(ElfLoadError::Magic);
    }

    // Program header sizes should match
    if usize::from(header.program_header_entry_size) != mem::size_of::<ProgramHeader>() {
        return Err(ElfLoadError::HeaderSize);
    }

    let mut bss_start = None;
    let mut bss_end = None;
    let mut ctx_addr = None;

    match FromPrimitive::from_u8(header.bit_version).ok_or(ElfLoadError::BitVersion)? {
        BitVersion::Bit32 => todo!("Implement 32-bit ELF loading"),
        BitVersion::Bit64 => {
            let offset = usize::try_from(header.program_header_offset)
                .expect("`usize` should fit into `u64`");
            let num_headers = usize::from(header.program_header_entry_count);

            if !mem::size_of::<ProgramHeader>()
                .checked_mul(num_headers)
                .and_then(|x| x.checked_add(offset))
                .is_some_and(|end| end <= elf_len)
            {
                return Err(ElfLoadError::UnexpectedEoF);
            }

            let prog_headers_ptr =
                // SAFETY: We have checked above that there is enough space for the program headers
                NonNull::from(unsafe { elf.get_unchecked(offset / 8) }).cast::<ProgramHeader>();

            if !prog_headers_ptr.as_ptr().is_aligned() {
                return Err(ElfLoadError::Alignment);
            }

            let prog_headers =
                // SAFETY: we have checked above for sufficient size and proper alignment, program
                // headers can be constructed from arbitrary bits, the memory is not mutated, and the size does not
                // overflow
                unsafe { NonNull::slice_from_raw_parts(prog_headers_ptr, num_headers).as_ref() };

            let entry = header.entry;
            for header in prog_headers {
                // ELF files are specified to have the same offset from a page in both the file and in
                // memory
                if header.offset & page_mask != header.va & page_mask {
                    return Err(ElfLoadError::Alignment);
                }

                match FromPrimitive::from_u32(header.p_type).ok_or(ElfLoadError::HeaderType)? {
                    ProgramHeaderType::Load => {
                        let virtual_start = header.va & !page_mask;
                        let virtual_backed_range = page_round_up(
                            header
                                .va
                                .checked_sub(virtual_start)
                                .and_then(|addr| addr.checked_add(header.filesz))
                                .ok_or(ElfLoadError::UnexpectedEoF)?,
                            PAGE_BITS,
                        );
                        if virtual_start <= entry && entry <= virtual_start + virtual_backed_range {
                            assert!(ctx_addr.is_none());
                            let e_as_bytes = unsafe {
                                NonNull::slice_from_raw_parts(
                                    NonNull::from(elf).cast::<u8>(),
                                    elf_len,
                                )
                                .as_ref()
                            };
                            let ctx_off = (page_round_down(header.offset, PAGE_BITS)
                                + (entry - virtual_start))
                                as usize;

                            let val = u32::from_le_bytes(
                                e_as_bytes[ctx_off..ctx_off + 4].try_into().unwrap(),
                            );
                            let off =
                                u64::from((((val >> 5) & 0x7_FFFF) << 2) | ((val >> 29) & 0b11));
                            let new_ctx = (entry + off);
                            // panic!(
                            //     "get {entry:X} {virtual_start:X} {:X} {ctx_off:X} {val:X} {off:X} {new_ctx:X}", header.offset
                            // );
                            // as *mut UserContext;
                            ctx_addr = Some(new_ctx);
                        } else {
                            // panic!("cant do virtual_start")
                        }
                        match header.filesz.cmp(&header.memsz) {
                            Ordering::Equal | Ordering::Less => {
                                let physical_start = elf_pa
                                    .checked_add(header.offset)
                                    .ok_or(ElfLoadError::UnexpectedEoF)?
                                    & !page_mask;
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
                                };
                                if header.memsz > header.filesz {
                                    assert!(bss_start.is_none());
                                    assert!(bss_end.is_none());
                                    bss_start = Some(header.va + header.filesz);
                                    bss_end = Some(header.va + header.memsz);
                                }
                            }
                            Ordering::Greater => {
                                // Invalid ELF - memsz shouldn't be smaller than filesz
                                return Err(ElfLoadError::MemSz);
                            }
                        }
                    }
                    ProgramHeaderType::GNUStack | ProgramHeaderType::Phdr => {}
                }
            }

            Ok((
                header.entry,
                bss_start.unwrap_or(0),
                bss_end.unwrap_or(0),
                ctx_addr.unwrap(),
            ))
        }
    }
}
