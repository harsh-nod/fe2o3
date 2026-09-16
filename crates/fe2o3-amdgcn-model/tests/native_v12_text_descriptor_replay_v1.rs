//! Actual target binding, policy-3 O, native text and inert descriptor relation.
//! These tests do not manufacture compiler/source/proof or execution authority.

use std::{fmt::Write, mem::size_of, ptr};

use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_amdgcn_model::{
    NativeV12TextDescriptorReplayErrorV1 as E,
    ReplayedNativeV12TextDescriptorRelationV1 as Relation, bind_production_llvm22_worker_layout_v1,
    bind_production_target_v1, check_native_v12_text_descriptor_relation_v1 as check,
    lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_942,
    lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_950,
};
use fe2o3_kernel_descriptor::{
    BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CodeObjectVersion, CompilerIdentityV1,
    DeviceDescriptorTableV1 as Table, DeviceTargetV1, DimensionsV1, EvidenceDigest,
    EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1, KernelId as DescriptorId,
    LaunchConstraintsV1, ProducerIdentityV1, Text, ValidName, encode_device_descriptor_table_v1,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    ComparePredicate, Constant, Function, InertCanonicalKernelIrContractCatalogV1 as Catalog,
    IntrinsicOperation, Kernel, KernelIrPipelineContractDefinitionV1,
    KernelIrPipelineStorageBindingV1, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation,
    OperationKind, Signature, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner, WorkgroupSize,
};
use fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1;

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 1_000_000_000;
const PRIOR: usize = 7;
// Conservative test-input envelope for these small raw modules, descriptor
// tables and formatting-oracle buffers, separate from exact B/O/catalog receipts.
const FIXTURE: usize = 1024 * 1024;

fn diamond(name: &str) -> Function {
    let expression = |id| {
        Operation::effect_free(
            ValueDef::new(ValueId(id), Type::INDEX),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        )
    };
    let store = |id| {
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(id),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        )
    };
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(1),
                Type::pointer(Type::INDEX, AddressSpace::Private, AccessMode::ReadWrite),
            ),
            OperationKind::Alloca {
                element: Type::INDEX,
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            OperationKind::Constant(Constant::Index(63)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::INDEX),
            OperationKind::Constant(Constant::Index(32)),
        ),
        expression(5),
        store(5),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(4),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(30),
        then_arguments: vec![],
        else_target: BlockId(70),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(30));
    left.operations = vec![expression(7), store(7)];
    left.terminator = Some(Terminator::Branch {
        target: BlockId(90),
        arguments: vec![],
    });
    let mut right = BasicBlock::new(BlockId(70));
    right.operations = vec![expression(8), store(8)];
    right.terminator = Some(Terminator::Branch {
        target: BlockId(90),
        arguments: vec![],
    });
    let mut join = BasicBlock::new(BlockId(90));
    join.terminator = Some(Terminator::Return { values: vec![] });
    Function::kernel_entry(
        name,
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry, right, left, join],
    )
}

