#![no_main]
#![no_std]

use core::arch::asm;
use core::panic::PanicInfo;

extern "C" {
    fn fe2o3_rust_rlib_symbol(value: usize) -> usize;
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let status = unsafe { fe2o3_rust_rlib_symbol(41) != 42 } as usize;
    unsafe {
        asm!(
            "syscall",
            in("rax") 60usize,
            in("rdi") status,
            options(noreturn, nostack)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {}
}
