#![no_std]
#![allow(unused_variables)]

use allocator::{BaseAllocator, ByteAllocator, AllocResult};
use axlog::ax_println;
use core::ptr::{NonNull, null_mut};
use core::alloc::Layout;
use core::mem;

// 优化1：使用更紧凑的内存块结构
#[repr(C)]
struct Block {
    next: *mut Block,
    size: usize,
}

// 优化2：使用常量定义关键参数
const MAX_INDICATOR: usize = 256;
const POOL_SIZES: [usize; 8] = [
    32, 128, 512, 2048,
    8 * 1024, 32 * 1024, 128 * 1024, 512 * 1024
];

// 优化3：使用类型别名提高代码可读性
type PoolArray = [u8];

// 优化4：静态内存池使用宏定义，减少代码重复
macro_rules! define_memory_pools {
    ($($name:ident: $size:expr),*) => {
        $(static mut $name: [u8; $size + MAX_INDICATOR] = [0; $size + MAX_INDICATOR];)*
    }
}

define_memory_pools! {
    POOL_32: 32,
    POOL_128: 128,
    POOL_512: 512,
    POOL_2048: 2048,
    POOL_8_1024: 8*1024,
    POOL_32_1024: 32*1024,
    POOL_128_1024: 128*1024,
    POOL_512_1024: 512*1024
}

// 优化5：添加内存池管理结构
#[derive(Debug)]
struct PoolInfo {
    base: *mut u8,
    size: usize,
    used: bool,
}

pub struct LabByteAllocator {
    start: usize,
    total_size: usize,
    used_size: usize,
    free_list: *mut Block,
    // 优化6：添加内存池追踪
    pools: [PoolInfo; 8],
    allocation_count: usize,
}

unsafe impl Send for LabByteAllocator {}
unsafe impl Sync for LabByteAllocator {}

impl LabByteAllocator {
    pub const fn new() -> Self {
        Self {
            start: 0,
            total_size: 0,
            used_size: 0,
            free_list: null_mut(),
            pools: [PoolInfo {
                base: null_mut(),
                size: 0,
                used: false
            }; 8],
            allocation_count: 0,
        }
    }

    // 优化7：改进内存块分配策略
    unsafe fn find_best_fit(&mut self, size: usize) -> Option<*mut Block> {
        let mut best_fit = None;
        let mut best_size = usize::MAX;
        let mut prev = &mut self.free_list as *mut *mut Block;
        let mut current = self.free_list;

        while !current.is_null() {
            let block_size = (*current).size;
            if block_size >= size && block_size < best_size {
                best_fit = Some((prev, current));
                best_size = block_size;
                
                // 如果找到完全匹配的块，立即返回
                if block_size == size {
                    break;
                }
            }
            prev = &mut (*current).next;
            current = *prev;
        }

        best_fit.map(|(prev, block)| {
            *prev = (*block).next;
            block
        })
    }

    // 优化8：改进内存池分配策略
    unsafe fn allocate_from_pool(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        if let Some(index) = POOL_SIZES.iter()
            .position(|&size| size >= layout.size() && size >= layout.align())
        {
            if !self.pools[index].used {
                let pool = match index {
                    0 => &mut POOL_32,
                    1 => &mut POOL_128,
                    2 => &mut POOL_512,
                    3 => &mut POOL_2048,
                    4 => &mut POOL_8_1024,
                    5 => &mut POOL_32_1024,
                    6 => &mut POOL_128_1024,
                    7 => &mut POOL_512_1024,
                    _ => return None,
                };
                self.pools[index].used = true;
                self.pools[index].base = pool.as_mut_ptr();
                self.pools[index].size = POOL_SIZES[index];
                return NonNull::new(pool.as_mut_ptr());
            }
        }
        None
    }

    // 优化9：添加内存对齐处理
    fn align_up(size: usize, align: usize) -> usize {
        (size + align - 1) & !(align - 1)
    }
}

impl BaseAllocator for LabByteAllocator {
    fn init(&mut self, start: usize, size: usize) {
        unsafe {
            let aligned_start = Self::align_up(start, mem::align_of::<Block>());
            let aligned_size = size - (aligned_start - start);
            
            self.start = aligned_start;
            self.total_size = aligned_size;
            
            let initial_block = aligned_start as *mut Block;
            (*initial_block).size = aligned_size - mem::size_of::<Block>();
            (*initial_block).next = null_mut();
            self.free_list = initial_block;
        }
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        unsafe {
            let aligned_start = Self::align_up(start, mem::align_of::<Block>());
            let aligned_size = size - (aligned_start - start);
            
            let new_block = aligned_start as *mut Block;
            (*new_block).size = aligned_size - mem::size_of::<Block>();
            (*new_block).next = self.free_list;
            self.free_list = new_block;
            
            self.total_size += aligned_size;
            self.merge_blocks();
        }
        Ok(())
    }
}

impl ByteAllocator for LabByteAllocator {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        unsafe {
            // 优化10：优先使用内存池
            if let Some(ptr) = self.allocate_from_pool(layout) {
                return Ok(ptr);
            }

            // 计算所需大小（包含对齐要求）
            let size = Self::align_up(layout.size(), layout.align());
            
            if let Some(block) = self.find_best_fit(size) {
                let aligned_ptr = Self::align_up(
                    block.add(1) as usize,
                    layout.align()
                );
                self.used_size += size;
                self.allocation_count += 1;
                Ok(NonNull::new_unchecked(aligned_ptr as *mut u8))
            } else {
                Err(allocator::AllocError::NoMemory)
            }
        }
    }

    fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        unsafe {
            // 检查是否是内存池分配的内存
            if self.pools.iter().any(|pool| {
                ptr.as_ptr() >= pool.base && 
                ptr.as_ptr() < pool.base.add(pool.size)
            }) {
                return;
            }

            let block = (ptr.as_ptr() as *mut Block).sub(1);
            (*block).next = self.free_list;
            self.free_list = block;
            
            self.used_size -= layout.size();
            self.allocation_count -= 1;
            
            self.merge_blocks();
        }
    }

    fn total_bytes(&self) -> usize {
        self.total_size
    }

    fn used_bytes(&self) -> usize {
        self.used_size
    }

    fn available_bytes(&self) -> usize {
        self.total_size - self.used_size
    }
}
