use super::*;
use std::collections::BTreeMap;

use dialect_mir::MirTypeKind;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFieldsShapeV1, SemanticMutabilityV1, SemanticPointerKindV1, SemanticRustcVariantsV1,
    SemanticTypeDeclV1, SemanticTypeIdentityV1, SemanticTypeShapeV1,
};
use fe2o3_rustc_front::StableTypeIdentityV1;
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::util::IntTypeExt;
use rustc_middle::ty::{TyKind, TypingEnv};

use crate::production_semantic_types_v1::construct_production_semantic_types_v1;
use crate::rust_type_layout_general::PointerKind;
use crate::rustc_semantic_adapter_v1::{
    canonical_target_layout_v1, rustc_semantic_layout_identity_v1, rustc_type_identity_v1,
    rustc_type_layout_sha256_v1,
};
use crate::rustc_semantic_plan_v1::RetainedSemanticTypeProducerV1;
use crate::semantic_layout_bridge::{
    SemanticLayoutBridgeError, SemanticLayoutEvidenceV1, SemanticLayoutTargetV1,
    extract_semantic_layout_evidence_v1, rustc_semantic_layout_target_v1,
};

pub(super) struct ClosureLayoutResult {
    name: &'static str,
    facts: TypeLayoutFacts,
    evidence: SemanticLayoutEvidenceV1,
    record: SemanticTypeDeclV1,
    types: Vec<SemanticTypeDeclV1>,
}

pub(super) fn extract_closures(tcx: TyCtxt<'_>) -> Vec<ClosureLayoutResult> {
    let target = rustc_semantic_layout_target_v1(tcx).unwrap();
    let canonical_target = canonical_target_layout_v1(&target);
    let layout_cx = LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
    [
        "captureless",
        "mixed_captures",
        "first_closure",
        "second_closure",
        "shared_capture",
        "mutable_capture",
        "generic_u32",
        "generic_f32",
        "optional_capture",
    ]
    .into_iter()
    .map(|name| {
        let TyKind::FnDef(definition, arguments) = *local_item_type(tcx, name).kind() else {
            panic!("expected fixture function");
        };
        let output = tcx
            .instantiate_bound_regions_with_erased(
                tcx.fn_sig(definition).instantiate(tcx, arguments),
            )
            .output();
        let ty = tcx
            .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), output)
            .unwrap();
        let identity = rustc_type_identity_v1(tcx, ty);
        let source_identity = StableTypeIdentityV1::new(*identity.as_bytes()).unwrap();
        let facts =
            extract_general_layout(tcx, ty).unwrap_or_else(|error| panic!("{name}: {error}"));
        let evidence = extract_semantic_layout_evidence_v1(tcx, ty, source_identity, &target)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(evidence.source_type_identity(), source_identity);
        assert_eq!(
            evidence.canonical_bytes(),
            extract_semantic_layout_evidence_v1(tcx, ty, source_identity, &target,)
                .unwrap()
                .canonical_bytes()
        );
        let other_target = SemanticLayoutTargetV1::new(
            "unrelated-target",
            target.data_layout(),
            target.default_pointer_width_bits(),
        )
        .unwrap();
        assert!(matches!(
            extract_semantic_layout_evidence_v1(tcx, ty, source_identity, &other_target),
            Err(SemanticLayoutBridgeError::TargetMismatch { .. })
        ));

        let mut inventory = BTreeMap::new();
        collect_fixture_types(tcx, ty, &mut inventory);
        let producers = inventory
            .into_iter()
            .map(|(identity, ty)| {
                let layout = layout_cx.layout_of(ty).unwrap();
                RetainedSemanticTypeProducerV1 {
                    identity,
                    ty,
                    layout,
                    rustc_layout_sha256: rustc_type_layout_sha256_v1(tcx, layout),
                    semantic_layout_identity: rustc_semantic_layout_identity_v1(
                        tcx,
                        canonical_target,
                        layout,
                    ),
                }
            })
            .collect::<Vec<_>>();
        let types = construct_production_semantic_types_v1(tcx, &producers)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .into_records();
        let record = types
            .iter()
            .find(|record| record.identity() == identity)
            .unwrap()
            .clone();
        if let TyKind::Closure(_, arguments) = ty.kind() {
            let TypeLayoutKind::Closure {
                identity: observed,
                fields,
            } = &facts.kind
            else {
                panic!("closure must retain its nominal identity");
            };
            assert_eq!(observed, identity.as_bytes());
            let captures = arguments.as_closure().upvar_tys();
            let layout = layout_cx.layout_of(ty).unwrap();
            let SemanticTypeShapeV1::Aggregate(aggregate) = record.shape() else {
                panic!("closure must be a nominal aggregate, including captureless closures");
            };
            let SemanticFieldsShapeV1::Arbitrary {
                source_order_offsets_bytes: offsets,
                memory_order_source_indices: memory_order,
            } = record.layout().fields()
            else {
                panic!("closure needs exact arbitrary field placement");
            };
            assert_eq!(fields.len(), captures.len());
            assert_eq!(aggregate.fields().len(), captures.len());
            for (index, capture) in captures.iter().enumerate() {
                let capture = tcx
                    .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), capture)
                    .unwrap();
                assert_eq!(layout.field(&layout_cx, index).ty, capture);
                assert_eq!(fields[index].source_index, index);
                assert_eq!(fields[index].name, Some(index.to_string()));
                assert_eq!(
                    fields[index].offset_bytes,
                    layout.fields.offset(index).bytes()
                );
                assert_eq!(offsets[index], fields[index].offset_bytes);
                assert_eq!(memory_order[fields[index].memory_index], index as u32);
                assert_eq!(
                    types[aggregate.fields()[index].index() as usize].identity(),
                    rustc_type_identity_v1(tcx, capture)
                );
            }
            let exact = ExtractionLimits {
                max_fields_per_aggregate: captures.len(),
                ..ExtractionLimits::default()
            };
            assert!(extract_general_layout_with_limits(tcx, ty, exact).is_ok());
            if !captures.is_empty() {
                assert!(matches!(
                    extract_general_layout_with_limits(
                        tcx,
                        ty,
                        ExtractionLimits {
                            max_fields_per_aggregate: captures.len() - 1,
                            ..exact
                        }
                    ),
                    Err(GeneralLayoutExtractError::BoundExceeded {
                        kind: LimitKind::Fields,
                        ..
                    })
                ));
            }
        }
        ClosureLayoutResult {
            name,
            facts,
            evidence,
            record,
            types,
        }
    })
    .collect()
}

