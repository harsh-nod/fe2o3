use fe2o3_device::KernelContext;

fn main() {
    let _context: KernelContext<'static> = unsafe { KernelContext::__compiler_issue() };
}
