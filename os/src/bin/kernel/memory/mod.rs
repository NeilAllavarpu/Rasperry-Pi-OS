use alloc::boxed::Box;
use core::sync::atomic::{AtomicU16, Ordering};
use core::{iter, mem};
use stdos::cell::OnceLock;

use crate::println;

pub type ProcessCount = u16;
pub type AtomicProcessCount = AtomicU16;
const PROCESS_COUNT_BITS: u32 = mem::size_of::<ProcessCount>() as u32;
// .expect("Process count size should be a small number of bits");

pub struct PhysicalPage(u64);

impl PhysicalPage {
    unsafe fn new(page: u64) -> Self {
        assert_eq!(page % PAGE_SIZE, 0);
        Self(page)
    }

    pub fn to_owned(self) -> Self {
        PAGE_ALLOCATOR.get().unwrap().to_owned(self)
    }

    // const UNALLOCATED: Self = Self(AtomicU32::new(0));

    // fn fetch_update(
    //     &self,
    //     mut f: impl FnMut(ProcessCount, ProcessCount) -> Option<(ProcessCount, ProcessCount)>,
    // ) -> bool {
    //     self.0
    //         .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
    //             let readers = (value & ((1 << PROCESS_COUNT_BITS) - 1))
    //                 .try_into()
    //                 .expect("Top 16 bits should be the writer value");

    //             let writers = value
    //                 .checked_shr(PROCESS_COUNT_BITS)
    //                 .and_then(|value| value.try_into().ok())
    //                 .expect("Top 16 bits should be the writer value");

    //             f(readers, writers).map(|(readers, writers)| {
    //                 u32::from(writers)
    //                     .checked_shl(PROCESS_COUNT_BITS)
    //                     .expect("Top 16 bits should be the writer value")
    //                     | u32::from(readers)
    //             })
    //         })
    //         .is_ok()
    // }
}

impl Clone for PhysicalPage {
    fn clone(&self) -> Self {
        PAGE_ALLOCATOR.get().unwrap().add_ref(self.0);
        Self(self.0)
    }
}

impl Drop for PhysicalPage {
    fn drop(&mut self) {
        unsafe { PAGE_ALLOCATOR.get().unwrap().remove_ref(self.0) }
    }
}

// impl From<u32> for PhysicalPage {
//     fn from(value: u32) -> Self {
//         Self {
//             writers: value
//                 .checked_shr(PROCESS_COUNT_BITS)
//                 .and_then(|value| value.try_into().ok())
//                 .expect("Top 16 bits should be the writer value"),

//             readers: (value & ((1 << PROCESS_COUNT_BITS) - 1))
//                 .try_into()
//                 .expect("Top 16 bits should be the writer value"),
//         }
//     }
// }

// impl From<PhysicalPage> for u32 {
//     fn from(value: PhysicalPage) -> Self {
//         u32::from(value.writers)
//             .checked_shl(PROCESS_COUNT_BITS)
//             .expect("Top 16 bits should be the writer value")
//             | u32::from(value.readers)
//     }
// }

#[derive(Clone)]
pub struct WriteablePage(PhysicalPage);

impl WriteablePage {
    /// Downgrades this page to read-only access.
    pub fn downgrade(self) -> ReadablePage {
        ReadablePage(self.0)
    }

    pub fn addr(&self) -> u64 {
        self.0 .0
    }
}

#[derive(Clone)]
pub struct ReadablePage(PhysicalPage);

struct RegionAllocator {
    start: u64,
    physical_pages: Box<[AtomicU16]>,
}

const PAGE_SIZE: u64 = 1 << 16;

