// Expected compile refusal; no captured diagnostic is claimed before root runs it.
use fe2o3_device::amdgpu_ordered_program;

fn candidate(a: u32, b: u32, c: u32) -> u32 {
    amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
        init { mov(out, input0); }
        repeat(0) { add(out, out, input1); }
    }
}

fn main() {}
