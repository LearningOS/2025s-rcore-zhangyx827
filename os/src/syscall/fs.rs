//! File and filesystem-related syscalls
use crate::fs::{delete_dirent, StatMode};
use crate::fs::{open_file, OpenFlags, Stat};
use crate::mm::{translated_byte_buffer, translated_str, translated_refmut, UserBuffer};
use crate::task::{current_task, current_user_token};
use crate::fs::add_dirent;
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(fd: usize, st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    // 以上参考了sys_read的实现判断fd是否有效
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        let stat = file.stat();
        let mut va = st as usize;
        for _ in 0..core::mem::size_of::<u64>() {
            let p = translated_refmut(token, va as *mut u8);
            *p = 0;
            va += 1;
        }
        let mut bits = 0b11111111;
        for i in 0..core::mem::size_of::<u64>() {
            let p = translated_refmut(token, va as *mut u8);
            *p = ((stat.ino & bits) >> (i * 8)) as u8;
            bits = bits << 8 | 0b11111111;
            va += 1;
        }
        // for i in 0..core::mem::size_of::<u32>() {
            //     let p = translated_refmut(token, va as *mut u8);
            //     *p = ((stat.mode.bits() as u64 & bits) >> (i * 8)) as u8;
            //     va += 1;
            // }
        bits = 0b11111111;
        *translated_refmut(token, va as *mut StatMode) = stat.mode;
        va += core::mem::size_of::<StatMode>();
        // debug!("stat.nlink is !!!!!!!!!!!!!!{}\n\n\n\n", stat.nlink);
        // debug!("stat.mode is !!!!!!!!!!!!!!{}\n\n\n\n", stat.mode);
        for i in 0..core::mem::size_of::<u32>() {
            let p = translated_refmut(token, va as *mut u8);
            *p = ((stat.nlink as u64 & bits) >> (i * 8)) as u8;
            bits = bits << 8 | 0b11111111;
            va += 1;
        }
    } else {
        return -1
    }
    0
}

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(old_name: *const u8, new_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // let task = current_task().unwrap();
    let token = current_user_token();
    let name1 = translated_str(token, old_name);   // 参考了sys_open得到字符串的实现
    let name2 = translated_str(token, new_name);
    if name1 == name2 {
        return -1;
    }
    // let inode = find(&name1).unwrap();
    debug!("after find! name1:{} name2{}\n", name1, name2);
    add_dirent(&name1, &name2);
    0
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let path = translated_str(token, name);
    delete_dirent(&path);
    0
}
