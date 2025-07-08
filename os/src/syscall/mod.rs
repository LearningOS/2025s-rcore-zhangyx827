//! Implementation of syscalls
//!
//! The single entry point to all system calls, [`syscall()`], is called
//! whenever userspace wishes to perform a system call using the `ecall`
//! instruction. In this case, the processor raises an 'Environment call from
//! U-mode' exception, which is handled as one of the cases in
//! [`crate::trap::trap_handler`].
//!
//! For clarity, each single syscall is implemented as its own function, named
//! `sys_` then the name of the syscall. You can find functions like this in
//! submodules, and you should also implement syscalls this way.

/// write syscall
const SYSCALL_WRITE: usize = 64;
/// exit syscall
const SYSCALL_EXIT: usize = 93;
/// yield syscall
const SYSCALL_YIELD: usize = 124;
/// gettime syscall
const SYSCALL_GET_TIME: usize = 169;
/// trace syscall
const SYSCALL_TRACE: usize = 410;

mod fs;
mod process;

// use core::simd::LaneCount;

use fs::*;
use process::*;
use crate::task::get_current_task;
use crate::config::MAX_APP_NUM;
#[derive(Copy, Clone)]

struct CountSyscall {
    write: usize,
    exit: usize,
    get_time: usize,
    trace: usize,
    _yield: usize
}

static mut COUNTER: [CountSyscall; MAX_APP_NUM] = [CountSyscall {
    write: 0,
    exit: 0,
    get_time: 0,
    trace: 0,
    _yield: 0
}; MAX_APP_NUM];

impl CountSyscall {
    fn get_count(&self, id: usize) -> usize{
        match id {
            SYSCALL_WRITE => self.write,
            SYSCALL_TRACE => self.trace,
            SYSCALL_YIELD => self._yield,
            SYSCALL_EXIT => self.exit,
            SYSCALL_GET_TIME => self.get_time,
            _ => 0,
        }
        
    }
    fn modify(&mut self, id: usize) {
        match id {
            SYSCALL_WRITE => self.write += 1,
            SYSCALL_TRACE => self.trace += 1,
            SYSCALL_YIELD => self._yield += 1,
            SYSCALL_EXIT => self.exit += 1,
            SYSCALL_GET_TIME => self.get_time += 1,
            _ => {}
        }
    }
}
/// handle syscall exception with `syscall_id` and other arguments
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    let current = get_current_task();
    unsafe {COUNTER[current].modify(syscall_id)};
    match syscall_id {
        SYSCALL_WRITE =>  sys_write(args[0], args[1] as *const u8, args[2]), 
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_GET_TIME => sys_get_time(args[0] as *mut TimeVal, args[1]),
        SYSCALL_TRACE => sys_trace(args[0], args[1], args[2]),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
