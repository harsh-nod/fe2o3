// The pinned closed recursive printers (builtin FunctionType and GPU
// PointerType, SliceType, FixedVectorTypeV12) emit '<' before formatting any
// child TypeHandle. FunctionType's TypeSig prints ' -> ', not a closing pair.
// PipelineType, RankedViewType and ExecutionRoleTypeV18 have one pair but no
// child types. StorageObjectTypeV18 has two pairs, including its fixed-field
// table key attribute. A storage leaf at structural depth 64 therefore needs
// 66 open pairs. This guard bounds formatting recursion only;
// the typed walk still checks exact depth, node counts, and semantic admission.
const MAX_TYPE_RENDER_DELIMITERS_V2: usize = MAX_PLIRON_IDENTITY_TYPE_NESTING_V1 + 2;

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

#[cfg(test)]
fn render_type_preflight_bounded_v2<'a, 'p: 'a, 'r: 'p>(
    location: PlironPreserveLocationV1,
    render: impl FnOnce(&mut LimitedTextV1<'a, 'p, 'r>) -> fmt::Result,
) -> Result<String, PlironIrIdentityErrorV1> {
    render_type_preflight_bounded_observed_v2(location, None, render)
}

fn render_type_preflight_bounded_observed_v2<'a, 'p, 'r>(
    location: PlironPreserveLocationV1,
    observer: RenderObserverV1<'a, 'p, 'r>,
    render: impl FnOnce(&mut LimitedTextV1<'a, 'p, 'r>) -> fmt::Result,
) -> Result<String, PlironIrIdentityErrorV1> {
    render_with_bounded_writer_v1(
        location,
        "type preflight",
        LimitedTextV1 {
            type_nesting: Some(TypeRenderNestingV2::default()),
            observer,
            ..LimitedTextV1::default()
        },
        render,
    )
}

