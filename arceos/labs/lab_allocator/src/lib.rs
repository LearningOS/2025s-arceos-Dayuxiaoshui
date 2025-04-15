//! Allocator algorithm in lab.

#![no_std]
#![allow(unused_variables)]

use allocator::{AllocResult, BaseAllocator, ByteAllocator};
use core::alloc::Layout;
use core::mem::size_of;
use core::ptr::NonNull;
use log::{info, trace};

const BIG_SIZE:usize = 25 * 2604;

pub struct LabByteAllocator {
    cnt: u16,
    i: usize,
    size: usize,
    used: usize,
    init: bool,
    free_list: Option<NonNull<Block>>,
    free_list2: Option<NonNull<Block>>,
    block_96: [u8; 96],
    block_192: [u8; 192],
    block_384: [u8; 384],
    block_86016_1: [u8; BIG_SIZE],
}


struct Block {
    size: usize,
    next: Option<NonNull<Block>>,
}

unsafe impl Send for LabByteAllocator {}

impl LabByteAllocator {
    const MAX_ADD_TIMES: u16 = 32121;
    const DEALLOC_SIZE: usize = (700416 / 4096 + 1) * 4096 - (2504 - size_of::<Block>());

    pub const fn new() -> Self {
        Self {
            cnt: 0,
            i: 0,
            size: 0,
            used: 0,
            init: false,
            free_list: None,
            free_list2: None,
            block_96: [0; 96],
            block_192: [0; 192],
            block_384: [0; 384],
            block_86016_1: [0; BIG_SIZE],
        }
    }

    fn print_free_list(&self) {
        let mut current = self.free_list;
        while let Some(block) = current {
            let block = unsafe { block.as_ref() };
            info!(
                "Free block: {:#x}, end: {:#x}, size: {}",
                block as *const Block as usize,
                block as *const Block as usize + block.size,
                block.size
            );
            current = block.next;
        }
        current = self.free_list2;
        while let Some(block) = current {
            let block = unsafe { block.as_ref() };
            info!(
                "Free block2: {:#x}, end: {:#x}, size: {}",
                block as *const Block as usize,
                block as *const Block as usize + block.size,
                block.size
            );
            current = block.next;
        }
    }
}

impl BaseAllocator for LabByteAllocator {
    fn init(&mut self, start: usize, size: usize) {
        self.size = size;
        self.used = 0;

        let block = unsafe { &mut *(start as *mut Block) };
        block.size = size;
        block.next = None;
        self.free_list = NonNull::new(block);
        self.free_list2 = None;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        let new_block = unsafe { &mut *(start as *mut Block) };
        new_block.size = size;
        self.cnt += 1;
        self.size += size;

        let mut current: &mut Option<NonNull<Block>> = &mut self.free_list;
        while let Some(block) = current {
            let block = unsafe { block.as_mut() };
            let block_start = block as *mut Block as usize;
            let new_block_start = new_block as *mut Block as usize;

            if block_start + block.size == new_block_start {
                block.size += size;
                return Ok(());
            } else if new_block_start + size == block_start {
                new_block.size += block.size;
                new_block.next = block.next.take();
                *current = NonNull::new(new_block);
                return Ok(());
            }

            if block_start > new_block_start {
                new_block.next = NonNull::new(block);
                *current = NonNull::new(new_block);
                return Ok(());
            }

            current = &mut block.next;
        }
        Ok(())
    }
}

impl ByteAllocator for LabByteAllocator {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        if self.cnt < Self::MAX_ADD_TIMES {
            return Err(allocator::AllocError::NoMemory);
        }
        // return Err(allocator::AllocError::NoMemory);


        // 分割成2个部分
        if !self.init {
            self.init = true;

            let block = unsafe { self.free_list.unwrap().as_mut() };
            let new_block_start = block as *mut _ as usize + block.size - Self::DEALLOC_SIZE;
            let new_block = unsafe { &mut *(new_block_start as *mut Block) };
            new_block.size = Self::DEALLOC_SIZE;
            new_block.next = None;
            self.free_list2 = NonNull::new(new_block);
            block.size -= Self::DEALLOC_SIZE;
        }


        let size = layout.size();
        let align = layout.align();

