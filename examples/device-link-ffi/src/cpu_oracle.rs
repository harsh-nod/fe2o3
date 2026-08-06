/// Independent CPU specification for the bidirectional fixture.
///
/// The GPU path computes `rust_accumulate(external_affine(value), lane)`.
/// This oracle intentionally does not call either boundary emulation.
pub fn evaluate(input: &[u32]) -> Vec<u32> {
    let mut lane = 0_u32;
    input
        .iter()
        .map(|value| {
            let output = value.wrapping_mul(3).wrapping_add(5).wrapping_add(lane);
            lane = lane.wrapping_add(1);
            output
        })
        .collect()
}