fn source(multiple: bool) -> Module {
    let mut module = Module::new("native-text-replay");
    module.functions.push(diamond("body_z"));
    let mut kernel = Kernel::new(
        "zeta",
        "body_z",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(128),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    if multiple {
        module.functions.insert(0, diamond("body_a"));
        let mut kernel = Kernel::new(
            "alpha",
            "body_a",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(128),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
    }
    module
}

fn with_actual(
    profile: Profile,
    multiple: bool,
    body: impl FnOnce(&Owner, &Owner, &Catalog, &mut Budget<'_>),
) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(PRIOR).unwrap();
    budget.reserve_storage(FIXTURE).unwrap();
    let raw = source(multiple);
    let bound = bind_production_target_v1(&raw, profile).unwrap();
    let (input, input_storage) =
        Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget).unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    let checked = optimize_checked_canonical_kernel_ir_policy3_v1(&input, &mut budget).unwrap();
    let checked_storage = checked.storage().retained_storage();
    budget.reserve_storage(checked_storage).unwrap();
    assert_eq!(
        checked.native_input_audit_bytes(),
        input.canonical().canonical_bytes()
    );
    assert_ne!(
        checked.owner().canonical().canonical_bytes(),
        input.canonical().canonical_bytes()
    );
    let census = |owner: &Owner| {
        owner
            .module()
            .functions
            .iter()
            .flat_map(|f| &f.body.as_ref().unwrap().blocks)
            .flat_map(|b| &b.operations)
            .filter(|op| {
                matches!(
                    op.kind,
                    OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        ..
                    }
                )
            })
            .count()
    };
    assert_eq!(census(&input), 3 * (1 + usize::from(multiple)));
    assert_eq!(census(checked.owner()), 1 + usize::from(multiple));
    let (catalog, catalog_storage) =
        Catalog::from_rows_with_budget([7; 32], &[], &[], &mut budget).unwrap();
    budget
        .reserve_storage(catalog_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    body(&input, checked.owner(), &catalog, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(catalog);
    budget
        .release_storage(catalog_storage.retained_storage())
        .unwrap();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(input);
    budget
        .release_storage(input_storage.retained_storage())
        .unwrap();
    drop(bound);
    drop(raw);
    assert_eq!(budget.storage(), FIXTURE);
    budget.release_storage(FIXTURE).unwrap();
    assert_eq!(budget.storage(), 0);
}

fn table(
    profile: Profile,
    entries: &[(&str, &str)],
    version: CodeObjectVersion,
    digest: u8,
    producer: &str,
) -> Table {
    let kernels = entries
        .iter()
        .enumerate()
        .map(|(index, &(entry, symbol))| {
            let evidence = BuildEvidenceV1::new(
                EvidenceIdentity::from_opaque_bytes([1; 32]),
                EvidenceDigest::from_sha256_bytes([2; 32]),
            );
            KernelDescriptorV1::new(
                DescriptorId::from_bytes([u8::try_from(index + 1).unwrap(); 32]),
                ValidName::new(format!("logical_{entry}")).unwrap(),
                ValidName::new(entry).unwrap(),
                ValidName::new(symbol).unwrap(),
                evidence,
                evidence,
                vec![],
                KernelAbiLayoutV1::new(0, 0, 8).unwrap(),
                LaunchConstraintsV1::new(
                    1,
                    BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
                    DimensionsV1::new(2, 1, 1).unwrap(),
                    64,
                    0,
                    0,
                )
                .unwrap(),
                vec![],
            )
            .unwrap()
        })
        .collect();
    Table::new(
        CanonicalCodeObjectDigest::from_bytes([digest; 32]),
        version,
        CompilerIdentityV1::new(
            Text::new("inert-test").unwrap(),
            Text::new("1").unwrap(),
            [0; 20],
        ),
        ProducerIdentityV1::new(
            Text::new("inert-test").unwrap(),
            Text::new(producer).unwrap(),
        ),
        DeviceTargetV1::parse(profile.device_target()).unwrap(),
        vec![],
        vec![],
        kernels,
    )
    .unwrap()
}

fn normal_table(profile: Profile, multiple: bool) -> Table {
    let entries: &[(&str, &str)] = if multiple {
        &[("alpha", "alpha.kd"), ("zeta", "zeta.kd")]
    } else {
        &[("zeta", "zeta.kd")]
    };
    table(profile, entries, CodeObjectVersion::V6, 0, "1")
}

fn prefix(output: &Owner, profile: Profile) -> String {
    let native = match profile {
        Profile::Gfx942 => lower_942(output),
        Profile::Gfx950 => lower_950(output),
    }
    .unwrap();
    bind_production_llvm22_worker_layout_v1(&native).unwrap()
}

// Independent formatting oracle mirrors the real compiler suffix, not the
// new byte matcher; its bytes are inert inputs, not source ABI evidence.
fn final_text(prefix: &str, table: &Table) -> String {
    let bytes = encode_device_descriptor_table_v1(table).unwrap();
    let mut text = prefix.to_owned();
    text.push_str(
        "\nmodule asm \".section .fe2o3.kd.v1,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n",
    );
    for chunk in bytes.chunks(16) {
        text.push_str("module asm \".byte ");
        for (ordinal, byte) in chunk.iter().enumerate() {
            if ordinal != 0 {
                text.push_str(", ");
            }
            write!(text, "0x{byte:02x}").unwrap();
        }
        text.push_str("\"\n");
    }
    text
}

fn accept(
    output: &Owner,
    catalog: &Catalog,
    profile: Profile,
    descriptors: &Table,
    llvm: &str,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let previous = budget.work();
    let functions = output.module().functions.as_ptr();
    let receipt = {
        let relation = check(
            output,
            catalog,
            output.canonical().canonical_bytes(),
            profile,
            descriptors,
            llvm,
            budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        let receipt = relation.storage().retained_storage();
        assert_eq!(receipt, size_of::<Relation<'_, '_, '_, '_>>());
        budget.reserve_storage(receipt).unwrap();
        assert!(ptr::eq(relation.output(), output));
        assert!(ptr::eq(relation.catalog(), catalog));
        assert!(ptr::eq(relation.descriptors(), descriptors));
        assert!(ptr::eq(relation.final_llvm(), llvm));
        assert_eq!(relation.profile(), profile);
        assert_eq!(relation.pre_descriptor_llvm(), prefix(output, profile));
        assert!(!relation.grants_authority());
        assert_eq!(output.module().functions.as_ptr(), functions);
        assert!(budget.work() > previous);
        receipt
    };
    budget.release_storage(receipt).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn actual_changed_policy3_output_replays_on_both_profiles_without_owner_reconstruction() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(profile, false, |_, output, catalog, budget| {
            let table = normal_table(profile, false);
            let llvm = final_text(&prefix(output, profile), &table);
            assert!(llvm.contains("store i64"));
            assert!(llvm.contains("target datalayout ="));
            accept(output, catalog, profile, &table, &llvm, budget);
            // Independently admitted immutable O is sufficient for this relation;
            // no local execution witness is reconstructed or claimed.
            let (readmitted, storage) =
                Owner::from_module_ref_with_verification_budget_v12(output.module(), budget)
                    .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_eq!(
                readmitted.canonical().canonical_bytes(),
                output.canonical().canonical_bytes()
            );
            accept(&readmitted, catalog, profile, &table, &llvm, budget);
            drop(readmitted);
            budget.release_storage(storage.retained_storage()).unwrap();
        });
    }
}

#[test]
fn full_nonlexical_roster_uses_exports_not_function_or_descriptor_ordinals() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(profile, true, |_, output, catalog, budget| {
            let table = normal_table(profile, true);
            assert_eq!(output.module().kernels[0].id.as_str(), "zeta");
            assert_eq!(output.module().functions[0].id.as_str(), "body_a");
            assert_eq!(table.kernels()[0].entry_name().as_str(), "alpha");
            let text = prefix(output, profile);
            assert!(text.contains("!fe2o3.semantic_anchor.absence.v1"));
            assert!(text.contains("!\"multiple_defined_bodies\""));
            assert!(!text.contains("llvm.pseudoprobe"));
            assert!(text.contains("@zeta("));
            assert!(text.contains("@alpha("));
            accept(
                output,
                catalog,
                profile,
                &table,
                &final_text(&text, &table),
                budget,
            );
        });
    }
}

