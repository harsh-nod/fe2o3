macro_rules! checked_u32_prefix_assemble_body_v1 {
    ($exec:ident, $types:ident, $locals:ident, $prefix:ident, $spans:ident,
     $root:ident, $function:ident, $block:ident, $kernel_block:ident, $operation:ident,
     $selected:ident, $index:ident, [$($selection_invariants:tt)*], [$($selection_proof:tt)*],
     $steps:ident, $next:ident, $ordinal:ident, [$($walk_invariants:tt)*], [$($walk_proof:tt)*]) => {
        $exec!({
            if $prefix.is_empty() || $prefix.len() > 256 {
                return Err(CheckedU32PrefixErrorV1::Capacity);
            }
            let mut $selected: Vec<Option<usize>> = vec![None; $prefix.len()];
            let mut $index = 0usize;
            while $index < $spans.len()
                $($selection_invariants)*
            {
                $($selection_proof)*
                let span = $spans[$index];
                let ordinal = span.statement_ordinal() as usize;
                if span.correspondence_owner().index() == $root
                    && span.semantic_function().index() == $function
                    && span.semantic_block().index() == $block
                    && ordinal < $prefix.len()
                {
                    if $selected[ordinal].is_some() {
                        return Err(CheckedU32PrefixErrorV1::Span);
                    }
                    $selected[ordinal] = Some($index);
                }
                $index += 1;
            }
            let mut $steps = Vec::with_capacity($prefix.len() - 1);
            let mut $next = 0u32;
            let mut $ordinal = 0usize;
            while $ordinal < $prefix.len()
                $($walk_invariants)*
            {
                $($walk_proof)*
                let Some(index) = $selected[$ordinal] else {
                    return Err(CheckedU32PrefixErrorV1::Span);
                };
                let span = $spans[index];
                if span.kernel_ir_block().0 != $kernel_block
                    || span.first_operation_ordinal() != $next
                {
                    return Err(CheckedU32PrefixErrorV1::Span);
                }
                let Some(end) = $next.checked_add(span.operation_count()) else {
                    return Err(CheckedU32PrefixErrorV1::Span);
                };
                $next = end;
                if $ordinal + 1 < $prefix.len() {
                    if let Some(step) = source_step(
                        $types, $locals, &$prefix[$ordinal], span.operation_count(),
                    )? {
                        $steps.push(step);
                    }
                } else if span.operation_count() != 2
                    || span.first_operation_ordinal().checked_add(1) != Some($operation)
                {
                    return Err(CheckedU32PrefixErrorV1::Span);
                }
                $ordinal += 1;
            }
            Ok(($steps, $next))
        })
    };
}
