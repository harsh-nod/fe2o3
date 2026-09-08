pub fn through_const<T: Copy, const LANES: usize>(value: T) -> T {
    let _ = LANES;
    value
}
