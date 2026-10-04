//! Pure projection/resource controls. These inert graphs never mint Checked owners.
use super::*;
use fe2o3_kernel_ir::{
    AssemblySourceIdentity, BlockId, FunctionId, Gfx942OrderedProgramRegistersV1,
    Gfx942OrderedProgramV1, Gfx942U32ProgramV1, Kernel, LaunchDomain, LaunchExtent, MemoryAccess,
    Signature, ValueDef,
};
fn fixture(edited: bool) -> (Module, Selection) {
    let mut module = Module::new("inert_roles_control");
    let mut root = BasicBlock::new(BlockId(40));
    root.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(51), U32),
        OperationKind::Call {
            callee: FunctionId::new("helper"),
            arguments: vec![ValueId(3), ValueId(6), ValueId(10)],
        },
    ));
    root.operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(62),
            Type::pointer(U32, AddressSpace::Global, AccessMode::ReadWrite),
        ),
        OperationKind::SliceData { slice: ValueId(8) },
    ));
    root.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(62),
            value: ValueId(51),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    root.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "root",
        Signature::new(
            vec![
                Type::slice(U32, AddressSpace::Global, AccessMode::ReadWrite),
                U32,
                U32,
                U32,
            ],
            vec![],
        ),
        vec![ValueId(8), ValueId(3), ValueId(6), ValueId(10)],
        vec![root],
    ));
    let mut helper = BasicBlock::new(BlockId(73));
    let mut descriptors = [0; 16];
    let count = if edited {
        descriptors[0] = 40;
        1
    } else {
        descriptors[0] = 133;
        descriptors[1] = 315;
        2
    };
    helper.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(49), U32),
        OperationKind::Gfx942OrderedProgram(
            Gfx942OrderedProgramV1::new(
                AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
                Gfx942OrderedProgramRegistersV1::new(8, 9, [10, 11, 12]).unwrap(),
                [ValueId(91), ValueId(17), ValueId(28)],
                Gfx942U32ProgramV1::from_descriptors(count, descriptors).unwrap(),
            )
            .unwrap(),
        ),
    ));
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(49)],
    });
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![U32, U32, U32], vec![U32]),
        vec![ValueId(91), ValueId(17), ValueId(28)],
        vec![helper],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(128),
        },
    ));
    (
        module,
        Selection {
            root: 0,
            helper: 1,
            call: Site {
                function: 0,
                ordinal: 0,
                block: BlockId(40),
                operation: 0,
            },
            definition: Site {
                function: 1,
                ordinal: 0,
                block: BlockId(73),
                operation: 0,
            },
        },
    )
}
fn op(m: &mut Module, f: usize, i: usize) -> &mut Operation {
    &mut m.functions[f].body.as_mut().unwrap().blocks[0].operations[i]
}
#[test]
fn transport_roles_exact_positional_graph_and_two_programs() {
    for edited in [false, true] {
        let (m, s) = fixture(edited);
        let rows = project(&m, s, edited).unwrap();
        assert_eq!(rows.len, if edited { 36 } else { 37 });
        assert!(rows.slice().contains(&[CALL_ARGUMENT, 0, 3, 91]));
        assert!(rows.slice().contains(&[PROGRAM_INPUT, 2, 28, 1]));
        assert!(rows.slice().contains(&[RETURN_VALUE, 49, 51, 1]));
        assert!(rows.slice().contains(&[STORE_DATA, 51, 62, NONE]));
        assert!(rows.len + 5 < ROW_CAP);
        assert!(serde_json::to_vec(rows.slice()).unwrap().len() < JSON_CAP);
    }
}
#[test]
fn transport_roles_distinct_ssa_argument_permutation_refuses_without_values() {
    let (mut m, s) = fixture(false);
    let OperationKind::Call { arguments, .. } = &mut op(&mut m, 0, 0).kind else {
        unreachable!()
    };
    arguments.swap(0, 1);
    assert_eq!(
        project(&m, s, false).unwrap_err(),
        "transport positional call argument ancestry"
    );
    // No concrete sample values exist here; equal runtime values cannot mask IDs.
}
#[test]
fn transport_roles_foreign_callee_and_coordinate_refuse() {
    let (mut m, s) = fixture(false);
    let OperationKind::Call { callee, .. } = &mut op(&mut m, 0, 0).kind else {
        unreachable!()
    };
    *callee = FunctionId::new("foreign");
    assert!(project(&m, s, false).is_err());
    let (m, mut s) = fixture(false);
    s.definition.block = BlockId(99);
    assert_eq!(
        project(&m, s, false).unwrap_err(),
        "transport mismatched block ID"
    );
    s.definition.function = 0;
    assert_eq!(
        project(&m, s, false).unwrap_err(),
        "transport foreign selected site"
    );
}
#[test]
fn transport_roles_program_input_and_return_substitution_refuse() {
    let (mut m, s) = fixture(false);
    let OperationKind::Gfx942OrderedProgram(p) = &op(&mut m, 1, 0).kind else {
        unreachable!()
    };
    let next = Gfx942OrderedProgramV1::new(
        p.source(),
        p.registers(),
        [ValueId(17), ValueId(91), ValueId(28)],
        *p.program(),
    )
    .unwrap();
    op(&mut m, 1, 0).kind = OperationKind::Gfx942OrderedProgram(next);
    assert_eq!(
        project(&m, s, false).unwrap_err(),
        "transport positional formal/program ancestry"
    );
    let (mut m, s) = fixture(false);
    m.functions[1].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(91)],
    });
    assert_eq!(
        project(&m, s, false).unwrap_err(),
        "transport program/return DATA ancestry"
    );
}
#[test]
fn transport_roles_store_data_and_type_substitution_refuse() {
    let (mut m, s) = fixture(false);
    let OperationKind::Store { value, .. } = &mut op(&mut m, 0, 2).kind else {
        unreachable!()
    };
    *value = ValueId(3);
    assert_eq!(
        project(&m, s, false).unwrap_err(),
        "transport call/store DATA ancestry"
    );
    let (mut m, s) = fixture(false);
    op(&mut m, 0, 0).results[0].ty = Type::BOOL;
    assert_eq!(
        project(&m, s, false).unwrap_err(),
        "transport result is not u32"
    );
}
#[test]
fn transport_roles_unrepresented_identity_cast_and_duplicate_return_refuse() {
    let (mut m, s) = fixture(false);
    let mut extra = m.functions[1].body.as_ref().unwrap().blocks[0].clone();
    extra.id = BlockId(74);
    extra.operations.clear();
    m.functions[1].body.as_mut().unwrap().blocks.push(extra);
    assert_eq!(
        project(&m, s, false).unwrap_err(),
        "transport unique helper return"
    );
    let (mut m, s) = fixture(false);
    // A real u32 -> u64 -> u32 round-trip is deliberately outside this first
    // direct-SSA transport language; do not infer a new allowed ancestry edge.
    let body = m.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.insert(
        2,
        Operation::effect_free(
            ValueDef::new(ValueId(99), Type::Scalar(ScalarType::U64)),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::ZeroExtend,
                value: ValueId(51),
                to: Type::Scalar(ScalarType::U64),
            },
        ),
    );
    body.blocks[0].operations.insert(
        3,
        Operation::effect_free(
            ValueDef::new(ValueId(100), U32),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::Truncate,
                value: ValueId(99),
                to: U32,
            },
        ),
    );
    let OperationKind::Store { value, .. } = &mut op(&mut m, 0, 4).kind else {
        unreachable!()
    };
    *value = ValueId(100);
    assert!(project(&m, s, false).is_err());
}
#[test]
fn transport_roles_wrong_descriptor_profile_and_readonly_output_refuse() {
    let (m, s) = fixture(true);
    assert_eq!(
        project(&m, s, false).unwrap_err(),
        "transport exact authored descriptors"
    );
    let (mut m, s) = fixture(false);
    m.functions[0].signature.parameters[0] =
        Type::slice(U32, AddressSpace::Global, AccessMode::ReadOnly);
    assert_eq!(
        project(&m, s, false).unwrap_err(),
        "transport output is readonly"
    );
}
#[test]
fn transport_roles_claim_count_order_and_value_are_exact() {
    let (m, s) = fixture(false);
    let a = project(&m, s, false).unwrap();
    let mut b = a.clone();
    b.values.swap(1, 2);
    assert!(matches_claim(&a, &b).is_err());
    b = a.clone();
    b.len -= 1;
    assert!(matches_claim(&a, &b).is_err());
    b = a.clone();
    b.push([IDENTITY, 63, 0, 0]).unwrap();
    assert!(matches_claim(&a, &b).is_err());
    b = a.clone();
    b.values[1][2] ^= 1;
    assert!(matches_claim(&a, &b).is_err());
}
#[test]
fn transport_roles_row_and_selector_bounds() {
    assert_eq!(std::mem::size_of::<[u32; 4]>(), 16);
    let mut rows = Rows::new();
    for _ in 0..ROW_CAP {
        rows.push([IDENTITY, 0, 0, 0]).unwrap();
    }
    assert!(rows.push([IDENTITY, 0, 0, 0]).is_err());
    for variant in ["copy", "preserve", "edit"] {
        assert!(case(variant).is_ok());
    }
    for variant in ["original", "", "COPY", "../copy"] {
        assert!(case(variant).is_err());
    }
    assert!(checks::normal_case("copy", MODE).is_err()); // old domain unchanged
}
#[test]
fn transport_roles_resource_exact_and_prefix_stay_charged() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let mut work = Work::new(ROLE_WORK + 7);
    work.charge_work(7).unwrap();
    let mut ledger = Ledger::new(work, 11 + ROLE_STORAGE + 19);
    ledger.with_budget(|b| {
        b.reserve_storage(11).unwrap();
        prepay(b, 11, 19).unwrap();
        assert_eq!(b.storage(), 11 + ROLE_STORAGE + 19);
    });
    assert_eq!(ledger.work(), ROLE_WORK + 7);
    assert_eq!(ledger.storage(), 11 + ROLE_STORAGE + 19);
    assert_eq!(ledger.peak_storage(), ledger.storage());
}
#[test]
fn transport_roles_resource_one_short_retains_denials_and_floor() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let mut ledger = Ledger::new(Work::new(ROLE_WORK), 11 + ROLE_STORAGE + 19 - 1);
    ledger.with_budget(|b| {
        b.reserve_storage(11).unwrap();
        assert!(prepay(b, 11, 19).is_err());
    });
    assert_eq!(ledger.storage(), 11);
    assert_eq!(ledger.failed_storage(), Some(11 + ROLE_STORAGE + 19));
    assert_eq!(ledger.work(), ROLE_WORK);
    let mut ledger = Ledger::new(Work::new(ROLE_WORK - 1), ROLE_STORAGE);
    ledger.with_budget(|b| assert!(prepay(b, 0, 0).is_err()));
    assert_eq!(ledger.storage(), 0);
    assert_eq!(ledger.failed_work(), Some(ROLE_WORK));
}
#[test]
fn transport_roles_resource_floor_and_byte_cap_fail_without_allocation() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    for (floor, bytes) in [(1, 0), (0, FILE_CAP + 1)] {
        let mut ledger = Ledger::new(Work::new(ROLE_WORK), ROLE_STORAGE + FILE_CAP + 1);
        ledger.with_budget(|b| assert!(prepay(b, floor, bytes).is_err()));
        assert_eq!(ledger.storage(), 0);
        assert_eq!(ledger.work(), ROLE_WORK);
    }
}