impl RegionAllocator {
    /// Creates a new physical memory allocator wrapping the given region
    #[expect(clippy::unwrap_in_result)]
    unsafe fn new(
        start: u64,
        size: u64,
        reserved: impl Iterator<Item = &(u64, u64)>,
    ) -> Option<Self> {
        if size % PAGE_SIZE != 0 || start % PAGE_SIZE != 0 || start.checked_add(size).is_none() {
            None
        } else {
            let num_pages = usize::try_from(size / PAGE_SIZE)
                .expect("Number of physical pages should fit into a `usize`");

            let region_allocator = Self {
                start,
                physical_pages: iter::repeat_with(|| AtomicU16::new(0))
                    .take(num_pages)
                    .collect(),
            };
            for &(start, size) in reserved {
                println!("start {start} size {size}");
                for page in (start
                    ..start
                        .checked_add(size)
                        .expect("Regions of physical memory should not overflow"))
                    .step_by(usize::try_from(PAGE_SIZE).unwrap())
                {
                    region_allocator.other_add_ref(page);
                }
            }
            Some(region_allocator)
        }
    }

    /// Increments the reference count
    fn other_add_ref(&self, page: u64) -> bool {
        self.get_page(page)
            .map(|page_refcount| {
                page_refcount
                    .fetch_update(Ordering::Release, Ordering::Acquire, |refcount| {
                        println!("setting {page:X} to 1");
                        assert_eq!(refcount, 0);
                        Some(1)
                    })
                    .expect("Refcount should not overflow")
            })
            .is_some()
    }

    /// Allocates a physical page from this region, if any are available.
    fn alloc(&self) -> Option<PhysicalPage> {
        self.physical_pages
            .iter()
            .enumerate()
            .find_map(|(index, page)| {
                page.fetch_update(Ordering::Acquire, Ordering::Relaxed, |refcount| {
                    (refcount == 0).then_some(1)
                    // (readers == 0).then(|| {
                    //     assert_eq!(
                    //         writers, 0,
                    //         "The number of writers should never exceed the number of readers"
                    //     );
                    //     (1, 1)
                    // })
                })
                .ok()
                .map(|_val| {
                    let paddr = u64::try_from(index)
                        .ok()
                        .and_then(|index| index.checked_mul(PAGE_SIZE))
                        .and_then(|offset| offset.checked_add(self.start))
                        .expect("Physical page should have been verified to be in bounds");
                    unsafe { PhysicalPage::new(paddr) }
                })
                // while let local_word = word.load(Ordering::Relaxed)
                //     && local_word != 0
                // {
                //     let offset = local_word.trailing_zeros();
                //     assert!(offset != usize::BITS);
                //     assert_eq!(local_word & (1 << offset), 1);
                //     if word
                //         .compare_exchange(
                //             local_word,
                //             local_word & !(1 << offset),
                //             Ordering::Acquire,
                //             Ordering::Relaxed,
                //         )
                //         .is_ok()
                //     {
                //         let full_index = index as u64 * u64::from(usize::BITS) + u64::from(offset);
                //         return unsafe {
                //             Some(PhysicalPage::new(self.start + full_index * PAGE_SIZE))
                //         };
                //     }
                // }
                // None
            })
    }

    /// Gets a reference to the refcount of a given physical page
    ///
    /// Returns `None` if the page is not in use by this allocator
    fn get_page(&self, page: u64) -> Option<&AtomicU16> {
        page.checked_sub(self.start).and_then(|offset| {
            assert_eq!(offset % PAGE_SIZE, 0, "Pages should be page aligned");
            let index = usize::try_from(offset / PAGE_SIZE)
                .expect("Physical page numbers should fit into a `usize`");
            // let index = full_index / usize::BITS as usize;
            // let offset = full_index % usize::BITS as usize;
            self.physical_pages.get(index)
        })
    }

    /// Increments the reference count
    fn add_ref(&self, page: u64) -> bool {
        self.get_page(page)
            .map(|page| {
                page.fetch_update(Ordering::Release, Ordering::Acquire, |refcount| {
                    assert_ne!(refcount, 0, "Page should have already been allocated");
                    refcount.checked_add(1)
                })
                .expect("Refcount should not overflow")
            })
            .is_some()
    }

    fn to_owned(&self, page: PhysicalPage) -> Result<PhysicalPage, PhysicalPage> {
        if let Some(refcount) = self.get_page(page.0) {
            match refcount.load(Ordering::Acquire) {
                0 => unreachable!("Refcount of an in-use page should never be zero"),
                1 => Ok(page),
                2.. => {
                    let new_page = self.alloc().unwrap();
                    todo!("Implement memcpy across physical pages");
                    Ok(new_page)
                }
            }
        } else {
            Err(page)
        }
        // .ok_or(page)
    }

