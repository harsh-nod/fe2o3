use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVersionV1 as Version, IntrinsicOperation, ValueDef,
    VerifiedCanonicalKernelIrV1 as Canonical,
};
#[path = "../v13/reusable_phase_tests/fixture.rs"]
mod fixture;

fn physical(module: Module) -> Module {
    let owner = Canonical::from_module(module.clone(), Version::V14).unwrap();
    let launch =
        crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
    let closure = crate::legalize_production_target_capabilities_kir_v1(
        &owner,
        7,
        &launch,
        ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    let authority =
        V13LoweringAuthorityV1::from_declared_closure(&module, &closure, &owner).unwrap();
    v13::lower_declared_execution_capabilities_v1(
        &module,
        ProductionAmdTargetProfileV1::Gfx942,
        &authority,
        Version::V14,
        ReusablePhaseCheckLimitsV1::DEFAULT,
    )
    .unwrap()
}

#[test]
fn structured_writer_records_require_full_census_and_original_limits() {
    let module = physical(fixture::memory(2));
    for limits in [
        ReusablePhaseCheckLimitsV1 {
            work: 0,
            ..ReusablePhaseCheckLimitsV1::DEFAULT
        },
        ReusablePhaseCheckLimitsV1 {
            temporary_bytes: 0,
            ..ReusablePhaseCheckLimitsV1::DEFAULT
        },
    ] {
        assert_eq!(
            Builder::new(&module, limits).err().unwrap().diagnostics()[0].code,
            LoweringDiagnosticCode::ResourceLimit
        );
    }
    assert_eq!(
        Builder::new(&module, ReusablePhaseCheckLimitsV1::DEFAULT)
            .unwrap()
            .finish(&module)
            .unwrap_err()
            .diagnostics()[0]
            .code,
        LoweringDiagnosticCode::ResourceLimit
    );
    let mut incomplete = Builder::new(&module, ReusablePhaseCheckLimitsV1::DEFAULT).unwrap();
    incomplete
        .record(&module, 0, BlockId(0), Some(0), None, 0, [0; 32])
        .unwrap();
    assert!(
        incomplete.finish(&module).unwrap().is_none(),
        "unsupported recipe cannot produce a partial derivation"
    );
}

#[test]
fn structured_writer_digest_is_actual_forwarded_text_and_failure_is_retained() {
    let mut text = String::new();
    let mut writer = DigestWriter::new(&mut text);
    fmt::Write::write_str(&mut writer, "first\n").unwrap();
    fmt::Write::write_str(&mut writer, "second").unwrap();
    let (bytes, digest) = writer.finish();
    assert_eq!(text, "first\nsecond");
    assert_eq!(bytes, text.len());
    assert_eq!(digest, <[u8; 32]>::from(Sha256::digest(text.as_bytes())));
    struct Reject;
    impl fmt::Write for Reject {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }
    let mut reject = Reject;
    assert!(fmt::Write::write_str(&mut DigestWriter::new(&mut reject), "not emitted").is_err());
}

#[test]
fn structured_writer_function_coordinate_requires_same_borrowed_roster_not_equal_payload() {
    let module = physical(fixture::memory(2));
    let copy = module.clone();
    for (index, function) in module.functions.iter().enumerate() {
        assert_eq!(function_ordinal(&module, function), Some(index as u32));
        assert_eq!(function_ordinal(&copy, function), None);
    }
}

#[test]
fn structured_writer_sealed_replay_rejects_coordinate_recipe_and_bytes_substitutions() {
    let owner = Canonical::from_module(fixture::memory(2), Version::V14).unwrap();
    let launch =
        crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
    for mutation in 0..5 {
        let mut output = lower_verified_canonical_kir_to_amd_llvm_ir_v1(
            &owner,
            7,
            &launch,
            ProductionAmdTargetProfileV1::Gfx942,
        )
        .unwrap();
        let mut replay = output.declared_replay.take().unwrap();
        // A mutable clone is deliberately unavailable publicly. This child is
        // testing the same sealed constructor through its private module.
        let structured = replay.structured_mut_for_test();
        let segment = structured
            .segments
            .iter_mut()
            .find(|s| {
                matches!(
                    s.kind,
                    DeclaredWriterSegmentKindV1::Operation(
                        DeclaredLlvmRecipeV1::PhysicalWorkgroupBarrier { .. }
                    )
                )
            })
            .unwrap();
        match mutation {
            0 => segment.block = BlockId(segment.block.0 + 1),
            1 => segment.operation = segment.operation.map(|i| i + 1),
            2 => {
                segment.kind =
                    DeclaredWriterSegmentKindV1::Operation(DeclaredLlvmRecipeV1::GlobalAlias)
            }
            3 => segment.llvm_bytes += 1,
            4 => segment.llvm_sha256[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                replay.replay(
                    &owner,
                    7,
                    &launch,
                    ProductionAmdTargetProfileV1::Gfx942,
                    output.llvm_ir().as_bytes()
                ),
                Err(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch)
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn structured_writer_uses_real_analysis_and_rejects_divergent_physical_barrier() {
    let mut module = physical(fixture::memory(2));
    let body = module.functions[0].body.as_mut().unwrap();
    let entry = &mut body.blocks[0];
    let next = entry
        .operations
        .iter()
        .flat_map(|o| &o.results)
        .map(|v| v.id.0)
        .max()
        .unwrap()
        + 1;
    let barrier_index = entry
        .operations
        .iter()
        .rposition(|o| matches!(o.kind, OperationKind::WorkgroupBarrier(_)))
        .unwrap();
    let barrier = entry.operations.remove(barrier_index);
    entry.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(next), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(next + 1), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(next + 2), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(next),
                rhs: ValueId(next + 1),
            },
        ),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(next + 2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut divergent = BasicBlock::new(BlockId(1));
    divergent.operations.push(barrier);
    divergent.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([divergent, exit]);
    fe2o3_kernel_ir::verify_module(&module)
        .expect("valid typed physical CFG, not a convergence proof");
    let trace = std::cell::RefCell::new(
        Builder::new(&module, ReusablePhaseCheckLimitsV1::DEFAULT).unwrap(),
    );
    let result = lower_compiler_module_to_llvm_ir_for_target_recorded(
        &module,
        LoweringTarget::Gfx942XnackMinusV1,
        None,
        None,
        true,
        Some(&trace),
    );
    assert!(
        matches!(result, Err(ref e) if e.diagnostics().iter().any(|d|
        d.code == LoweringDiagnosticCode::UnprovenBarrierConvergence && d.location.block == Some(BlockId(1)))),
        "{result:?}"
    );
}

#[test]
fn structured_writer_unsupported_recipe_keeps_entire_operational_roster_fail_closed() {
    let mut module = fixture::memory(2);
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let next = operations
        .iter()
        .flat_map(|o| &o.results)
        .map(|v| v.id.0)
        .max()
        .unwrap()
        + 1;
    operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(next), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(7)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(next + 1), Type::Scalar(ScalarType::U32)),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(next),
            },
        ),
    ]);
    let owner = Canonical::from_module(module, Version::V14).unwrap();
    let launch =
        crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
    let output = lower_verified_canonical_kir_to_amd_llvm_ir_v1(
        &owner,
        7,
        &launch,
        ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    assert!(
        output.llvm_ir().contains("xor i32"),
        "real writer still supports this scalar operation"
    );
    assert!(output.declared_replay().unwrap().structured().is_none());
    assert!(!output.has_complete_operational_translation_derivation());
    assert!(
        output
            .unsupported_operational_translation()
            .iter()
            .any(|o| o.family() == ProductionV13KirOperationFamilyV1::ReusablePhase)
    );
    output
        .declared_replay()
        .unwrap()
        .replay(
            &owner,
            7,
            &launch,
            ProductionAmdTargetProfileV1::Gfx942,
            output.llvm_ir().as_bytes(),
        )
        .unwrap();
}
