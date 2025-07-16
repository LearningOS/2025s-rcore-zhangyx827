//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.

mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::loader::{get_app_data, get_num_app};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;
use crate::syscall::{SYSCALL_EXIT, SYSCALL_GET_TIME, SYSCALL_MMAP, SYSCALL_MUNMAP, SYSCALL_SBRK, SYSCALL_TRACE, SYSCALL_WRITE, SYSCALL_YIELD};

use switch::__switch;
use crate::mm::PageTable;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;

/// The task manager, where all the tasks are managed.
///
/// Functions implemented on `TaskManager` deals with all task state transitions
/// and task context switching. For convenience, you can find wrappers around it
/// in the module level.
///
/// Most of `TaskManager` are hidden behind the field `inner`, to defer
/// borrowing checks to runtime. You can see examples on how to use `inner` in
/// existing functions on `TaskManager`.
pub struct TaskManager {
    /// total number of tasks
    num_app: usize,
    /// use inner value to get mutable access
    inner: UPSafeCell<TaskManagerInner>,
}

struct CntSys {
    cnt_write: isize,
    cnt_exit: isize,
    cnt_gettime: isize,
    cnt_yield: isize,
    cnt_trace: isize,
    cnt_mmap: isize,
    cnt_munmap: isize,
    cnt_sbrk: isize
}

impl CntSys {
    pub fn new() -> Self {
        Self {
            cnt_write: 0,
            cnt_exit: 0,
            cnt_gettime: 0,
            cnt_yield: 0,
            cnt_trace: 0,
            cnt_mmap: 0,
            cnt_munmap: 0,
            cnt_sbrk: 0
        }
    }
}

/// The task manager inner in 'UPSafeCell'
struct TaskManagerInner {
    /// task list
    tasks: Vec<TaskControlBlock>,
    cnts: Vec<CntSys>,
    /// id of current `Running` task
    current_task: usize
}

lazy_static! {
    /// a `TaskManager` global instance through lazy_static!
    pub static ref TASK_MANAGER: TaskManager = {
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        let mut cnts: Vec<CntSys> =  Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
            cnts.push(CntSys::new());
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    cnts,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    /// Run the first task in task list.
    ///
    /// Generally, the first task in task list is an idle task (we call it zero process later).
    /// But in ch4, we load apps statically, so the first task is a real app.
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let next_task = &mut inner.tasks[0];
        next_task.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(&mut _unused as *mut _, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// Change the status of current `Running` task into `Ready`.
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Ready;
    }

    /// Change the status of current `Running` task into `Exited`.
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Exited;
    }

    /// Find next task to run and return task id.
    ///
    /// In this case, we only return the first `Ready` task in task list.
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// Get the current 'Running' task's token.
    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_user_token()
    }

    
    /// Change the current 'Running' task's program break
    pub fn change_current_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].change_program_brk(size)
    }
    
    /// Switch current `Running` task to the task we have found,
    /// or there is no `Ready` task and we can exit with all applications completed
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // go back to user mode
        } else {
            panic!("All applications completed!");
        }
    }
    /// Get the current 'Running' task's trap contexts.
    fn get_current_trap_cx(&self) -> &'static mut TrapContext {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_trap_cx()
    }
    /// get the pointer of current page_table 
    pub fn current_pagetable(&self) -> *mut PageTable{
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        let page_table = inner.tasks[cur].get_user_page_table();
        page_table
    }
    fn modify_cnt(&self, syscall_id: usize) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        match syscall_id {
            SYSCALL_WRITE => inner.cnts[cur].cnt_write += 1,
            SYSCALL_EXIT => inner.cnts[cur].cnt_exit += 1,
            SYSCALL_YIELD =>inner.cnts[cur].cnt_yield += 1,
            SYSCALL_GET_TIME => inner.cnts[cur].cnt_gettime += 1,
            SYSCALL_TRACE => inner.cnts[cur].cnt_trace += 1,
            SYSCALL_MMAP => inner.cnts[cur].cnt_mmap += 1,
            SYSCALL_MUNMAP => inner.cnts[cur].cnt_munmap += 1,
            SYSCALL_SBRK => inner.cnts[cur].cnt_sbrk += 1,
            _ => {}
        }
    }
    fn get_count(&self, syscall_id: usize) -> isize {
        let inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        match syscall_id {
            SYSCALL_WRITE =>  inner.cnts[cur].cnt_write,
            SYSCALL_EXIT => inner.cnts[cur].cnt_exit, 
            SYSCALL_YIELD =>inner.cnts[cur].cnt_yield, 
            SYSCALL_GET_TIME => inner.cnts[cur].cnt_gettime, 
            SYSCALL_TRACE => inner.cnts[cur].cnt_trace, 
            SYSCALL_MMAP => inner.cnts[cur].cnt_mmap,
            SYSCALL_MUNMAP => inner.cnts[cur].cnt_munmap, 
            SYSCALL_SBRK => inner.cnts[cur].cnt_sbrk, 
            _ => { -1 as isize }
        }
    }

}

/// Run the first task in task list.
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// Switch current `Running` task to the task we have found,
/// or there is no `Ready` task and we can exit with all applications completed
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// Change the status of current `Running` task into `Ready`.
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

/// Change the status of current `Running` task into `Exited`.
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

/// Get the current 'Running' task's token.
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

/// Get the current 'Running' task's trap contexts.
pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}

/// Change the current 'Running' task's program break
pub fn change_program_brk(size: i32) -> Option<usize> {
    TASK_MANAGER.change_current_program_brk(size)
}
/// get the pointer of the current pagetable
pub fn current_pagetable() -> *mut PageTable {
    TASK_MANAGER.current_pagetable()
}

/// modify the syscall count of the current task
pub fn modify_cnt(syscall_id: usize) {
    TASK_MANAGER.modify_cnt(syscall_id);
}

/// get the syscall count of the current task
pub fn get_count(syscall_id: usize) -> isize{
    TASK_MANAGER.get_count(syscall_id)
}