    /// Decrements the reference count a physical page with this region, freeing it if there are no other accessers.
    /// Returns false if the physical page is not in range of this region
    ///
    /// # Safety
    ///
    /// If the page is in range of this region, the page to deallocate must have been from an allocation or refcount increment from this region.
    /// The page is invalid to read after being deallocated, using this source.
    unsafe fn remove_ref(&self, page: u64) -> bool {
        self.get_page(page)
            .map(|page| {
                page.fetch_update(Ordering::Release, Ordering::Relaxed, |refcount| {
                    refcount.checked_sub(1)
                })
                .expect("Refcount should not overflow")
                // let final_readers = readers
                //     .checked_sub(1)
                //     .expect("Number of readers should have been at least one");
                // assert_eq!(
                //     readers, 1,
                //     "Must only deallocate a page when the last person is yielding it"
                // );
                // assert!(
                //     writers <= 1,
                //     "The number of writers should never exceed the number of readers"
                // );
                // Some((0, 0))
            })
            .is_some()
    }
}

/// An allocator for physical memory pages
pub struct PageAllocator {
    /// The individual contiguous regions of memory that can be allocated from
    regions: Box<[RegionAllocator]>,
}

impl PageAllocator {
    /// Creates a new page allocator from some list of ranges
    ///
    /// # Safety
    ///
    /// The regions must be nonoverlapping, and reserved for memory allocations via this allocator solely.
    /// It may not be accessed through any other means.
    unsafe fn new(
        ranges: impl Iterator<Item = &(u64, u64)>,
        reserved: &(impl Iterator<Item = &(u64, u64)> + Clone),
    ) -> Option<Self> {
        ranges
            .into_iter()
            .map(|&(start, size)| unsafe { RegionAllocator::new(start, size, reserved.clone()) })
            .try_collect()
            .map(|regions| Self { regions })
    }

    /// Allocates an available page, if any are available
    pub fn alloc(&self) -> Option<WriteablePage> {
        self.regions
            .iter()
            .find_map(RegionAllocator::alloc)
            .map(WriteablePage)
    }

    /// Increments the refcount for a given page
    fn add_ref(&self, page: u64) {
        self.regions
            .iter()
            .find(|region| region.add_ref(page))
            .expect("Physical page should have been allocated prior from some region");
    }

    fn to_owned(&self, mut page: PhysicalPage) -> PhysicalPage {
        for region in self.regions.iter() {
            match region.to_owned(page) {
                Ok(new_page) => return new_page,
                Err(og_page) => page = og_page,
            }
        }
        unreachable!("Physical page should have been allocated prior from some region")
    }

    /// Decreases the refcount of a physical page
    ///
    /// # Safety
    ///
    /// The page to deref must have been derived from this allocator. The page is invalid to use after being derefed.
    unsafe fn remove_ref(&self, page: u64) {
        self.regions
            .iter()
            .find(|region|
                // SAFETY: The caller promises that the page was properly received from this allocator, so there is a single allocator for which this is in range, and for that allocator, the allocation should have been valid
                unsafe { region.remove_ref(page) })
            .expect("Physical page should have been allocated prior from some region");
    }
}

/// The global page allocator for all of physical memory
pub static PAGE_ALLOCATOR: OnceLock<PageAllocator> = OnceLock::new();

/// Initializes the memory allocator using the given memory regions
///
///
/// # Safety
///
/// The regions must be nonoverlapping, and reserved for memory allocations via this allocator solely.
/// It may not be accessed through any other means.
pub unsafe fn init(
    ranges: impl Iterator<Item = &(u64, u64)>,
    reserved: &(impl Iterator<Item = &(u64, u64)> + Clone),
) {
    PAGE_ALLOCATOR.set(unsafe { PageAllocator::new(ranges, reserved) }.unwrap());
}
