#![allow(dead_code)]

use axerrno::LinuxError;
use axhal::arch::TrapFrame;
use axhal::mem::VirtAddr;
use axhal::paging::MappingFlags;
use axhal::trap::{register_trap_handler, PAGE_FAULT, SYSCALL};
use axtask::*;
const SYS_EXIT: usize = 93;

#[register_trap_handler(SYSCALL)]
fn handle_syscall(tf: &TrapFrame, syscall_num: usize) -> isize {
    ax_println!("handle_syscall ...");
    let ret = match syscall_num {
        SYS_EXIT => {
            ax_println!("[SYS_EXIT]: process is exiting ..");
            axtask::exit(tf.arg0() as _)
        }
        _ => {
            ax_println!("Unimplemented syscall: {}", syscall_num);
            -LinuxError::ENOSYS.code() as _
        }
    };
    ret
}

#[register_trap_handler(PAGE_FAULT)]
fn handle_page_fault(addr: VirtAddr, flags: MappingFlags, from_user: bool) -> bool {
    ax_println!(
        "[PAGE_FAULT] addr = {:#x}, flags = {:?}, from_user = {}",
        addr,
        flags,
        from_user
    );
    let current_task = current();
    let mut addr_space = current_task.task_ext().aspace.lock();
    if !addr_space.handle_page_fault(addr, flags) {
        exit(-1);
    }
    true
}
