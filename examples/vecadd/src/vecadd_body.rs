// This macro is the single control, index, and memory-access body of the real
// f32 GPU vecadd kernel. The capability kernel and Verus source model both
// expand these tokens; only their target-specific adapters differ.
macro_rules! vecadd_kernel_body {
    (
        @capability
        $index:ident,
        $add:ident,
        $a:ident,
        $b:ident,
        $output:ident $(,)?
    ) => {{
        let i = $index.get();
        if i < $output.len() {
            if let Some(left) = $a.load(i) {
                if let Some(right) = $b.load(i) {
                    $output.store($index.into_disjoint(), $add!(left, right))
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    }};
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
