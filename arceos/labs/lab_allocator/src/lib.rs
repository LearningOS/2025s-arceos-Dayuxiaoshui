//! Allocator algorithm in lab.

#![no_std]
#![allow(unused_variables)]

use allocator::{BaseAllocator, ByteAllocator, AllocResult};
use core::ptr::NonNull;
use core::alloc::Layout;

// 内存块元数据结构
struct BlockMeta {
    size: usize,      // 块大小(不包括元数据本身)
    free: bool,       // 是否空闲
    next: Option<NonNull<BlockMeta>>, // 下一个块的指针
}

// 确保BlockMeta是线程安全的(如果需要)
unsafe impl Send for BlockMeta {}

pub struct LabByteAllocator {
    head: Option<NonNull<BlockMeta>>, // 内存块链表头
    total_bytes: usize,               // 总内存大小
    used_bytes: usize,                // 已使用内存大小
}

impl LabByteAllocator {
    pub const fn new() -> Self {
        Self {
            head: None,
            total_bytes: 0,
            used_bytes: 0,
        }
    }

    // 合并相邻的空闲块以减少碎片
    unsafe fn coalesce(&mut self) {
        let mut current = self.head;
        while let Some(mut curr_ptr) = current {
            let curr = curr_ptr.as_mut();
            if let Some(mut next_ptr) = curr.next {
                let next = next_ptr.as_mut();
                if curr.free && next.free {
                    // 合并当前块和下一个块
                    curr.size += next.size + core::mem::size_of::<BlockMeta>();
                    curr.next = next.next;
                    // 不需要移动current，因为可能还需要继续合并
                    continue;
                }
            }
            current = curr.next;
        }
    }

    // 计算对齐调整后的地址和填充大小
    fn align_up(addr: usize, align: usize) -> (usize, usize) {
        let remainder = addr % align;
        if remainder == 0 {
            (addr, 0)
        } else {
            (addr + align - remainder, align - remainder)
        }
    }
}

impl BaseAllocator for LabByteAllocator {
    fn init(&mut self, start: usize, size: usize) {
        unsafe {
            // 确保有足够空间存储元数据
            if size < core::mem::size_of::<BlockMeta>() {
                panic!("Initial memory region too small");
            }

            // 初始化第一个内存块
            let block_ptr = start as *mut BlockMeta;
            (*block_ptr).size = size - core::mem::size_of::<BlockMeta>();
            (*block_ptr).free = true;
            (*block_ptr).next = None;
            
            self.head = NonNull::new(block_ptr);
            self.total_bytes = size;
            self.used_bytes = 0;
        }
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult<()> {
        if size < core::mem::size_of::<BlockMeta>() {
            return Err(allocator::AllocError::NoMemory);
        }

        unsafe {
            // 创建新内存块
            let new_block = start as *mut BlockMeta;
            (*new_block).size = size - core::mem::size_of::<BlockMeta>();
            (*new_block).free = true;
            (*new_block).next = self.head;

            // 更新分配器状态
            self.head = NonNull::new(new_block);
            self.total_bytes += size;
            self.coalesce(); // 合并空闲块
        }
        
        Ok(())
    }
}

impl ByteAllocator for LabByteAllocator {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        // 计算所需总大小(包括对齐填充和元数据)
        let (aligned_addr, padding) = Self::align_up(
            core::mem::size_of::<BlockMeta>(), 
            layout.align()
        );
        let required_size = layout.size() + padding;

        // 遍历链表寻找合适的空闲块
        let mut prev: Option<NonNull<BlockMeta>> = None;
        let mut current = self.head;
        
        while let Some(mut curr_ptr) = current {
            unsafe {
                let curr = curr_ptr.as_mut();
                if curr.free && curr.size >= required_size {
                    // 计算剩余空间是否足够分割新块
                    let remaining_size = curr.size - required_size;
                    if remaining_size > core::mem::size_of::<BlockMeta>() {
                        // 分割剩余空间为新块
                        let new_block_addr = curr_ptr.as_ptr() as usize + 
                            core::mem::size_of::<BlockMeta>() + required_size;
                        let new_block = new_block_addr as *mut BlockMeta;
                        
                        (*new_block).size = remaining_size - core::mem::size_of::<BlockMeta>();
                        (*new_block).free = true;
                        (*new_block).next = curr.next;
                        
                        curr.size = required_size;
                        curr.next = NonNull::new(new_block);
                    }
                    
                    // 标记为已分配
                    curr.free = false;
                    self.used_bytes += curr.size + core::mem::size_of::<BlockMeta>();
                    
                    // 返回分配的内存(跳过元数据和填充)
                    let user_ptr = (curr_ptr.as_ptr() as *mut u8)
                        .add(core::mem::size_of::<BlockMeta>() + padding);
                    return Ok(NonNull::new_unchecked(user_ptr));
                }
                prev = current;
                current = curr.next;
            }
        }
        
        Err(allocator::AllocError::NoMemory)
    }

    fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        unsafe {
            // 获取块元数据指针
            let block_ptr = (ptr.as_ptr() as usize - core::mem::size_of::<BlockMeta>()) as *mut BlockMeta;
            let block = &mut *block_ptr;
            
            // 安全检查
            assert!(!block.free, "double free detected");
            assert!(block.size >= layout.size(), "invalid deallocation size");
            
            // 标记为空闲
            block.free = true;
            self.used_bytes -= block.size + core::mem::size_of::<BlockMeta>();
            
            // 合并空闲块
            self.coalesce();
        }
    }

    fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    fn available_bytes(&self) -> usize {
        self.total_bytes - self.used_bytes
    }
}
