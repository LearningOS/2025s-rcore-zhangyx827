//! Process management syscalls
// use riscv::addr::Page;

use core::intrinsics::size_of;

use crate::{config::PAGE_SIZE, mm::{frame_alloc, PTEFlags, PageTable, VirtAddr, VirtPageNum, VA_WIDTH_SV39}, task::{change_program_brk, current_user_token, exit_current_and_run_next, suspend_current_and_run_next}};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let token = current_user_token();  // 得到当前用户的一级页表的token
    let page_table = PageTable::from_token(token); // 得到页表
    let start_va = VirtAddr::from(ts as usize);
    let end_va = VirtAddr::from(ts as usize + size_of(TimeVal));
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    let token = current_user_token(); // 参考自 os/src/syscall/fs.rs 中sys_write通过传入current_user_token得到
    // 当前用户的一级页表的基地址的物理号 从而得到一级页表的基地址
    let page_table = PageTable::from_token(token); // 参考自 os/src/mm/page_table.rs 中translated_byte_buffer 
    // 通过token得到当前用户的页表
    let va = VirtAddr::from(id);
    let sign_bit = ((id & ((1 << VA_WIDTH_SV39) - 1)) >> VA_WIDTH_SV39) & 1;
    let high_bits = id >> VA_WIDTH_SV39;
    if (sign_bit == 1 && high_bits == 0) || (sign_bit == 0 && high_bits != 0) {
        return -1 as isize;
    }
    // 表示访问到不可访问的地址 返回-1
    let vpn = va.floor();
    let page_offset = va.page_offset(); // 得到页内偏移
    let op_pte = page_table.translate(vpn);
    let pte = match op_pte {
        None => { return -1; }
        _ =>  op_pte.unwrap()
    };
    match trace_request {
        0 => {
            if !pte.readable() || !pte.is_valid() {
                return -1 as isize;
            }
            let ppn = pte.ppn();  // 通过页表项得到物理页号
            let array = ppn.get_bytes_array(); //得到一个页面的字节数组
            let ptr: &'static mut u8 = &mut array[page_offset]; // 通过页内偏移得到物理地址
            return *ptr as isize;
        }
        1 => {
            if !pte.writable() || !pte.is_valid(){
                return -1 as isize;
            }
            let ppn = pte.ppn();
            // let mut pa = PhysAddr::from(ppn);
            let array = ppn.get_bytes_array();
            let ptr: &'static mut u8 = &mut array[page_offset];
            *ptr = data as u8;
            return 0;
        }
        _ => { return -1 as isize; }
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    let token= current_user_token(); // 得到当前用户的地址空间的页表的token
    let mut page_table = PageTable::from_token(token);  //得到页表
    let start_va = VirtAddr::from(start);
    if start_va.page_offset() != 0 {
        return -1 as isize;  // start 没有按照页面的大小对齐。
    }
    if prot & !0x7 != 0 || prot & 0x7 == 0 {
        return -1 as isize;  
    }
    let mut newlen = len;
    if len % PAGE_SIZE != 0 {
        newlen = ((len / PAGE_SIZE) + 1) * PAGE_SIZE;  // 如果没有按照页面的大小对齐 那么向上取整。
    }
    let end_va = VirtAddr::from(start + newlen);
    let start_vpn = VirtPageNum::from(start_va);
    let end_vpn = VirtPageNum::from(end_va);
    for vpn in start_vpn.0..end_vpn.0 {
        let op_pte = page_table.translate(VirtPageNum(vpn)); // 通过translate方法查找页表
        match op_pte {
            None => {
                let op_frame = frame_alloc();
                match op_frame {
                    None => { return -1 as isize; }   // 物理内存不足
                    _ => {
                        let mut pte_flag = PTEFlags::U;   // 增加PTE_U
                        let frame = op_frame.unwrap();
                        let ppn = frame.ppn;
                        let read_bit = prot & 1;
                        let write_bit = prot & (1 << 1);
                        let exe_bit = prot & (1 << 2);
                        if read_bit == 1 {
                            pte_flag |= PTEFlags::R;
                        }
                        if write_bit == 1 {
                            pte_flag |= PTEFlags::W;
                        }
                        if exe_bit == 1 {
                            pte_flag |= PTEFlags::X;
                        } 
                        // 这个部分参考了 os/src/mm/memory_set.rs中的from_elf的将LOAD段加载到程序地址空间的实现
                        page_table.map(VirtPageNum(vpn), ppn, pte_flag);  // 映射的是页表项不是与MapPermission不同
                    }
                }
            }
            _ => { return -1 as isize; }    // 该虚拟页面已经被映射过了
        }
    }
    return 0;
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    let token = current_user_token();
    let mut page_table = PageTable::from_token(token);
    let mut newlen = len;
    if len / PAGE_SIZE != 0 {
        newlen = ((len % PAGE_SIZE) + 1) * PAGE_SIZE;  // 如果没有按照页面的大小对齐 那么向上取整。
    }
    let start_va = VirtAddr::from(start);
    let end_va = VirtAddr::from(start + newlen);
    let start_vpn = VirtPageNum::from(start_va);
    let end_vpn = VirtPageNum::from(end_va);
    for vpn in start_vpn.0..end_vpn.0 {
        let op_pte = page_table.translate(VirtPageNum(vpn));
        match op_pte {
            None => { return -1 as isize; } // 存在没有映射的页面 
            _ => {
                let pte = op_pte.unwrap();
                if !pte.is_valid() {
                    return -1 as isize; // 这里目前不确定是不是这个呢。
                }
                page_table.unmap(VirtPageNum(vpn));
            }
        }
    }
    return 0;
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
