//! Observations of the real frontend and the exact source KIR consumed by Policy5.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use std::collections::BTreeMap;

const CONFIG: &str = "FE2O3_TEST_SHARED_PRIMITIVE_POLICY5";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Config {
    field1: bool,
    retained: bool,
}

impl Config {
    pub(in super::super) fn name(self) -> String {
        format!(
            "shared-primitive-{}-{}",
            if self.field1 { "field1" } else { "direct" },
            if self.retained { "opt0" } else { "normal" }
        )
    }

    pub(in super::super) fn configure(self, args: &mut Vec<String>) {
        if self.retained {
            args.push("-Zmir-opt-level=0".into());
        }
        if self.field1 {
            args.push("--cfg=feature=\"shared-primitive-policy5-field1\"".into());
        }
    }
}

pub(super) fn configure_child(command: &mut Command, case: &OrdinarySourceCase) {
    command.env_remove(CONFIG);
    if let OrdinarySourceCase::SharedPrimitivePolicy5(config) = case {
        command.env(CONFIG, serde_json::to_string(config).unwrap());
    }
}

pub(super) fn requested() -> Option<Config> {
    env::var(CONFIG)
        .ok()
        .map(|value| serde_json::from_str(&value).unwrap())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum FrontendShape {
    RetainedDirect,
    RetainedField1,
    RustcErased,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observed {
    config: Config,
    shape: FrontendShape,
    source_semantic: [u8; 32],
    consumed_ssa: [u8; 32],
    source_kir: [u8; 32],
    promoted_variables: usize,
    memory_variables: usize,
    // function, block, statement, referent local, reference local
    shared_borrows: Vec<[u32; 5]>,
    field1_transports: usize,
    dereference_reads: usize,
    // Alloca, Load, Store in the pre-Policy5 original KIR, not optimized O.
    original_private: [usize; 3],
}

fn place(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => Some(place),
        SemanticOperandV1::Constant(_) => None,
    }
}

fn scalar_u32(semantic: &AdmittedInertSemanticMirV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        semantic.types()[ty.index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        })
    )
}

fn path(place: &SemanticPlaceV1) -> Vec<SemanticProjectionKindV1> {
    place
        .projections()
        .iter()
        .map(|projection| projection.kind())
        .collect()
}

fn source_shape(semantic: &AdmittedInertSemanticMirV1) -> (Vec<[u32; 5]>, usize, usize) {
    let [root] = semantic.roots() else {
        panic!("one actual Shared source root")
    };
    let selected = semantic.select_kernel_body_for_root_v1(*root).unwrap();
    assert_eq!(selected.root(), *root);
    let function = &semantic.functions()[selected.body().index() as usize];
    let mut borrows = Vec::new();
    let mut fields = 0;
    let mut reads = 0;
    for (block_index, block) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: referent,
            } = assignment.value().kind()
            else {
                continue;
            };
            if !referent.projections().is_empty() || !scalar_u32(semantic, referent.ty()) {
                continue;
            }
            assert!(assignment.destination().projections().is_empty());
            let reference = assignment.destination().local();
            borrows.push([
                selected.body().index(),
                block_index.try_into().unwrap(),
                statement_index.try_into().unwrap(),
                referent.local().index(),
                reference.index(),
            ]);
            // This is a test observation, not a second storage-selection planner.
            // Follow only the original same-block copies and exact tuple Field1.
            let mut aliases = BTreeMap::from([(reference, Vec::new())]);
            for statement in &block.statements()[statement_index + 1..] {
                match statement.kind() {
                    SemanticStatementKindV1::StorageLive(local)
                    | SemanticStatementKindV1::StorageDead(local) => {
                        aliases.remove(local);
                    }
                    SemanticStatementKindV1::Deinitialize(place)
                        if place.projections().is_empty() =>
                    {
                        aliases.remove(&place.local());
                    }
                    _ => {}
                }
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let mut new_alias = None;
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Use(operand) => {
                        if let Some(source) = place(operand)
                            && let Some(prefix) = aliases.get(&source.local())
                        {
                            let actual = path(source);
                            if actual == *prefix {
                                new_alias = Some(Vec::new());
                            }
                            let mut dereference = prefix.clone();
                            dereference.push(SemanticProjectionKindV1::Dereference);
                            if actual == dereference && scalar_u32(semantic, source.ty()) {
                                reads += 1;
                            }
                        }
                    }
                    SemanticRvalueKindV1::Aggregate(value)
                        if matches!(value.kind(), SemanticAggregateKindV1::Tuple)
                            && value.operands().len() == 2 =>
                    {
                        if let Some(source) = place(&value.operands()[1])
                            && aliases
                                .get(&source.local())
                                .is_some_and(|prefix| path(source) == *prefix)
                        {
                            assert!(scalar_u32(semantic, value.operands()[0].ty()));
                            new_alias = Some(vec![SemanticProjectionKindV1::Field(1)]);
                            fields += 1;
                        }
                    }
                    _ => {}
                }
                if assignment.destination().projections().is_empty() {
                    aliases.remove(&assignment.destination().local());
                    if let Some(prefix) = new_alias {
                        aliases.insert(assignment.destination().local(), prefix);
                    }
                }
            }
        }
    }
    (borrows, fields, reads)
}

