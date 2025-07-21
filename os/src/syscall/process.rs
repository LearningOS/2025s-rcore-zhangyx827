//! Process management syscalls
use crate::task::TaskControlBlock;
use crate::timer::get_time_us;
use alloc::sync::Arc;
use crate::mm::address::StepByOne;
use crate::task::current_pagetable_ptr;
use crate::config::PAGE_SIZE;
use crate::mm::VirtAddr;
use crate::mm::{VirtPageNum, PageTable, PTEFlags};
use core::mem;
use crate::mm::frame_alloc;
use crate::{
    fs::{open_file, OpenFlags},
    mm::{translated_refmut, translated_str},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_get_time NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let us = get_time_us();
    let mut va = ts as usize;
    let size = mem::size_of::<usize>();
    let token = current_user_token();
    let mut bits = 0b11111111;
    let sec = us / 1_000_000;
    let usec = us % 1_000_000;
    // println!("sec is {} and usec is {} in kernel\n", sec, usec);
    for i in 0..size {
        let ptr: &'static mut u8 = translated_refmut(token, va as *mut u8);
        *ptr = ((sec & bits) >> (i * 8)) as u8;
        // println!("*ptr of {} is {}\n",i, *ptr);
        bits = bits << 8 | 0b11111111;
        va += 1;
    }
    bits = 0b11111111;
    for i in 0..size {
        let ptr: &'static mut u8 = translated_refmut(token, va as *mut u8);
        *ptr = ((usec & bits) >> (i * 8)) as u8;
        bits = bits << 8 | 0b11111111;
        va += 1;
    }
    return 0;
}



/// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_mmap NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let page_table_ptr: *mut PageTable  = current_pagetable_ptr();
    if start % PAGE_SIZE != 0 {
        return -1 as isize;
    }
    if ((prot & !0x7) != 0) || (prot & 0x7 == 0) {
        print!("1!!!!!!!!!!!!!!!!!\n");
        return -1 as isize;
    }
    let mut newlen = len;
    if len % PAGE_SIZE != 0 {
        newlen = (len / PAGE_SIZE + 1) * PAGE_SIZE;
    }
    let va_start = VirtAddr::from(start);
    let mut vpn: VirtPageNum = va_start.into();
    for _ in 0.. newlen / PAGE_SIZE {
        unsafe {
            let opt_pte = (*page_table_ptr).find_pte(vpn);
            match opt_pte {
                None => { 
                    let opt_frame = frame_alloc();
                    match opt_frame {
                        None => { 
                            print!("2!!!!!!!!!!!!!!!!!\n");
                            return -1 as isize; 
                        }
                        _ => {
                            let frame = opt_frame.unwrap();
                            let mut pte_flag = PTEFlags::U;
                            let read_bit = prot & 1;
                            let write_bit = prot & (1 << 1);
                            let exe_bit = prot & (1 << 2);
                            if read_bit != 0 {
                                pte_flag |= PTEFlags::R;
                            }
                            if write_bit != 0 {
                                pte_flag |= PTEFlags::W;
                            }
                            if exe_bit != 0 {
                                pte_flag |= PTEFlags::X;
                            } 
                            (*page_table_ptr).map(vpn, frame.ppn, pte_flag);
                        }
                    }
                }
                _ => {
                    let pte = opt_pte.unwrap();
                    if pte.is_valid() {
                        print!("3!!!!!!!!!!!!!!!!!\n");
                        return -1 as isize;
                    }
                    let opt_frame = frame_alloc();
                    match opt_frame {
                        None => { 
                            print!("4!!!!!!!!!!!!!!!!!\n");
                            return -1 as isize; 
                        }
                        _ => {
                            let frame = opt_frame.unwrap();
                            let mut pte_flag = PTEFlags::U;
                            let read_bit = prot & 1;
                            let write_bit = prot & (1 << 1);
                            let exe_bit = prot & (1 << 2);
                            if read_bit != 0 {
                                pte_flag |= PTEFlags::R;
                            }
                            if write_bit != 0 {
                                pte_flag |= PTEFlags::W;
                            }
                            if exe_bit != 0 {
                                pte_flag |= PTEFlags::X;
                            } 
                            (*page_table_ptr).map(vpn, frame.ppn, pte_flag);
                        }
                    }
                }
            }
        }
        vpn.step();
    }
    0
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_munmap NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let page_table_ptr: *mut PageTable  = current_pagetable_ptr();
    if start % PAGE_SIZE != 0 {
        return -1 as isize;
    }
    let mut newlen = len;
    if len % PAGE_SIZE != 0 {
        newlen = (len / PAGE_SIZE + 1) * PAGE_SIZE;
    }
    let va_start = VirtAddr::from(start);
    let mut vpn: VirtPageNum = va_start.into();
    for _ in 0.. newlen / PAGE_SIZE {
        unsafe {
            let opt_pte = (*page_table_ptr).find_pte(vpn);
            match opt_pte {
                None => { 
                    return -1 as isize;
                }
                _ => {
                    let pte = opt_pte.unwrap();
                    if !pte.is_valid() {
                        return -1 as isize;
                    }
                    (*page_table_ptr).unmap(vpn);
                }
            }
        }
        vpn.step();
    }
    0
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let file_name = translated_str(token, path);
    if let Some(inode) = open_file(file_name.as_str(), OpenFlags::RDONLY) {
        let data = inode.read_all();
        let child_task = TaskControlBlock::new(data.as_slice());
        let parent_task = current_task().unwrap();
        child_task.inner_exclusive_access().parent = Some(Arc::downgrade(&parent_task));    // 参考了task/task.rs中fork的实现
        let child_pid = child_task.pid.0 as isize;
        let wrapper = Arc::new(child_task);
        parent_task.inner_exclusive_access().children.push(wrapper.clone());
        add_task(wrapper.clone());   // 加入到调度队列当中
        return child_pid;
    } else {
        return -1 as isize;
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if prio <= 1 {
        return -1
    } else {
        let current_task = current_task().unwrap();
        current_task.inner_exclusive_access().priority = prio;
        prio
    }
}