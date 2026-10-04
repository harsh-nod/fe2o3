// This macro is the single control, index, and memory-access body of the real
// f32 GPU vecadd kernel. The GPU kernel and Verus source model both expand these
// tokens. Thread and arithmetic adapters isolate the target-specific intrinsic
// and f32 operation without duplicating the guarded access structure.
macro_rules! vecadd_kernel_body {
    (
        $thread:ident,
        ($($thread_arg:expr),* $(,)?),
        $add:ident,
        $a:ident,
        $b:ident,
        $output:ident $(,)?
    ) => {{
        let idx = $thread::index_1d($($thread_arg),*);
        let i = idx.get();
        if let Some(out) = $output.get_mut(idx) {
            *out = $add!($a[i], $b[i]);
        }
    }};
}