#[cfg(test)]
mod type_rendering_v2_tests {
    use super::*;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1, InvocationReceiptV1,
    };
    use dialect_gpu::storage_types_v18::{
        ExecutionRoleTypeV18, StorageObjectTypeV18, StorageOrdinalAttrV18, StorageTableKeyAttrV18,
    };

    fn rendering_receipt() -> (
        InvocationReceiptV1<'static>,
        ProductionAnalysisResourceUpperBoundV1,
    ) {
        let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
        let floor =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 17, 64, 32).unwrap();
        let reservation = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            phase,
            4 * MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1 + 128,
            0,
            4 * MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1 + 128,
        )
        .unwrap();
        (
            InvocationReceiptV1::new(
                floor,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            )
            .unwrap(),
            reservation,
        )
    }

    #[test]
    fn observed_renderer_denial_survives_swallow_and_later_panic() {
        let phase_kind = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
        for nesting in [false, true] {
            for behavior in 0..3 {
                let (mut receipt, reservation) = rendering_receipt();
                let phase = receipt.phase(phase_kind, 0).unwrap();
                let observer = phase.observer(&Ok);
                observer
                    .require(
                        ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
                        phase_kind,
                        Ok(reservation),
                    )
                    .unwrap();
                let print = |writer: &mut LimitedTextV1<'_, '_, '_>| {
                    let error = if nesting {
                        writer.write_str(&"<".repeat(MAX_TYPE_RENDER_DELIMITERS_V2 + 1))
                    } else {
                        writer.write_str(&"x".repeat(MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1 + 1))
                    };
                    assert!(error.is_err());
                    match behavior {
                        0 => error,
                        1 => Ok(()),
                        _ => panic!("printer panic after quota"),
                    }
                };
                let result = if nesting {
                    render_type_preflight_bounded_observed_v2(
                        PlironPreserveLocationV1::Function,
                        Some(&observer),
                        print,
                    )
                } else {
                    render_bounded_observed_v1(
                        PlironPreserveLocationV1::Function,
                        "test",
                        Some(&observer),
                        print,
                    )
                };
                if behavior == 1 {
                    assert_eq!(result.unwrap(), "");
                } else if behavior == 2 {
                    assert!(matches!(
                        result,
                        Err(PlironIrIdentityErrorV1::RenderingFailed { .. })
                    ));
                } else {
                    assert!(matches!(
                        result,
                        Err(PlironIrIdentityErrorV1::ResourceLimitExceeded { .. })
                    ));
                }
                phase.commit(reservation).unwrap();
                let expected = crate::production_analysis::ProductionAnalysisResourceLimitV1 {
                    phase: phase_kind,
                    resource: if nesting {
                        "type rendering nesting"
                    } else {
                        "rendered entity bytes"
                    },
                };
                assert_eq!(receipt.snapshot().first_denial, Some(expected));
                assert_eq!(receipt.snapshot().caught_panic, behavior == 2);
                assert_eq!(
                    receipt.complete(),
                    Err(InvocationReceiptFailureV1::Denied(expected))
                );
                assert_eq!(
                    receipt.snapshot().committed.work_upper_bound(),
                    reservation.work_upper_bound()
                );
            }
        }
    }

    #[test]
    fn observed_renderer_distinguishes_plain_failure_and_panic() {
        let phase_kind = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
        for panic in [false, true] {
            let (mut receipt, reservation) = rendering_receipt();
            let phase = receipt.phase(phase_kind, 0).unwrap();
            let observer = phase.observer(&Ok);
            observer
                .require(
                    ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
                    phase_kind,
                    Ok(reservation),
                )
                .unwrap();
            let result = render_bounded_observed_v1(
                PlironPreserveLocationV1::Function,
                "test",
                Some(&observer),
                |_| {
                    assert!(!panic, "plain printer panic");
                    Err(fmt::Error)
                },
            );
            assert!(matches!(
                result,
                Err(PlironIrIdentityErrorV1::RenderingFailed { .. })
            ));
            phase.commit(reservation).unwrap();
            assert_eq!(receipt.snapshot().first_denial, None);
            assert_eq!(receipt.snapshot().caught_panic, panic);
            if panic {
                assert_eq!(
                    receipt.complete(),
                    Err(InvocationReceiptFailureV1::CaughtPanic)
                );
            } else {
                assert_eq!(receipt.complete(), Ok(reservation));
            }
        }
    }

    #[test]
    fn observed_renderer_does_not_classify_diagnostic_truncation_as_denial() {
        let phase_kind = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
        let (mut receipt, reservation) = rendering_receipt();
        let phase = receipt.phase(phase_kind, 0).unwrap();
        phase
            .observer(&Ok)
            .require(
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
                phase_kind,
                Ok(reservation),
            )
            .unwrap();
        let mut summary = DiagnosticSummaryV1::default();
        summary.append(format_args!(
            "{}",
            "x".repeat(MAX_DIAGNOSTIC_DETAIL_CHARS_V1 + 1)
        ));
        assert!(summary.finish().ends_with("..."));
        phase.commit(reservation).unwrap();
        assert_eq!(receipt.complete(), Ok(reservation));
    }

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

    fn storage_leaf_v18(context: &Context, digest: [u8; 32], length: u64, row: u32) -> TypeHandle {
        StorageObjectTypeV18::get(
            context,
            StorageTableKeyAttrV18::new(digest, length),
            StorageOrdinalAttrV18(row),
        )
        .into()
    }

    fn identity_for_v18_leaf(
        leaf: impl FnOnce(&Context) -> TypeHandle,
        wrappers: usize,
    ) -> PlironIrStructuralIdentityV1 {
        let mut context = setup();
        dialect_gpu::register_dialect(&mut context).unwrap();
        let mut ty = leaf(&context);
        for depth in 0..wrappers {
            ty = if depth % 2 == 0 {
                dialect_gpu::optimization_v1::PointerType::get(
                    &context,
                    ty,
                    dialect_gpu::AddressSpaceAttr::Private,
                    dialect_gpu::optimization_v1::AccessModeAttr::ReadWrite,
                )
                .into()
            } else {
                dialect_gpu::optimization_v1::SliceType::get(
                    &context,
                    ty,
                    dialect_gpu::AddressSpaceAttr::Private,
                    dialect_gpu::optimization_v1::AccessModeAttr::ReadOnly,
                )
                .into()
            };
        }
        let signature = FunctionType::get(&context, vec![ty], vec![]);
        let function = FuncOp::new(&mut context, "identity_v18".try_into().unwrap(), signature);
        dialect_kernel::ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(function.get_entry_block(&context), &context);
        let identity = derive_pliron_ir_structural_identity_v1(&context, &function).unwrap();
        assert!(!identity.grants_operational_semantics_or_refinement_authority());
        identity
    }

    #[test]
    fn v18_storage_identity_keeps_every_key_byte_length_row_and_wrapper() {
        for wrappers in [0, 1, 3] {
            let baseline =
                identity_for_v18_leaf(|context| storage_leaf_v18(context, [0; 32], 4, 0), wrappers);
            let independent =
                identity_for_v18_leaf(|context| storage_leaf_v18(context, [0; 32], 4, 0), wrappers);
            assert!(baseline.exactly_matches(&independent));
            for byte in 0..32 {
                let mut digest = [0; 32];
                digest[byte] = 1;
                let changed = identity_for_v18_leaf(
                    |context| storage_leaf_v18(context, digest, 4, 0),
                    wrappers,
                );
                assert!(!baseline.exactly_matches(&changed), "key byte {byte}");
            }
            for (length, row) in [(5, 0), (u64::MAX, 0), (4, 1), (4, u32::MAX)] {
                let changed = identity_for_v18_leaf(
                    |context| storage_leaf_v18(context, [0; 32], length, row),
                    wrappers,
                );
                assert!(!baseline.exactly_matches(&changed));
            }
            let changed = identity_for_v18_leaf(
                |context| storage_leaf_v18(context, [0; 32], 4, 0),
                wrappers + 1,
            );
            assert!(!baseline.exactly_matches(&changed));
        }
    }

    #[test]
    fn v18_execution_identity_keeps_role_and_geometry_without_owner_authority() {
        let mut identities = Vec::new();
        for (role, lanes, elements) in [
            (1, 0, 0),
            (2, 0, 0),
            (3, 1, 1),
            (4, 1, 1),
            (3, 2, 1),
            (3, 1, 2),
            (3, 256, 125),
        ] {
            let make = |context: &Context| {
                ExecutionRoleTypeV18::get(
                    context,
                    StorageOrdinalAttrV18(role),
                    StorageOrdinalAttrV18(lanes),
                    StorageOrdinalAttrV18(elements),
                )
                .into()
            };
            let identity = identity_for_v18_leaf(make, 0);
            assert!(identity.exactly_matches(&identity_for_v18_leaf(make, 0)));
            for earlier in &identities {
                assert!(!identity.exactly_matches(earlier));
            }
            identities.push(identity);
        }
    }

    #[test]
    fn v18_identity_refuses_malformed_storage_and_execution_leaves_under_pointers() {
        let mut context = setup();
        dialect_gpu::register_dialect(&mut context).unwrap();
        let mut invalid = (0..4)
            .map(|length| storage_leaf_v18(&context, [0; 32], length, 0))
            .collect::<Vec<_>>();
        for (role, lanes, elements) in [
            (0, 0, 0),
            (5, 1, 1),
            (1, 1, 0),
            (2, 0, 1),
            (3, 0, 1),
            (3, 257, 1),
            (4, 1, 0),
            (4, 1, 126),
        ] {
            invalid.push(
                ExecutionRoleTypeV18::get(
                    &context,
                    StorageOrdinalAttrV18(role),
                    StorageOrdinalAttrV18(lanes),
                    StorageOrdinalAttrV18(elements),
                )
                .into(),
            );
        }
        for leaf in invalid {
            let pointer = dialect_gpu::optimization_v1::PointerType::get(
                &context,
                leaf,
                dialect_gpu::AddressSpaceAttr::Private,
                dialect_gpu::optimization_v1::AccessModeAttr::ReadWrite,
            )
            .into();
            for ty in [leaf, pointer] {
                assert!(matches!(
                    validate_and_count_type_handle_v1(
                        &context,
                        ty,
                        PlironPreserveLocationV1::Function
                    ),
                    Err(PlironIrIdentityErrorV1::UnsupportedType { .. })
                ));
            }
        }
    }

    #[test]
    fn v18_structural_names_do_not_extend_the_legacy_bridge_type_profile() {
        let mut context = setup();
        dialect_gpu::register_dialect(&mut context).unwrap();
        let object = storage_leaf_v18(&context, [0; 32], 4, 0);
        let role: TypeHandle = ExecutionRoleTypeV18::get(
            &context,
            StorageOrdinalAttrV18(1),
            StorageOrdinalAttrV18(0),
            StorageOrdinalAttrV18(0),
        )
        .into();
        for ty in [object, role] {
            assert_eq!(validate_and_count_type_handle_v1(
                &context, ty, PlironPreserveLocationV1::Function,
            ).unwrap(), 1);
            assert!(
                !crate::kir_bridge_v1::ranked_data_type_node_is_supported_v2(&*ty.deref(&context))
            );
        }
    }

    #[test]
    fn v18_storage_rendering_preserves_exact_depth_and_refuses_one_over() {
        let mut context = setup();
        dialect_gpu::register_dialect(&mut context).unwrap();
        let leaf = storage_leaf_v18(&context, [u8::MAX; 32], u64::MAX, u32::MAX);
        let depth = MAX_PLIRON_IDENTITY_TYPE_NESTING_V1;
        let exact = nested_function(&context, depth, leaf);
        assert_eq!(
            validate_and_count_type_handle_v1(&context, exact, PlironPreserveLocationV1::Function)
                .unwrap(),
            depth + 1,
        );
        let expected = format!("{}", exact.disp(&context));
        let (_, encoded) =
            render_type_handle(&context, exact, PlironPreserveLocationV1::Function, None).unwrap();
        assert_eq!(encoded, expected);
        let over = nested_function(&context, depth + 1, leaf);
        assert!(matches!(
            validate_and_count_type_handle_v1(&context, over, PlironPreserveLocationV1::Function),
            Err(PlironIrIdentityErrorV1::ResourceLimitExceeded {
                resource: "type rendering nesting", actual, limit, ..
            }) if actual == depth + 3 && limit == depth + 2
        ));
        let ordinary = nested_function(
            &context,
            depth + 1,
            PipelineType::new(&context, 3, 2).unwrap().into(),
        );
        assert!(matches!(
            validate_and_count_type_handle_v1(&context, ordinary, PlironPreserveLocationV1::Function),
            Err(PlironIrIdentityErrorV1::ResourceLimitExceeded {
                resource: "type nesting depth", actual, limit, ..
            }) if actual == depth + 1 && limit == depth
        ));
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
    fn type_rendering_stops_two_over_single_pair_leaf_before_descending() {
        let context = setup();
        let leaf = PipelineType::new(&context, 3, 2).unwrap().into();
        let ty = nested_function(&context, MAX_PLIRON_IDENTITY_TYPE_NESTING_V1 + 2, leaf);
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
                resource: "type nesting depth",
                ..
            })
        ));
        let render_over = nested_function(&context, 4, data);
        assert!(matches!(
            validate_and_count_type_handle_v1(
                &context,
                render_over,
                PlironPreserveLocationV1::Function
            ),
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