#[test]
fn pairwise_symbols_and_complete_export_roster_are_mandatory() {
    with_actual(Profile::Gfx942, true, |_, output, catalog, budget| {
        let good = normal_table(Profile::Gfx942, true);
        let llvm = final_text(&prefix(output, Profile::Gfx942), &good);
        let cases: &[(&[(&str, &str)], &str)] = &[
            (
                &[("alpha", "zeta.kd"), ("zeta", "alpha.kd")],
                "descriptor entry/symbol pair",
            ),
            (
                &[("alpha", "alpha.kd")],
                "complete descriptor/kernel roster",
            ),
            (
                &[("alpha", "alpha.kd"), ("foreign", "foreign.kd")],
                "descriptor entry bijection",
            ),
            (
                &[("alpha", "alpha.kd"), ("zeta", "zeta.bad")],
                "descriptor symbol suffix",
            ),
            (
                &[
                    ("alpha", "alpha.kd"),
                    ("zeta", "zeta.kd"),
                    ("foreign", "foreign.kd"),
                ],
                "complete descriptor/kernel roster",
            ),
        ];
        for &(entries, expected) in cases {
            let bad = table(Profile::Gfx942, entries, CodeObjectVersion::V6, 0, "1");
            let floor = budget.storage();
            assert!(
                matches!(check(output, catalog, output.canonical().canonical_bytes(), Profile::Gfx942, &bad, &llvm, budget), Err(E::Invalid(reason)) if reason == expected)
            );
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn exact_o_bytes_and_every_llvm_byte_reject_substitution_or_trailing_material() {
    with_actual(Profile::Gfx942, false, |input, output, catalog, budget| {
        let table = normal_table(Profile::Gfx942, false);
        let prefix = prefix(output, Profile::Gfx942);
        let llvm = final_text(&prefix, &table);
        let bytes = output.canonical().canonical_bytes();
        let mut changed = bytes.to_vec();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        for wire in [
            &changed[..],
            &bytes[..bytes.len() - 1],
            input.canonical().canonical_bytes(),
        ] {
            let floor = budget.storage();
            assert!(matches!(
                check(
                    output,
                    catalog,
                    wire,
                    Profile::Gfx942,
                    &table,
                    &llvm,
                    budget
                ),
                Err(E::OutputBytes)
            ));
            assert_eq!(budget.storage(), floor);
        }
        for location in [0, prefix.len() / 2, prefix.len(), llvm.len() - 1] {
            let mut altered = llvm.as_bytes().to_vec();
            altered[location] = if altered[location] == b'X' {
                b'Y'
            } else {
                b'X'
            };
            let text = String::from_utf8(altered).unwrap();
            let floor = budget.storage();
            assert!(matches!(
                check(
                    output,
                    catalog,
                    bytes,
                    Profile::Gfx942,
                    &table,
                    &text,
                    budget
                ),
                Err(E::Invalid("exact native LLVM/descriptor text"))
            ));
            assert_eq!(budget.storage(), floor);
        }
        for text in [
            format!("{llvm}\n"),
            llvm[..llvm.len() - 1].to_owned(),
            format!("; extra\n{llvm}"),
        ] {
            let floor = budget.storage();
            assert!(matches!(
                check(
                    output,
                    catalog,
                    bytes,
                    Profile::Gfx942,
                    &table,
                    &text,
                    budget
                ),
                Err(E::Invalid("complete native LLVM length"))
            ));
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn closed_profile_cov6_zero_digest_and_actual_o_target_are_independent_checks() {
    with_actual(Profile::Gfx942, false, |_, output, catalog, budget| {
        let good = normal_table(Profile::Gfx942, false);
        let llvm = final_text(&prefix(output, Profile::Gfx942), &good);
        for (profile, version, digest) in [
            (Profile::Gfx950, CodeObjectVersion::V6, 0),
            (Profile::Gfx942, CodeObjectVersion::V5, 0),
            (Profile::Gfx942, CodeObjectVersion::V6, 1),
        ] {
            let bad = table(profile, &[("zeta", "zeta.kd")], version, digest, "1");
            let floor = budget.storage();
            assert!(matches!(
                check(
                    output,
                    catalog,
                    output.canonical().canonical_bytes(),
                    Profile::Gfx942,
                    &bad,
                    &llvm,
                    budget
                ),
                Err(E::Invalid("descriptor profile/COV6/zero digest"))
            ));
            assert_eq!(budget.storage(), floor);
        }
        let wrong_target_table = normal_table(Profile::Gfx950, false);
        let floor = budget.storage();
        assert!(matches!(
            check(
                output,
                catalog,
                output.canonical().canonical_bytes(),
                Profile::Gfx950,
                &wrong_target_table,
                &llvm,
                budget
            ),
            Err(E::Lowering(_))
        ));
        assert_eq!(budget.storage(), floor);
        let raw = source(false);
        let (neutral, storage) =
            Owner::from_module_ref_with_verification_budget_v12(&raw, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(matches!(
            check(
                &neutral,
                catalog,
                neutral.canonical().canonical_bytes(),
                Profile::Gfx942,
                &good,
                &llvm,
                budget
            ),
            Err(E::Lowering(_))
        ));
        drop(neutral);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn changed_descriptor_and_exact_suffix_remain_only_a_text_relation() {
    with_actual(Profile::Gfx942, false, |_, output, catalog, budget| {
        let old = normal_table(Profile::Gfx942, false);
        let changed = table(
            Profile::Gfx942,
            &[("zeta", "zeta.kd")],
            CodeObjectVersion::V6,
            0,
            "untrusted-change",
        );
        let prefix = prefix(output, Profile::Gfx942);
        let old_text = final_text(&prefix, &old);
        let changed_text = final_text(&prefix, &changed);
        assert_ne!(old_text, changed_text);
        assert!(
            check(
                output,
                catalog,
                output.canonical().canonical_bytes(),
                Profile::Gfx942,
                &changed,
                &old_text,
                budget
            )
            .is_err()
        );
        accept(
            output,
            catalog,
            Profile::Gfx942,
            &changed,
            &changed_text,
            budget,
        );
    });
}

#[test]
fn graph_foreign_catalog_cannot_borrow_a_valid_native_text() {
    with_actual(Profile::Gfx942, false, |_, output, _, budget| {
        let definition = KernelIrPipelineContractDefinitionV1 {
            key: 0,
            semantic_pipeline_type: 1,
            semantic_payload_type: 2,
            buffers: 2,
            elements: 8,
            prefetch_distance: 1,
            packed_bits: 32,
            source_size_bytes: 4,
            source_alignment_bytes: 4,
        };
        let binding = KernelIrPipelineStorageBindingV1 {
            function: 0,
            storage: u32::MAX,
            key: 0,
            block: 0,
            operation: 0,
        };
        let (catalog, storage) =
            Catalog::from_rows_with_budget([7; 32], &[definition], &[binding], budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let table = normal_table(Profile::Gfx942, false);
        let llvm = final_text(&prefix(output, Profile::Gfx942), &table);
        let floor = budget.storage();
        assert!(matches!(
            check(
                output,
                &catalog,
                output.canonical().canonical_bytes(),
                Profile::Gfx942,
                &table,
                &llvm,
                budget
            ),
            Err(E::Catalog(_))
        ));
        assert_eq!(budget.storage(), floor);
        drop(catalog);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

#[test]
fn storage_denial_keeps_prior_floor_and_failure_history_on_same_ledger_retry() {
    with_actual(Profile::Gfx942, false, |_, output, catalog, budget| {
        let table = normal_table(Profile::Gfx942, false);
        let llvm = final_text(&prefix(output, Profile::Gfx942), &table);
        let floor = budget.storage();
        let held = STORAGE - floor;
        budget.reserve_storage(held).unwrap();
        let before = budget.work();
        assert!(matches!(
            check(
                output,
                catalog,
                output.canonical().canonical_bytes(),
                Profile::Gfx942,
                &table,
                &llvm,
                budget
            ),
            Err(E::Resource(_))
        ));
        assert_eq!(budget.storage(), STORAGE);
        assert!(budget.work() > before);
        let failure = budget.failed_storage().expect("denial must be retained");
        assert!(failure > STORAGE);
        budget.release_storage(held).unwrap();
        accept(output, catalog, Profile::Gfx942, &table, &llvm, budget);
        assert_eq!(budget.failed_storage(), Some(failure));
        assert_eq!(budget.storage(), floor);
    });
}
