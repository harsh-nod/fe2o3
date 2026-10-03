macro_rules! checked_u32_prefix_basis_body_v1 {
    ($exec:ident, $arguments:ident, $source:ident, $kernel:ident,
     $clear:ident, [$($clear_invariants:tt)*], $index:ident, [$($invariants:tt)*]) => {
        $exec!({
            if $kernel.len() != $arguments.len() {
                return false;
            }
            let mut $clear = 0usize;
            while $clear < $source.len()
                $($clear_invariants)*
            {
                $source[$clear] = Origin::Uninitialized;
                $clear += 1;
            }
            let mut $index = 0usize;
            while $index < $arguments.len()
                $($invariants)*
            {
                let binding = $arguments[$index];
                let local = binding.semantic_local as usize;
                if binding.argument != $index || local >= $source.len() {
                    return false;
                }
                if !matches!($source[local], Origin::Uninitialized) {
                    return false;
                }
                $source[local] = Origin::Argument($index);
                $kernel[$index] = Origin::Argument($index);
                $index += 1;
            }
            true
        })
    };
}