#[test]
fn transport_roles_descriptor_components_refuse_padding_order_and_width() {
    use fe2o3_kernel_descriptor::{PhysicalAbiComponentKind as K, ScalarTypeV1 as S};
    let pointer = (K::GlobalPointer, 0, 8, 8);
    let length = (K::SliceLengthU64, 8, 8, 8);
    assert!(check_components(0, [pointer, length].into_iter()).is_ok());
    for entries in [
        vec![length, pointer],
        vec![pointer],
        vec![pointer, length, pointer],
    ] {
        assert!(check_components(0, entries.into_iter()).is_err());
    }
    assert!(check_components(1, [(K::ScalarByValue(S::U32), 16, 4, 4)].into_iter()).is_ok());
    for row in [
        (K::ScalarByValue(S::U32), 28, 4, 4),
        (K::ScalarByValue(S::U32), 16, 8, 4),
        (K::ScalarByValue(S::U32), 16, 4, 8),
    ] {
        assert!(check_components(1, [row].into_iter()).is_err());
    }
    assert!(check_components(4, [(K::ScalarByValue(S::U32), 28, 4, 4)].into_iter()).is_err());
}

fn inert_publication() -> (Value, Value) {
    let snapshot = json!({"bytes":7,"sha256":"a","device":2,"inode":3});
    let value = json!({"case":"copy","actual_extractor":true,"exit":0,
        "source_custody_from_files":false,"hardware_observed":false,"protected_authority":false,
        "result":{"stage":"actual_cli_published","candidate":snapshot,
            "report":{"schema":"fe2o3-ordered-composition-diagnostic-v1","source_promotion":{
                "created_new":true,"original_overwritten":false,"fresh_compilation_required":true,
                "grants_artifact_or_launch_authority":false,"source_custody_exported":false,
                "program_edited":false,"request_sha256":"b","candidate_bytes":7,
                "candidate_sha256":"a","candidate_device":2,"candidate_inode":3}}}});
    (value, snapshot)
}
#[test]
fn transport_roles_publication_snapshot_and_request_substitutions_refuse() {
    let (record, snapshot) = inert_publication();
    publication_fields(&record, &snapshot, "b", "copy").unwrap();
    for field in ["bytes", "sha256", "device", "inode"] {
        let mut changed = snapshot.clone();
        changed[field] = json!("foreign");
        assert!(publication_fields(&record, &changed, "b", "copy").is_err());
    }
    assert!(publication_fields(&record, &snapshot, "stale", "copy").is_err());
    assert!(publication_fields(&record, &snapshot, "b", "edit").is_err());
}
#[test]
fn transport_roles_publication_does_not_upgrade_claimed_authority() {
    let (record, snapshot) = inert_publication();
    for field in [
        "grants_artifact_or_launch_authority",
        "source_custody_exported",
        "original_overwritten",
    ] {
        let mut changed = record.clone();
        changed["result"]["report"]["source_promotion"][field] = json!(true);
        assert!(publication_fields(&changed, &snapshot, "b", "copy").is_err());
    }
    let mut changed = record;
    changed["result"]["report"]["source_promotion"]["candidate_inode"] = json!(9);
    assert!(publication_fields(&changed, &snapshot, "b", "copy").is_err());
}
