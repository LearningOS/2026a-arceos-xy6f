#![no_std]

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;

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
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,
    end: usize,
    b_pos: usize,
    p_pos: usize,
    b_count: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            b_count: 0,
        }
    }

    #[inline]
    fn is_initialized(&self) -> bool {
        self.start != self.end
    }

    #[inline]
    fn align_down(pos: usize, align: usize) -> usize {
        pos & !(align - 1)
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        if self.is_initialized() {
            panic!("EarlyAllocator::init: already initialized");
        }
        if start.checked_add(size).is_none() {
            panic!("EarlyAllocator::init: memory region overflow");
        }
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = self.end;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        if size == 0 {
            return Err(AllocError::InvalidParam);
        }
        if !self.is_initialized() {
            self.init(start, size);
            return Ok(());
        }
        // The bump allocator can only manage one contiguous range, so the
        // new region is accepted only if it extends the current range.
        let Some(new_end) = start.checked_add(size) else {
            return Err(AllocError::InvalidParam);
        };
        if new_end == self.start {
            self.start = start;
            self.b_pos = start;
            Ok(())
        } else if start == self.end {
            self.end = new_end;
            self.p_pos = new_end;
            Ok(())
        } else {
            Err(AllocError::InvalidParam)
        }
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(
        &mut self,
        layout: core::alloc::Layout,
    ) -> AllocResult<core::ptr::NonNull<u8>> {
        let size = layout.size();
        let align = layout.align().max(1);
        if size == 0 {
            return Err(AllocError::InvalidParam);
        }
        let pos = Self::align_down(self.b_pos + (align - 1), align);
        let new_b_pos = pos.checked_add(size).ok_or(AllocError::NoMemory)?;
        if new_b_pos > self.p_pos {
            return Err(AllocError::NoMemory);
        }
        self.b_pos = new_b_pos;
        self.b_count += 1;
        Ok(NonNull::new(pos as *mut u8).unwrap())
    }

    fn dealloc(&mut self, pos: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        debug_assert!(pos.as_ptr() as usize + layout.size() <= self.b_pos);
        self.b_count = self.b_count.saturating_sub(1);
        if self.b_count == 0 {
            // All byte allocations have been freed, reclaim the whole area.
            self.b_pos = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    fn used_bytes(&self) -> usize {
        self.b_pos - self.start
    }

    fn available_bytes(&self) -> usize {
        self.p_pos - self.b_pos
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> AllocResult<usize> {
        let align = 1usize
            .checked_shl(align_pow2 as u32)
            .unwrap_or(0)
            .max(SIZE);
        let aligned_end = Self::align_down(self.p_pos, align);
        let new_p_pos = aligned_end
            .checked_sub(num_pages.checked_mul(SIZE).ok_or(AllocError::NoMemory)?)
            .ok_or(AllocError::NoMemory)?;
        if new_p_pos < self.b_pos {
            return Err(AllocError::NoMemory);
        }
        self.p_pos = new_p_pos;
        Ok(new_p_pos)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
        // Pages are never freed in the early allocator.
    }

    fn total_pages(&self) -> usize {
        (self.end - self.start) / SIZE
    }

    fn used_pages(&self) -> usize {
        (self.end - self.p_pos) / SIZE
    }

    fn available_pages(&self) -> usize {
        (self.p_pos - self.b_pos) / SIZE
    }
}
