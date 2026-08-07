use gpu_device::kernel;

#[kernel(unsafe_asm(
    target = "gfx1100",
    operands(sgpr),
    options(nomem),
    effects(none)
))]
unsafe fn unsupported_target() {}

#[kernel(unsafe_asm(
    target = "gfx942",
    operands(sgpr),
    options(nomem),
    effects(write_global)
))]
unsafe fn contradictory_effects() {}

#[kernel(unsafe_asm(
    target = "gfx942",
    operands(sgpr),
    options(nomem),
    effects(none)
))]
fn safe_kernel_cannot_claim_assembly() {}

fn main() {}
