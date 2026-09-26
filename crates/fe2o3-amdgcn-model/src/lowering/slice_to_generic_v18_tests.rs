use super::storage_native_v18::tests::*;
use super::*;

fn expose(id: u32, source: u32, element: Type, access: AccessMode) -> Operation {
    let to = Type::slice(element, Space::Generic, access);
    value(
        id,
        to.clone(),
        OperationKind::Cast {
            kind: CastKind::SliceToGeneric,
            value: ValueId(source),
            to,
        },
    )
}

fn fixture(profile: Profile, space: Space, object_value: bool) -> Module {
    let element = if object_value {
        object(0)
    } else {
        Type::Scalar(ScalarType::U32)
    };
    let access = AccessMode::ReadOnly;
    let concrete = Type::slice(element.clone(), space, access);
    let generic = Type::slice(element.clone(), Space::Generic, access);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        expose(1, 0, element, access),
        value(
            2,
            generic.clone(),
            OperationKind::Call {
                callee: fe2o3_kernel_ir::FunctionId::new("identity"),
                arguments: vec![ValueId(1)],
            },
        ),
    ];
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(2)],
    });
    let mut exit = BasicBlock::new(BlockId(1));
    exit.parameters
        .push(ValueDef::new(ValueId(3), generic.clone()));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let mut identity = BasicBlock::new(BlockId(0));
    identity.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let mut source = module(vec![scalar()], vec![], profile);
    source.functions.push(Function::internal_helper(
        "expose",
        Signature::new(vec![concrete], vec![generic.clone()]),
        vec![ValueId(0)],
        vec![entry, exit],
    ));
    source.functions.push(Function::internal_helper(
        "identity",
        Signature::new(vec![generic.clone()], vec![generic]),
        vec![ValueId(0)],
        vec![identity],
    ));
    bind(&mut source, profile);
    source
}

#[test]
fn whole_slice_call_phi_return_emits_pointer_cast_and_unchanged_length_on_both_targets() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for object_value in [false, true] {
            for (space, number) in [
                (Space::Global, 1),
                (Space::Constant, 4),
                (Space::Private, 5),
                (Space::Workgroup, 3),
            ] {
                let text = emit(&fixture(profile, space, object_value), profile);
                assert!(text.contains(&format!("addrspacecast ptr addrspace({number})")));
                assert!(text.contains("to ptr addrspace(0)"));
                assert!(text.contains("add i64"));
                assert!(text.contains(", 0"));
                assert!(text.contains("phi ptr addrspace(0)"));
                assert!(text.contains("phi i64"));
                assert!(text.contains("extractvalue"));
                assert!(text.contains("insertvalue"));
                assert!(!text.contains("ptrtoint"));
                assert!(!text.contains("inttoptr"));
                assert!(!text.contains("call void @llvm.trap()"));
            }
        }
    }
}

#[test]
fn native_slice_validation_cannot_change_length_shape_space_element_or_permissions() {
    let private = Type::slice(Type::F32, Space::Private, AccessMode::ReadOnly);
    let generic = Type::slice(Type::F32, Space::Generic, AccessMode::ReadOnly);
    for target in [
        LoweringTarget::Gfx942XnackMinusV1,
        LoweringTarget::Gfx950XnackMinusV1,
    ] {
        assert!(validate_cast(CastKind::SliceToGeneric, &private, &generic, target).is_ok());
        for (from, to) in [
            (generic.clone(), private.clone()),
            (generic.clone(), generic.clone()),
            (
                private.clone(),
                Type::slice(Type::F32, Space::Generic, AccessMode::ReadWrite),
            ),
            (
                private.clone(),
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    Space::Generic,
                    AccessMode::ReadOnly,
                ),
            ),
            (
                private.clone(),
                Type::pointer(Type::F32, Space::Generic, AccessMode::ReadOnly),
            ),
            (
                Type::slice(Type::F32, Space::Constant, AccessMode::ReadWrite),
                Type::slice(Type::F32, Space::Generic, AccessMode::ReadWrite),
            ),
        ] {
            assert!(validate_cast(CastKind::SliceToGeneric, &from, &to, target).is_err());
        }
    }
}

#[test]
fn actual_v18_slice_native_work_and_storage_boundaries_preserve_the_source_floor() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        VerifiedCanonicalKernelIrModuleV18 as Owner,
    };
    let graph = fixture(Profile::Gfx942, Space::Private, false);
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let admitted = Owner::from_module_ref_with_verification_budget_v18(
            &graph,
            StorageLayoutLimitsV1 {
                rows: 64,
                edges: 256,
                containment_depth: 32,
                object_bytes: 4096,
            },
            &mut budget,
        );
        let success = match admitted {
            Ok((owner, receipt)) => {
                if budget.reserve_storage(receipt.retained_storage()).is_err() {
                    false
                } else {
                    let floor = budget.storage();
                    let result = super::storage_native_v18::lower_canonical_storage_module_v18(
                        &owner,
                        Profile::Gfx942,
                        &mut budget,
                    );
                    assert_eq!(budget.storage(), floor);
                    let success = result.is_ok();
                    drop((result, owner));
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    success
                }
            }
            Err(_) => false,
        };
        assert_eq!(budget.storage(), 17);
        let used = budget.work();
        let peak = budget.peak_storage();
        let denied_storage = budget.failed_storage().is_some();
        (
            success,
            used,
            peak,
            work.failed_work().is_some(),
            denied_storage,
        )
    };
    let baseline = run(1_000_000_000, 1_000_000_000);
    assert!(baseline.0);
    assert!(run(baseline.1, baseline.2).0);
    let short_work = run(baseline.1 - 1, baseline.2);
    assert!(!short_work.0 && short_work.3);
    let short_storage = run(baseline.1, baseline.2 - 1);
    assert!(!short_storage.0 && short_storage.4);
}
