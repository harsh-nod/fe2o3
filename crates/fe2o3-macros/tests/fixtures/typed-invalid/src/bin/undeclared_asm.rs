use gpu_device::kernel;

#[kernel]
unsafe fn undeclared_assembly() {
    unsafe { core::arch::asm!("nop") }
}

fn main() {}
