// These are the single executable bodies shared by the ordinary Rust model
// and the Verus model in `verus/two_kernel.rs`. The output lookup deliberately
// precedes every input read so rounded-up launch threads are inert.
macro_rules! alpha_kernel_body {
    ($thread:ident, $mul:path, $scale:ident, $input:ident, $output:ident) => {{
        if let Some(out) = $output.get_mut($thread) {
            *out = $mul($scale, $input[$thread]);
        }
    }};
}

macro_rules! zeta_kernel_body {
    ($thread:ident, $add_bias:path, $a:ident, $b:ident, $bias:ident, $output:ident) => {{
        if let Some(out) = $output.get_mut($thread) {
            *out = $add_bias($a[$thread], $b[$thread], $bias);
        }
    }};
}