fn collect_fixture_types<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    inventory: &mut BTreeMap<SemanticTypeIdentityV1, Ty<'tcx>>,
) {
    let ty = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), ty)
        .unwrap();
    if inventory
        .insert(rustc_type_identity_v1(tcx, ty), ty)
        .is_some()
    {
        return;
    }
    assert!(inventory.len() < 128, "fixture type graph must stay small");
    match ty.kind() {
        TyKind::Closure(_, args) => {
            for capture in args.as_closure().upvar_tys() {
                collect_fixture_types(tcx, capture, inventory);
            }
        }
        TyKind::Ref(_, pointee, _) => collect_fixture_types(tcx, *pointee, inventory),
        TyKind::Adt(definition, args) => {
            if definition.is_enum() {
                collect_fixture_types(tcx, definition.repr().discr_type().to_ty(tcx), inventory);
            }
            for variant in definition.variants() {
                for field in &variant.fields {
                    collect_fixture_types(tcx, field.ty(tcx, args), inventory);
                }
            }
        }
        TyKind::Bool | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_) => {}
        other => panic!("unexpected fixture type {other:?}"),
    }
}

fn result<'a>(results: &'a [ClosureLayoutResult], name: &str) -> &'a ClosureLayoutResult {
    results.iter().find(|result| result.name == name).unwrap()
}

#[test]
fn live_closure_types_preserve_nominal_identity_layout_and_capture_order() {
    let results = compiler_results().closures;
    assert_eq!(results.len(), 9);
    let empty = result(&results, "captureless");
    assert_eq!(empty.facts.size_bytes, 0);
    let MirTypeKind::Struct(empty_struct) = &empty.evidence.semantic_type().kind else {
        panic!("nominal empty struct");
    };
    assert!(empty_struct.aggregate.fields.is_empty());
    let mixed = result(&results, "mixed_captures");
    let TypeLayoutKind::Closure { fields, .. } = &mixed.facts.kind else {
        unreachable!()
    };
    assert_eq!(fields.len(), 4);
    assert!(fields.iter().any(|field| field.layout.size_bytes == 0));
    assert!(
        fields
            .iter()
            .any(|field| field.source_index != field.memory_index)
    );
    for (left, right) in [
        ("first_closure", "second_closure"),
        ("generic_u32", "generic_f32"),
    ] {
        let left = result(&results, left);
        let right = result(&results, right);
        assert_eq!(left.facts.size_bytes, right.facts.size_bytes);
        assert_eq!(
            left.facts.abi_alignment_bytes,
            right.facts.abi_alignment_bytes
        );
        assert_ne!(left.record.identity(), right.record.identity());
        assert_ne!(
            left.evidence.semantic_type(),
            right.evidence.semantic_type()
        );
        assert_ne!(
            left.evidence.canonical_bytes(),
            right.evidence.canonical_bytes()
        );
    }
}

#[test]
fn live_closure_types_preserve_reference_mutability_and_niche_payloads() {
    let results = compiler_results().closures;
    for (name, expected, expected_semantic) in [
        (
            "shared_capture",
            PointerKind::SharedReference,
            SemanticMutabilityV1::Immutable,
        ),
        (
            "mutable_capture",
            PointerKind::MutableReference,
            SemanticMutabilityV1::Mutable,
        ),
    ] {
        let result = result(&results, name);
        let TypeLayoutKind::Closure { fields, .. } = &result.facts.kind else {
            unreachable!()
        };
        let TypeLayoutKind::Pointer(pointer) = &fields[0].layout.kind else {
            panic!("reference capture");
        };
        assert_eq!(pointer.kind, expected);
        let SemanticTypeShapeV1::Aggregate(aggregate) = result.record.shape() else {
            unreachable!()
        };
        let SemanticTypeShapeV1::Pointer(pointer) =
            result.types[aggregate.fields()[0].index() as usize].shape()
        else {
            panic!("semantic reference capture");
        };
        assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
        assert_eq!(pointer.mutability(), expected_semantic);
    }
    let optional = result(&results, "optional_capture");
    let TypeLayoutKind::Adt(adt) = &optional.facts.kind else {
        panic!("Option closure");
    };
    assert!(matches!(
        adt.tag.unwrap().encoding,
        EnumTagEncodingFacts::Niche { .. }
    ));
    assert!(matches!(
        optional.record.layout().variants(),
        SemanticRustcVariantsV1::Multiple(_)
    ));
}
