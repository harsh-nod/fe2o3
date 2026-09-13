// The pinned closed recursive printers (builtin FunctionType and GPU
// PointerType, SliceType, FixedVectorTypeV12) emit '<' before formatting any
// child TypeHandle. FunctionType's TypeSig prints ' -> ', not a closing pair.
// PipelineType and RankedViewType have one pair but no child types; all other
// admitted types are leaves. A delimiter-bearing leaf at structural depth 64
// therefore needs 65 open pairs. This guard bounds formatting recursion only;
// the typed walk still checks exact depth, node counts, and semantic admission.
const MAX_TYPE_RENDER_DELIMITERS_V2: usize = MAX_PLIRON_IDENTITY_TYPE_NESTING_V1 + 1;

#[derive(Default)]
struct TypeRenderNestingV2 {
    depth: usize,
    previous_byte: u8,
    exceeded: bool,
}

impl TypeRenderNestingV2 {
    fn observe(&mut self, value: &str) -> fmt::Result {
        for byte in value.bytes() {
            if byte == b'<' {
                if self.depth == MAX_TYPE_RENDER_DELIMITERS_V2 {
                    self.exceeded = true;
                    return Err(fmt::Error);
                }
                self.depth += 1;
            } else if byte == b'>' && self.previous_byte != b'-' {
                self.depth = self.depth.saturating_sub(1);
            }
            self.previous_byte = byte;
        }
        Ok(())
    }
}

fn render_type_preflight_bounded_v2(
    location: PlironPreserveLocationV1,
    render: impl FnOnce(&mut LimitedTextV1) -> fmt::Result,
) -> Result<String, PlironIrIdentityErrorV1> {
    render_with_bounded_writer_v1(
        location,
        "type preflight",
        LimitedTextV1 {
            type_nesting: Some(TypeRenderNestingV2::default()),
            ..LimitedTextV1::default()
        },
        render,
    )
}

#[cfg(test)]
mod type_rendering_v2_tests {
    use super::*;

    #[test]
    fn typed_data_identity_rejects_malformed_fixed_vector_descriptor() {
        let mut context = setup();
        dialect_gpu::register_dialect(&mut context).unwrap();
        let element: TypeHandle = FP32Type::get(&context).into();
        let vector: TypeHandle = dialect_gpu::vector_v12::FixedVectorTypeV12::get(
            &context,
            element,
            dialect_gpu::vector_v12::VectorLaneCountAttrV12(1),
            dialect_gpu::vector_v12::VectorLayoutAttrV12(0),
        )
        .into();
        assert!(matches!(
            validate_and_count_type_handle_v1(&context, vector, PlironPreserveLocationV1::Function),
            Err(PlironIrIdentityErrorV1::UnsupportedType { .. })
        ));
    }

    fn setup() -> Context {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        context
    }

    fn nested_function(context: &Context, depth: usize, leaf: TypeHandle) -> TypeHandle {
        (0..depth).fold(leaf, |child, _| {
            FunctionType::get(context, vec![], vec![child]).into()
        })
    }

    #[test]
    fn type_rendering_preserves_exact_limit_with_delimited_leaf() {
        let context = setup();
        let leaf = PipelineType::new(&context, 3, 2).unwrap().into();
        let ty = nested_function(&context, MAX_PLIRON_IDENTITY_TYPE_NESTING_V1, leaf);
        let plain = render_bounded(PlironPreserveLocationV1::Function, "test", |writer| {
            write!(writer, "{}", ty.disp(&context))
        })
        .unwrap();
        let guarded =
            render_type_preflight_bounded_v2(PlironPreserveLocationV1::Function, |writer| {
                write!(writer, "{}", ty.disp(&context))
            })
            .unwrap();
        assert_eq!(guarded.as_bytes(), plain.as_bytes());
        assert_eq!(
            validate_and_count_type_handle_v1(&context, ty, PlironPreserveLocationV1::Function)
                .unwrap(),
            MAX_PLIRON_IDENTITY_TYPE_NESTING_V1 + 1
        );
    }

    #[test]
    fn type_rendering_stops_one_over_before_descending() {
        let context = setup();
        let leaf = PipelineType::new(&context, 3, 2).unwrap().into();
        let ty = nested_function(&context, MAX_PLIRON_IDENTITY_TYPE_NESTING_V1 + 1, leaf);
        assert!(matches!(
            validate_and_count_type_handle_v1(&context, ty, PlironPreserveLocationV1::Function),
            Err(PlironIrIdentityErrorV1::ResourceLimitExceeded {
                resource: "type rendering nesting",
                actual,
                limit,
                ..
            }) if actual == MAX_TYPE_RENDER_DELIMITERS_V2 + 1
                && limit == MAX_TYPE_RENDER_DELIMITERS_V2
        ));
    }