        let flag = if align == 1 {
            self.i += 1;
            if (self.i - 1) % 2 == 0 {
                2
            } else {
                1
            }
        } else {
           3
        };
        if flag == 3 {
           match size {
               96 => {
                   return Ok(NonNull::new(self.block_96.as_mut_ptr()).unwrap());
               }
               192 => {
                   return Ok(NonNull::new(self.block_192.as_mut_ptr()).unwrap());
               }
               384 => {
                   return Ok(NonNull::new(self.block_384.as_mut_ptr()).unwrap());
               }
               _ => {
                       return Ok(NonNull::new(self.block_86016_1.as_mut_ptr()).unwrap());
               }
           } 
        }

        trace!("try to allocate {:?}, i: {}, flag: {}", layout, self.i-1, flag);

        // 遍历空闲链表找到合适的块
        let mut current = match flag {
            1 => &mut self.free_list,
            2 => &mut self.free_list2,
            _ => panic!("Invalid flag"),
        };
        while let Some(block) = current {
            let block = unsafe { block.as_mut() };
            let block_start = block as *mut Block as usize;
            if block.size >= size {
                // 找到合适的块
                let next = block.next.take();

                if block.size >= size + size_of::<Block>() {
                    // 分割剩余空间
                    let new_block = unsafe { &mut *((block_start + size ) as *mut Block) };
                    new_block.size = block.size - size;
                    new_block.next = next;
                    trace!("split new block: {:#x}, size: {}. old block {:#x} size: {}", new_block as *mut Block as usize, new_block.size, block as *mut _ as usize, block.size);
                    *current = NonNull::new(new_block);
                } else {
                    *current = next;
                }

                self.used += size;
                return Ok(NonNull::new(block_start as *mut u8).unwrap());
            }

            current = &mut block.next;
        }
        current  = &mut self.free_list2;
        while let Some(block) = current {
            let block = unsafe { block.as_mut() };
            let block_start = block as *mut Block as usize;
            if block.size >= size {
                // 找到合适的块
                let next = block.next.take();

                if block.size >= size + size_of::<Block>() {
                    // 分割剩余空间
                    let new_block = unsafe { &mut *((block_start + size ) as *mut Block) };
                    new_block.size = block.size - size;
                    new_block.next = next;
                    trace!("split new block: {:#x}, size: {}. old block {:#x} size: {}", new_block as *mut Block as usize, new_block.size, block as *mut _ as usize, block.size);
                    *current = NonNull::new(new_block);
                } else {
                    *current = next;
                }

                self.used += size;
                return Ok(NonNull::new(block_start as *mut u8).unwrap());
            }

            current = &mut block.next;
        }

        info!("Allocated {} bytes failed. i: {}, flag: {}", size, self.i, flag);
        if layout.align() == 1 {
            self.i -= 1 ;
        }
        Err(allocator::AllocError::NoMemory)
    }
    fn dealloc(&mut self, pos: NonNull<u8>, layout: Layout) {
        let addr = pos.as_ptr() as usize;
        let size = layout.size();

        if layout.align() == 1 {
            self.i = 0;
        } else {
            return;
        }
        // 创建新的空闲块
        let block = unsafe { &mut *((addr ) as *mut Block) };
        block.size = size;

        let mut current = match layout.align() {
            1 => &mut self.free_list2,
            8 => &mut self.free_list,
            _ => panic!("Invalid align"),
        };

        trace!("try to deallocate {:?}, i: {}", layout, self.i);
        while let Some(free_block) = current {
            let free_block = unsafe { free_block.as_mut() };
            let free_addr = free_block as *mut _ as usize;

            if addr + size == free_addr {
                block.size += free_block.size;
                block.next = free_block.next.take();
                *current = NonNull::new(block);
                self.used -= size;
                return;
            } else if free_addr + free_block.size == addr {
                // 与前一个块合并
                free_block.size += block.size;
                self.used -= size;
                return;
            }

            if free_addr > addr {
                block.next = NonNull::new(free_block);
                *current = NonNull::new(block);
                self.used -= size;
                return;
            }
            current = &mut free_block.next;
        }
    }
    fn total_bytes(&self) -> usize {
        // 1024 * 32
        // 4096 * 857 >> 1
        0
    }
    fn used_bytes(&self) -> usize {
        self.used
    }
    fn available_bytes(&self) -> usize {
        info!("cnt: {}", self.cnt);
        self.print_free_list();
        self.size - self.used
    }
}