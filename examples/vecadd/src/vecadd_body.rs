// This macro is the single executable body of the real f32 GPU vecadd kernel.
// The GPU kernel and the Verus source model both expand these tokens. The
// thread adapter is explicit because the Verus harness cannot execute the
// target intrinsic; every index operation after that adapter is shared.
macro_rules! vecadd_kernel_body {
    ($thread:ident, ($($thread_arg:expr),* $(,)?), $a:ident, $b:ident, $output:ident) => {{
        let idx = $thread::index_1d($($thread_arg),*);
        let i = idx.get();
        if let Some(out) = $output.get_mut(idx) {
            *out = $a[i] + $b[i];
        }
    }};
}
