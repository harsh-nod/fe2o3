#![forbid(unsafe_code)]

/// Sequential reference for an arbitrary per-element transform.
pub fn map_reference<T: Copy, U>(input: &[T], output: &mut [U], mut map: impl FnMut(T) -> U) {
    assert_eq!(input.len(), output.len());
    for (source, destination) in input.iter().copied().zip(output.iter_mut()) {
        *destination = map(source);
    }
}

/// Sequential reference for an arbitrary two-input elementwise operation.
pub fn zip_reference<T: Copy, U: Copy, V>(
    left: &[T],
    right: &[U],
    output: &mut [V],
    mut combine: impl FnMut(T, U) -> V,
) {
    assert_eq!(left.len(), right.len());
    assert_eq!(left.len(), output.len());
    for ((left, right), destination) in left
        .iter()
        .copied()
        .zip(right.iter().copied())
        .zip(output.iter_mut())
    {
        *destination = combine(left, right);
    }
}

pub fn fill_reference(output: &mut [f32], value: f32) {
    output.fill(value);
}

pub fn vecadd_reference(left: &[f32], right: &[f32], output: &mut [f32]) {
    zip_reference(left, right, output, |left, right| left + right);
}
