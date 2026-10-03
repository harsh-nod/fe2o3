macro_rules! checked_u32_prefix_fold_body_v1 {
    ($exec:ident, $state:ident, $steps:ident, $index:ident, [$($invariants:tt)*]) => {
        $exec!({
            let mut $index = 0usize;
            while $index < $steps.len()
                $($invariants)*
            {
                let step = $steps[$index];
                if step.destination >= $state.len() {
                    return false;
                }
                let origin = match step.input {
                    PrefixInput::Constant(value) => Origin::Constant(value),
                    PrefixInput::Cell(source) => {
                        if source >= $state.len() {
                            return false;
                        }
                        $state[source]
                    }
                };
                if matches!(origin, Origin::Uninitialized) {
                    return false;
                }
                $state[step.destination] = origin;
                $index += 1;
            }
            true
        })
    };
}