pub(super) fn observe(
    stage: &crate::production_pipeline::fixed_checked_output_v1::FixedCheckedOutputProductionCompilationV1,
    config: Config,
) -> Observed {
    let semantic = stage.semantic();
    let (shared_borrows, field1_transports, dereference_reads) = source_shape(semantic);
    let (consumed_ssa, summary) = stage.original_ssa_observation();
    let mut original_private = [0; 3];
    for function in &stage.original_module().functions {
        let Some(body) = &function.body else { continue };
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            match &operation.kind {
                OperationKind::Alloca {
                    address_space: AddressSpace::Private,
                    ..
                } => original_private[0] += 1,
                OperationKind::Load { access, .. }
                    if access.address_space == AddressSpace::Private =>
                {
                    original_private[1] += 1
                }
                OperationKind::Store { access, .. }
                    if access.address_space == AddressSpace::Private =>
                {
                    original_private[2] += 1
                }
                _ => {}
            }
        }
    }
    let observed = Observed {
        config,
        shape: if shared_borrows.is_empty() {
            FrontendShape::RustcErased
        } else if field1_transports == 0 {
            FrontendShape::RetainedDirect
        } else {
            FrontendShape::RetainedField1
        },
        source_semantic: *semantic.semantic_sha256().as_bytes(),
        consumed_ssa,
        source_kir: stage.original_digest(),
        promoted_variables: summary.promotable_variables(),
        memory_variables: summary.memory_variables(),
        shared_borrows,
        field1_transports,
        dereference_reads,
        original_private,
    };
    check(&observed, config);
    observed
}

pub(super) fn check(observed: &Observed, config: Config) {
    assert_eq!(observed.config, config);
    assert_ne!(observed.source_semantic, [0; 32]);
    assert_ne!(observed.consumed_ssa, [0; 32]);
    assert_ne!(observed.source_kir, [0; 32]);
    assert!(observed.promoted_variables > 0);
    assert_eq!(
        observed.original_private, [0; 3],
        "backing must already be absent in source KIR"
    );
    if config.retained {
        assert_eq!(
            observed.shape,
            if config.field1 {
                FrontendShape::RetainedField1
            } else {
                FrontendShape::RetainedDirect
            },
            "MIR opt0 must retain the exact original Shared primitive path"
        );
    }
    match observed.shape {
        FrontendShape::RetainedDirect | FrontendShape::RetainedField1 => {
            assert_eq!(observed.shared_borrows.len(), 1);
            assert!(observed.dereference_reads > 0);
            assert_eq!(
                observed.field1_transports,
                usize::from(observed.shape == FrontendShape::RetainedField1)
            );
            if !config.field1 {
                assert_eq!(observed.shape, FrontendShape::RetainedDirect);
            }
        }
        FrontendShape::RustcErased => {
            assert!(!config.retained);
            assert!(observed.shared_borrows.is_empty());
            assert_eq!(
                (observed.field1_transports, observed.dereference_reads),
                (0, 0)
            );
        }
    }
}

#[test]
fn shared_primitive_observation_requires_original_path_and_pre_policy_backing_absence() {
    for field1 in [false, true] {
        let config = Config {
            field1,
            retained: true,
        };
        let positive = Observed {
            config,
            shape: if field1 {
                FrontendShape::RetainedField1
            } else {
                FrontendShape::RetainedDirect
            },
            source_semantic: [1; 32],
            consumed_ssa: [2; 32],
            source_kir: [3; 32],
            promoted_variables: 1,
            memory_variables: 0,
            shared_borrows: vec![[0, 1, 2, 3, 4]],
            field1_transports: usize::from(field1),
            dereference_reads: 1,
            original_private: [0; 3],
        };
        check(&positive, config);
        let encoded = serde_json::to_value(&positive).unwrap();
        let decoded: Observed = serde_json::from_value(encoded.clone()).unwrap();
        check(&decoded, config);
        let mut extra = encoded;
        extra["unclaimed"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Observed>(extra).is_err());
        for fault in 0..8 {
            let mut wrong = positive.clone();
            match fault {
                0 => wrong.shared_borrows.clear(),
                1 => wrong.shared_borrows.push([0, 1, 5, 6, 7]),
                2 => wrong.dereference_reads = 0,
                3 => wrong.field1_transports += 1,
                4 => wrong.original_private[0] = 1,
                5 => wrong.original_private[1] = 1,
                6 => wrong.original_private[2] = 1,
                _ => wrong.consumed_ssa = [0; 32],
            }
            assert!(
                std::panic::catch_unwind(|| check(&wrong, config)).is_err(),
                "fault {fault}"
            );
        }
        let mut erased = positive;
        erased.config.retained = false;
        erased.shape = FrontendShape::RustcErased;
        erased.shared_borrows.clear();
        erased.field1_transports = 0;
        erased.dereference_reads = 0;
        check(&erased, erased.config);
        assert!(std::panic::catch_unwind(|| check(&erased, config)).is_err());
    }
}

#[test]
#[ignore = "compiles actual Rust source for both built-in targets and runs the existing simulator"]
fn ordinary_rust_shared_primitive_zero_backing_both_profiles() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let mut cases = Vec::new();
        for retained in [true, false] {
            for field1 in [false, true] {
                cases.push(OrdinarySourceCase::SharedPrimitivePolicy5(Config {
                    field1,
                    retained,
                }));
            }
        }
        // Keep the actual mutable, intervening-global-store control backed.
        cases.push(OrdinarySourceCase::ScalarBorrowPolicy5Barrier);
        ordinary_rust_checked_output_cases_for_profile(&cases, profile);
    }
}
