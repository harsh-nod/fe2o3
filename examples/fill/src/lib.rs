#![no_std]

use fe2o3_device::{DisjointWrite, Global, Index1D, KernelContext, kernel};

#[kernel(
    typed,
    reference = fill_point_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
pub fn fill(context: KernelContext<'_>, mut out: Global<'_, f32, DisjointWrite<Index1D>>) {
    let index = context.invocation().index_1d();
    let _stored = out.store(index.into_disjoint(), 42.5);
}

fn fill_point_reference(_point: usize, output: &mut f32) {
    *output = 42.5;
}

#[cfg(not(target_arch = "amdgpu"))]
pub fn fill_cpu_reference(output: &mut [f32], launched: usize) {
    for (point, value) in output.iter_mut().take(launched).enumerate() {
        fill_point_reference(point, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_reference_is_independent_of_the_invocation_coordinate() {
        for point in [0, 1, 63, 64, 65, usize::MAX] {
            let mut output = f32::from_bits(0x7fc0_0123);
            fill_point_reference(point, &mut output);
            assert_eq!(output.to_bits(), 42.5_f32.to_bits());
        }
    }

    #[test]
    fn batch_reference_preserves_every_unlaunched_output_bit() {
        let untouched = f32::from_bits(0x7fc0_0123);
        for length in [0, 1, 63, 64, 65] {
            for launched in [0, 1, 63, 64, 65, usize::MAX] {
                let mut output = [untouched; 65];
                fill_cpu_reference(&mut output[..length], launched);
                for (point, actual) in output.iter().enumerate() {
                    let expected = if point < length.min(launched) {
                        42.5
                    } else {
                        untouched
                    };
                    assert_eq!(
                        actual.to_bits(),
                        expected.to_bits(),
                        "length={length}, launched={launched}, point={point}"
                    );
                }
            }
        }
    }
}
