//! Tampered data against the unchanged real pending owner, not forged proof fixtures.
use super::*;
use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingV1 as Binding, ReferenceArgumentRelationV1 as Relation,
    ReferenceEffectExpressionV1 as Expr, ReferenceLogicalSignaturePreimageV1 as Preimage,
    ReferenceOutputCoordinateV1 as Coordinate, ReferencePlaceProjectionV1 as Projection,
    ReferenceScalarTypeV1 as Scalar,
    reference_signature_preimage_v1::{
        ReferenceCarrierV1 as Carrier, ReferencePointeeV1 as Pointee, ReferenceRegionV1 as Region,
        ReferenceReturnShapeV1 as Return, ReferenceSignatureInputV1 as Input,
    },
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExternAbiV1 as Abi, SemanticFunctionSafetyV1 as Safety,
    SemanticMutabilityV1 as Mutability,
};

pub(super) fn check(
    owner: &ProductionPreRankedKirOwnerV1,
    pending: &ProductionConditionalRankedAnalysisV1,
    candidate: NativeRankedSourceCandidateV1<'_>,
    source: &BoundSourceV1<'_>,
) {
    let [original] = source.references.as_slice() else {
        panic!("single-reference source fixture");
    };
    let refuse = |label: &str, expected: &str, bindings: Vec<Binding>| {
        let references = AuthenticatedReferenceEffectBindingsV1::new(bindings);
        let floor = owner.retained_analysis_storage_v1();
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, floor + 1024 * 1024);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let mut callbacks = 0;
        let result = with_conditional_reference_output_v1(
            owner,
            pending,
            candidate,
            &references,
            source.logical_name,
            &mut budget,
            |_| {
                callbacks += 1;
                Ok(())
            },
        );
        assert!(
            matches!(result, Err(JoinError::Reference(reason)) if reason == expected),
            "{label}: {result:?}"
        );
        assert_eq!(callbacks, 0, "{label}");
        assert_eq!(budget.storage(), floor, "{label}");
        assert!(budget.work_ledger_identity_v1() == ledger);
    };
    refuse("missing reference", "binding count", vec![]);
    refuse(
        "duplicate reference",
        "binding count",
        vec![original.clone(), original.clone()],
    );
    type Mutation = fn(&mut Binding);
    let cases: &[(&str, &str, Mutation)] = &[
        ("logical name", "logical name", |b| {
            b.logical_kernel_name.push_str("_changed")
        }),
        ("kernel function", "kernel identity", |b| {
            b.kernel.function_sha256[0] ^= 1
        }),
        ("kernel item", "kernel identity", |b| {
            b.kernel.item_definition_sha256[0] ^= 1
        }),
        ("kernel instantiation", "kernel identity", |b| {
            b.kernel.monomorphization_sha256[0] ^= 1
        }),
        ("kernel type args", "kernel identity", |b| {
            b.kernel.generic_type_arguments_sha256[0] ^= 1
        }),
        ("kernel const args", "kernel identity", |b| {
            b.kernel.const_generic_arguments_sha256[0] ^= 1
        }),
        ("kernel MIR", "contract subjects", |b| {
            b.kernel.rustc_mir_body_sha256[0] ^= 1
        }),
        ("reference function", "contract subjects", |b| {
            b.reference.function_sha256[0] ^= 1
        }),
        ("reference MIR", "contract subjects", |b| {
            b.reference.rustc_mir_body_sha256[0] ^= 1
        }),
        ("effect digest", "effect digest", |b| {
            b.effect_ir_sha256[0] ^= 1
        }),
        ("missing relation", "IR header", |b| {
            b.effect_ir.relations = Box::default()
        }),
        ("reordered relation", "IR relation", |b| {
            b.effect_ir.relations.swap(0, 1)
        }),
        ("raw ordinal as logical", "IR relation", |b| {
            let Relation::DisjointOutputCoordinate { argument, .. } = &mut b.effect_ir.relations[1]
            else {
                panic!("point output relation");
            };
            *argument = 1;
        }),
        ("missing store", "output count", |b| {
            b.observable_output_writes = Box::default()
        }),
        ("extra store", "output count", |b| {
            b.observable_output_writes =
                vec![b.observable_output_writes[0].clone(); 2].into_boxed_slice()
        }),
        ("missing effect", "effect count", |b| {
            b.effect_ir.observable_output_effects = Box::default()
        }),
        ("store raw ordinal", "CPU write identity/value", |b| {
            b.observable_output_writes[0].argument = 1
        }),
        ("store block", "CPU write identity/value", |b| {
            b.observable_output_writes[0].block = 1
        }),
        ("store statement", "CPU write identity/value", |b| {
            b.observable_output_writes[0].statement += 1
        }),
        ("point axis", "CPU coordinate", |b| {
            b.observable_output_writes[0].coordinate =
                Coordinate::LogicalPoint(vec![Expr::PointCoordinate { axis: 1 }].into_boxed_slice())
        }),
        ("missing point", "CPU coordinate", |b| {
            b.observable_output_writes[0].coordinate = Coordinate::SingleCoordinate
        }),
        ("false domain", "CPU guard", |b| {
            b.observable_output_writes[0].guard.clauses = Box::default()
        }),
        ("nonconstant RHS", "CPU RHS", |b| {
            b.observable_output_writes[0].rhs = Expr::KernelScalarArgument { argument: 0 }
        }),
        ("IR argument count", "IR header", |b| {
            b.effect_ir.argument_count += 1
        }),
        ("IR local count", "IR header", |b| {
            b.effect_ir.local_count = 0
        }),
        ("IR missing block", "CPU blocks", |b| {
            b.effect_ir.blocks = Box::default()
        }),
        ("IR extra block", "CPU blocks", |b| {
            b.effect_ir.blocks = vec![b.effect_ir.blocks[0].clone(); 2].into_boxed_slice()
        }),
        ("IR missing assignments", "CPU block", |b| {
            b.effect_ir.blocks[0].assignments = Box::default()
        }),
        ("IR output local", "CPU assignment", |b| {
            let write = b.effect_ir.blocks[0]
                .assignments
                .iter_mut()
                .find(|a| !a.destination.projection.is_empty())
                .unwrap();
            write.destination.local = 1;
        }),
        ("IR output projection", "CPU assignment", |b| {
            let write = b.effect_ir.blocks[0]
                .assignments
                .iter_mut()
                .find(|a| !a.destination.projection.is_empty())
                .unwrap();
            write.destination.projection = vec![Projection::Field(0)].into_boxed_slice();
        }),
    ];
    for &(label, expected, mutate) in cases {
        let mut changed = original.clone();
        mutate(&mut changed);
        refuse(label, expected, vec![changed]);
    }
    let [Input::NominalOutput { element, .. }] = original.signature_preimage.kernel_inputs() else {
        panic!("single output source");
    };
    let element = *element;
    let output = Input::NominalOutput {
        carrier: Carrier::DisjointSlice,
        element,
    };
    let point = Input::Scalar(Scalar::Usize);
    let cell = Input::Reference {
        region: Region::Erased,
        mutability: Mutability::Mutable,
        pointee: Pointee::Scalar(element),
    };
    for (label, expected, kernel, reference) in [
        (
            "u64 is not usize",
            "signature relation",
            output,
            vec![Input::Scalar(Scalar::U64), cell],
        ),
        (
            "missing point prefix",
            "signature fragment",
            output,
            vec![cell],
        ),
        (
            "immutable output",
            "signature relation",
            output,
            vec![
                point,
                Input::Reference {
                    region: Region::Erased,
                    mutability: Mutability::Immutable,
                    pointee: Pointee::Scalar(element),
                },
            ],
        ),
        (
            "slice is not point output",
            "IR relation",
            output,
            vec![
                point,
                Input::Reference {
                    region: Region::Erased,
                    mutability: Mutability::Mutable,
                    pointee: Pointee::Slice(element),
                },
            ],
        ),
        (
            "different output width",
            "signature fragment",
            Input::NominalOutput {
                carrier: Carrier::DisjointSlice,
                element: Scalar::U64,
            },
            vec![
                point,
                Input::Reference {
                    region: Region::Erased,
                    mutability: Mutability::Mutable,
                    pointee: Pointee::Scalar(Scalar::U64),
                },
            ],
        ),
    ] {
        let mut changed = original.clone();
        changed.signature_preimage = Preimage::new(
            vec![kernel].into_boxed_slice(),
            reference.into_boxed_slice(),
            Return::Unit,
            Abi::Rust,
            Safety::Safe,
            false,
        )
        .unwrap();
        // The old effect hash is unchanged: signature replay is a separate check.
        assert_eq!(changed.effect_ir_sha256, original.effect_ir_sha256);
        refuse(label, expected, vec![changed]);
    }
}
