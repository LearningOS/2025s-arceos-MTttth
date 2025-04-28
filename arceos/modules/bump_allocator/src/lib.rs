#![no_std]

use allocator::{BaseAllocator, ByteAllocator, PageAllocator};

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
    end: usize,
    b_pos: usize,
    p_pos: usize,
}

impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
        }
    }
}

impl<const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = start + size;
    }
    fn add_memory(&mut self, _start: usize, _size: usize) -> allocator::AllocResult {
        Err(allocator::AllocError::NoMemory)
    }
}

impl<const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    fn alloc(
        &mut self,
        layout: core::alloc::Layout,
    ) -> allocator::AllocResult<core::ptr::NonNull<u8>> {
        let align = layout.align();
        let size = layout.size();
        let aligned_cursor = (self.b_pos + align - 1) & !(align - 1);
        if aligned_cursor + size > self.p_pos {
            return Err(allocator::AllocError::NoMemory);
        }
        let ptr = aligned_cursor as *mut u8;
        self.b_pos = aligned_cursor + size;
        Ok(unsafe { core::ptr::NonNull::new_unchecked(ptr) })
    }
    fn dealloc(&mut self, pos: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        let size = layout.size();
        let ptr = pos.as_ptr() as usize;
    
        // 检查基本合法性
        if ptr < self.start || ptr >= self.end {
            panic!("EarlyAllocator: invalid deallocation, out of memory bounds");
        }
    
        // 不允许 dealloc Page 区域分配出来的内存
        if ptr >= self.p_pos {
            panic!("EarlyAllocator: cannot deallocate memory allocated by PageAllocator");
        }
    
        // 只允许回滚最近一次分配的内存 (LIFO)
        if ptr + size == self.b_pos {
            self.b_pos = ptr;
        } else {
            panic!("EarlyAllocator: invalid deallocation order (non-LIFO dealloc)");
        }
    }
    
    fn used_bytes(&self) -> usize {
        self.b_pos - self.start
    }
    fn available_bytes(&self) -> usize {
        self.p_pos - self.b_pos
    }
    fn total_bytes(&self) -> usize {
        self.end - self.start
    }
}

impl<const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    const PAGE_SIZE: usize = 4096; // Define the page size, e.g., 4KB

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> allocator::AllocResult<usize> {
        if align_pow2 % PAGE_SIZE != 0 {
            return Err(allocator::AllocError::InvalidParam);
        }
        let align_pages = align_pow2 / PAGE_SIZE;
        if !align_pages.is_power_of_two() {
            return Err(allocator::AllocError::InvalidParam);
        }
    
        let total_size = num_pages * PAGE_SIZE;
    
        let mut alloc_start = self.p_pos.checked_sub(total_size)
            .ok_or(allocator::AllocError::NoMemory)?;
        // 做对齐：alloc_start 向下对齐 align_pow2
        alloc_start = alloc_start & !(align_pow2 - 1);
        if alloc_start < self.b_pos {
            return Err(allocator::AllocError::NoMemory);
        }
        self.p_pos = alloc_start;
        Ok(alloc_start)
    }
    
    fn available_pages(&self) -> usize {
        (self.p_pos - self.b_pos) >> Self::PAGE_SIZE.trailing_zeros() as usize
    }
    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {
        let size = num_pages * PAGE_SIZE;
        let expected_pos = self.p_pos + size;
        if pos != expected_pos {
            panic!("EarlyAllocator: invalid deallocation order (non-LIFO dealloc)");
        }
        self.p_pos += size;
    }
    
    fn used_pages(&self) -> usize {
        (self.p_pos - self.end) >> Self::PAGE_SIZE.trailing_zeros() as usize
    }
    fn total_pages(&self) -> usize {
        (self.end - self.start) >> Self::PAGE_SIZE.trailing_zeros() as usize
    }
}
