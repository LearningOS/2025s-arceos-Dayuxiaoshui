#![no_std]

use core::ptr::NonNull;

use allocator::{AllocError, BaseAllocator, ByteAllocator, PageAllocator};

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const PAGE_SIZE: usize> {
    start: usize,
    b_pos: usize,
    p_pos: usize,
    end: usize,
    count: usize,
}

impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            b_pos: 0,
            p_pos: 0,
            end: 0,
            count: 0,
        }
    }
}

impl<const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = self.end;
        self.count = 0;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> allocator::AllocResult {
        unreachable!()
    }
}

impl<const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    fn alloc(
        &mut self,
        layout: core::alloc::Layout,
    ) -> allocator::AllocResult<core::ptr::NonNull<u8>> {
        let align = layout.align();
        let size = layout.size();

        if size == 0 || !align.is_power_of_two() {
            return Err(allocator::AllocError::InvalidParam);
        }

        let start = (self.b_pos + align - 1) & !(align - 1);
        let end = start
            .checked_add(size)
            .ok_or(allocator::AllocError::NoMemory)?;

        if end > self.p_pos {
            return Err(AllocError::NoMemory);
        }

        self.b_pos = end;
        self.count += 1;

        Ok(unsafe { NonNull::new_unchecked(start as *mut u8) })
    }

    fn dealloc(&mut self, pos: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        self.count -= 1;
        if self.count == 0 {
            self.b_pos = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.p_pos - self.start
    }

    fn used_bytes(&self) -> usize {
        self.b_pos - self.start
    }

    fn available_bytes(&self) -> usize {
        self.p_pos - self.b_pos
    }
}

impl<const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    const PAGE_SIZE: usize = PAGE_SIZE;

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> allocator::AllocResult<usize> {
        let align: usize = 1 << align_pow2;
        let size = num_pages * Self::PAGE_SIZE;

        if align.count_ones() != 1 || align < Self::PAGE_SIZE {
            return Err(AllocError::InvalidParam);
        }

        let end = self.p_pos & !(align - 1);
        if end < size {
            return Err(AllocError::NoMemory);
        }

        let start = end - size;
        if start < self.b_pos {
            return Err(AllocError::NoMemory);
        }
        self.p_pos = start;
        Ok(start)
    }

    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {
        unreachable!()
    }

    fn total_pages(&self) -> usize {
        (self.end - self.start)/ Self::PAGE_SIZE
    }

    fn used_pages(&self) -> usize {
        (self.end - self.p_pos) / Self::PAGE_SIZE
    }

    fn available_pages(&self) -> usize {
        (self.p_pos - self.b_pos) / Self::PAGE_SIZE
    }
}