// These macros are the single executable bodies shared by ordinary Rust and
// the Verus source models in `verus/elementwise.rs`.
macro_rules! copy_kernel_body {
    ($thread:ident, $source_index:path, $input:ident, $output:ident, $mismatch:path) => {{
        if $thread >= $output.len() || $input.len() != $output.len() {
            Err($mismatch)
        } else {
            let source = $source_index($thread);
            if source >= $input.len() {
                Err($mismatch)
            } else {
                $output[$thread] = $input[source];
                Ok(())
            }
        }
    }};
}

macro_rules! affine_map_kernel_body {
    (
        $thread:ident,
        $affine:path,
        $input:ident,
        $output:ident,
        $scale:ident,
        $bias:ident,
        $mismatch:path
    ) => {{
        if $thread >= $output.len() || $input.len() != $output.len() {
            Err($mismatch)
        } else {
            $output[$thread] = $affine($input[$thread], $scale, $bias);
            Ok(())
        }
    }};
}

macro_rules! gather_kernel_body {
    (
        $thread:ident,
        $gather_index:path,
        $input:ident,
        $indices:ident,
        $output:ident,
        $mismatch:path,
        $out_of_bounds:path
    ) => {{
        if $thread >= $output.len() || $indices.len() != $output.len() {
            Err($mismatch)
        } else {
            let source = $gather_index($indices, $thread);
            if source >= $input.len() {
                Err($out_of_bounds)
            } else {
                $output[$thread] = $input[source];
                Ok(())
            }
        }
    }};
}