    #[test]
    fn type_rendering_chunk_boundaries_preserve_function_arrows() {
        let mut writer = LimitedTextV1 {
            type_nesting: Some(TypeRenderNestingV2::default()),
            ..LimitedTextV1::default()
        };
        for _ in 0..MAX_TYPE_RENDER_DELIMITERS_V2 {
            writer.write_str("builtin.function <() -").unwrap();
            writer.write_str("> (").unwrap();
        }
        assert_eq!(
            writer.type_nesting.as_ref().unwrap().depth,
            MAX_TYPE_RENDER_DELIMITERS_V2
        );
        let before = writer.text.clone();
        assert!(writer.write_str("<").is_err());
        assert_eq!(writer.text, before);
        assert!(writer.type_nesting.as_ref().unwrap().exceeded);
        let unguarded = render_bounded(PlironPreserveLocationV1::Function, "attribute", |writer| {
            writer.write_str(&"<".repeat(MAX_TYPE_RENDER_DELIMITERS_V2 + 1))
        })
        .unwrap();
        assert_eq!(unguarded.len(), MAX_TYPE_RENDER_DELIMITERS_V2 + 1);
    }

    #[test]
    fn type_rendering_gpu_spines_share_the_function_depth_guard() {
        let mut context = setup();
        dialect_gpu::register_dialect(&mut context).unwrap();
        let scalar = FP32Type::get(&context).into();
        let vector = dialect_gpu::vector_v12::FixedVectorTypeV12::try_get(
            &context,
            scalar,
            4,
            dialect_gpu::vector_v12::VectorLayoutAttrV12::CONTIGUOUS,
        )
        .unwrap()
        .into();
        let mut data = vector;
        for ordinal in 0..MAX_PLIRON_IDENTITY_TYPE_NESTING_V1 - 2 {
            data = if ordinal % 2 == 0 {
                dialect_gpu::optimization_v1::PointerType::get(
                    &context,
                    data,
                    dialect_gpu::AddressSpaceAttr::Global,
                    dialect_gpu::optimization_v1::AccessModeAttr::ReadOnly,
                )
                .into()
            } else {
                dialect_gpu::optimization_v1::SliceType::get(
                    &context,
                    data,
                    dialect_gpu::AddressSpaceAttr::Global,
                    dialect_gpu::optimization_v1::AccessModeAttr::ReadOnly,
                )
                .into()
            };
        }
        let ty = nested_function(&context, 1, data);
        assert_eq!(
            validate_and_count_type_handle_v1(&context, ty, PlironPreserveLocationV1::Function)
                .unwrap(),
            MAX_PLIRON_IDENTITY_TYPE_NESTING_V1 + 1
        );
        let over = nested_function(&context, 3, data);
        assert!(matches!(
            validate_and_count_type_handle_v1(&context, over, PlironPreserveLocationV1::Function),
            Err(PlironIrIdentityErrorV1::ResourceLimitExceeded {
                resource: "type rendering nesting",
                ..
            })
        ));
    }

    #[test]
    fn type_rendering_chunked_pairs_preserve_bytes_and_return_to_zero() {
        let expected =
            "builtin.function <() -> (gpu.pointer <builtin.integer ui32,Global,ReadOnly>)>";
        let mut writer = LimitedTextV1 {
            type_nesting: Some(TypeRenderNestingV2::default()),
            ..LimitedTextV1::default()
        };
        for byte in expected.as_bytes().chunks(1) {
            writer
                .write_str(std::str::from_utf8(byte).unwrap())
                .unwrap();
        }
        assert_eq!(writer.text.as_bytes(), expected.as_bytes());
        assert_eq!(writer.type_nesting.as_ref().unwrap().depth, 0);
        assert!(!writer.type_nesting.as_ref().unwrap().exceeded);
    }

    #[test]
    fn type_rendering_guard_is_not_a_structural_depth_substitute() {
        let context = setup();
        let leaf = UnitType::get(&context).into();
        let ty = nested_function(&context, MAX_PLIRON_IDENTITY_TYPE_NESTING_V1 + 1, leaf);
        assert!(matches!(
            validate_and_count_type_handle_v1(&context, ty, PlironPreserveLocationV1::Function),
            Err(PlironIrIdentityErrorV1::ResourceLimitExceeded {
                resource: "type nesting depth",
                actual,
                limit,
                ..
            }) if actual == limit + 1 && limit == MAX_PLIRON_IDENTITY_TYPE_NESTING_V1
        ));
    }
}
