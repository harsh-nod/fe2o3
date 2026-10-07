// Immutable metadata preparation only. Outer resource validation, allocation
// custody, roster hashing, and epoch reservation are separate operations.
#[allow(unused_macros)]
macro_rules! dispatch_template_abi_matches_body {
    ($syntax:ident, $bound:ident, $layout:ident, $abi:ident) => {
        dispatch_template_abi_matches_body!(@annotated $syntax, $bound, $layout,
            $abi, difference, index, [], [], [], [])
    };
    (@annotated $syntax:ident, $bound:ident, $layout:ident, $abi:ident,
     $difference:ident, $index:ident, [$($invariants:tt)*], [$($before:tt)*],
     [$($step:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            if !$bound {
                return true;
            }
            let mut $difference = 0u8;
            let mut $index = 0;
            while $index < 32 $($invariants)* {
                $($before)*
                $difference |= $layout[$index] ^ $abi[$index];
                $($step)*
                $index += 1;
            }
            $($finish)*
            $difference == 0
        })
    };
}

#[allow(unused_macros)]
macro_rules! dispatch_template_generation_new_body {
    ($syntax:ident, $queue:ident, $code:ident, $kernarg:ident, $generation:ident) => {
        $syntax!({
            Self {
                queue: $queue,
                code: $code,
                kernarg: $kernarg,
                dispatch_generation: $generation,
            }
        })
    };
}

#[allow(unused_macros)]
macro_rules! dispatch_template_new_body {
    ($syntax:ident, $geometry:ident, $ordering:ident, $private:ident, $group:ident,
     $kernel:ident, $kernarg:ident, $alignment:ident, $generations:ident) => {
        $syntax!({
            Self {
                geometry: $geometry,
                ordering: $ordering,
                private_segment_size: $private,
                group_segment_size: $group,
                kernel_object: $kernel,
                kernarg_address: $kernarg,
                kernarg_alignment: $alignment,
                generations: $generations,
            }
        })
    };
}

#[allow(unused_macros)]
macro_rules! dispatch_prepare_templates_body {
    ($syntax:ident, $packets:ident, $codes:ident, $queue:ident, $generation:ident) => {
        dispatch_prepare_templates_body!(@annotated $syntax, $packets, $codes,
            $queue, $generation, templates, index, [], [], [])
    };
    (@annotated $syntax:ident, $packets:ident, $codes:ident, $queue:ident,
     $generation:ident, $templates:ident, $index:ident, [$($invariants:tt)*],
     [$($before:tt)*], [$($step:tt)*]) => {
        $syntax!({
            let mut $templates = Vec::<CompletionPacketTemplateV1>::new();
            let mut $index = 0;
            while $index < $packets.len() $($invariants)* {
                $($before)*
                let packet = &$packets[$index];
                if packet.code_index >= $codes.len() {
                    return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
                        "packet program index",
                    ));
                }
                let code = &$codes[packet.code_index];
                if !prepared_kernarg_layout_matches_code(
                    packet.code_bound_kernarg_layout,
                    packet.kernarg_layout_identity,
                    code.dispatch_abi_identity,
                ) {
                    return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                        packet: $index,
                        detail: "prepared kernarg dispatch ABI identity",
                    });
                }
                $templates.push(CompletionPacketTemplateV1::new(
                    packet.geometry,
                    packet.ordering,
                    packet.private_segment_size,
                    packet.group_segment_size,
                    code.descriptor_address,
                    packet.kernarg_address,
                    packet.kernarg_alignment,
                    CompletionDispatchGenerationBindingV1::new(
                        $queue,
                        code.mapping,
                        packet.kernarg_mapping,
                        $generation,
                    ),
                ));
                $($step)*
                $index += 1;
            }
            Ok($templates)
        })
    };
}
