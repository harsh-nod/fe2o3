#![cfg(test)]
//! Component encoder/ledger checks. No authenticated target, Request, execution,
//! collector receipt, production FinalChain, or native authority is constructed.
use super::*;
use fe2o3_kernel_descriptor::{
    decode_device_descriptor_table_v3, decode_device_descriptor_table_v5,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[path = "compiler_descriptor_conditional_native_v5_fixture_tests.rs"]
mod fixture;

const LIMIT: usize = 64 * 1024 * 1024;
const FLOOR: usize = 173;
const PRIOR: usize = 29;
const PROFILES: [Profile; 2] = [Profile::Gfx942, Profile::Gfx950];
fn free(_: usize) -> Result<(), Resource> {
    Ok(())
}

struct Model {
    semantic: Semantic,
    roots: Vec<TypedDescriptorRootV1>,
    owner: Owner,
    contracts: Vec<Vec<u8>>,
    profile: Profile,
}
impl Model {
    fn new(count: usize, profile: Profile) -> Self {
        let semantic = fixture::source(count);
        let roots = fixture::roots(&semantic);
        let owner = fixture::admit(&fixture::module(count, profile));
        let contracts = fixture::contract_bytes(&semantic, profile, |_| {});
        Self {
            semantic,
            roots,
            owner,
            contracts,
            profile,
        }
    }
    fn encode(&self, budget: &mut Budget<'_>) -> R<Output> {
        // Decoded backing/owners belong to fixture setup, not fresh production
        // custody. The public entry additionally requires the original target.
        let contracts = self
            .contracts
            .iter()
            .map(|bytes| decode_conditional_invocation_contract_v2(bytes, &mut free).unwrap())
            .collect::<Vec<_>>();
        scope(budget, |b| {
            encode_rows(
                &self.roots,
                &self.semantic,
                &self.owner,
                self.profile,
                64,
                &contracts,
                b,
            )
        })
    }
    fn run(
        &self,
        work: usize,
        storage: usize,
    ) -> (R<Output>, usize, usize, usize, Option<usize>, Option<usize>) {
        let mut work = Work::new(work);
        let mut b = Budget::new(&mut work, storage);
        b.reserve_storage(FLOOR).unwrap();
        b.charge_work(PRIOR).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = self.encode(&mut b);
        assert!(b.work_ledger_identity_v1() == ledger);
        (
            result,
            b.work(),
            b.storage(),
            b.peak_storage(),
            b.failed_work(),
            b.failed_storage(),
        )
    }
    fn bytes(&self) -> Vec<u8> {
        let (result, _, current, _, _, _) = self.run(LIMIT, LIMIT);
        assert_eq!(current, FLOOR);
        let (bytes, receipt) = result.unwrap();
        assert_eq!(
            receipt.retained_storage(),
            size_of::<Vec<u8>>() + bytes.capacity()
        );
        bytes
    }
}

#[test]
fn conditional_descriptor_v5_component_both_profiles_complete_ordered_rows() {
    for profile in PROFILES {
        for count in [1, 2, 4] {
            let mut model = Model::new(count, profile);
            let first = model.bytes();
            let table = decode_device_descriptor_table_v5(&first, &mut free).unwrap();
            assert_eq!(table.kernel_count(), count);
            assert_eq!(table.producer_version(), PRODUCER);
            assert_eq!(table.canonical_code_object_digest().as_bytes(), &[0; 32]);
            assert!(decode_device_descriptor_table_v3(&first, &mut free).is_err());
            for i in 0..count {
                let row = table.kernel(i, &mut free).unwrap();
                assert_eq!(row.kernel_id().as_bytes(), &[13 + i as u8; 32]);
                let contract = row.conditional_contract(&mut free).unwrap();
                assert_eq!(contract.canonical_bytes(), model.contracts[i]);
                assert_eq!(
                    &contract.subjects().source_semantic_identity,
                    model.semantic.semantic_sha256().as_bytes()
                );
            }
            model.roots.reverse();
            model.contracts.rotate_left(count / 2);
            assert_eq!(first, model.bytes());
        }
    }
}

#[test]
fn conditional_descriptor_v5_component_exact_and_one_short_resources() {
    for profile in PROFILES {
        let model = Model::new(2, profile);
        let (result, work, current, peak, failed_work, failed_storage) = model.run(LIMIT, LIMIT);
        let expected = result.unwrap().0;
        assert_eq!(current, FLOOR);
        assert_eq!((failed_work, failed_storage), (None, None));
        let (exact, used, current, observed, _, _) = model.run(work, peak);
        assert_eq!(exact.unwrap().0, expected);
        assert_eq!((used, current, observed), (work, FLOOR, peak));
        let (short, _, current, _, denied, _) = model.run(work - 1, peak);
        assert!(short.is_err());
        assert!(denied.is_some());
        assert!(current >= FLOOR + HEADER);
        let (short, _, current, _, _, denied) = model.run(work, peak - 1);
        assert!(short.is_err());
        assert_eq!(denied, Some(peak));
        assert!(current >= FLOOR + HEADER);
    }
}

#[test]
fn conditional_descriptor_v5_component_binding_source_and_count_substitutions() {
    for mutation in 0..6 {
        let mut model = Model::new(2, Profile::Gfx942);
        match mutation {
            0 => {
                model.contracts.pop();
            }
            1 => model.contracts[1] = model.contracts[0].clone(),
            2 => {
                model.contracts = fixture::contract_bytes(&model.semantic, model.profile, |f| {
                    f.subjects.kernel_id = [99; 32]
                })
            }
            3 => {
                model.contracts = fixture::contract_bytes(&model.semantic, model.profile, |f| {
                    f.subjects.source_semantic_identity = [99; 32]
                })
            }
            4 => model.roots[1] = model.roots[0].clone(),
            5 => {
                model.roots[0].kernel_binding =
                    reserved_fe2o3_symbols::KernelBindingIdV1::from_bytes([99; 32])
            }
            _ => unreachable!(),
        }
        assert!(
            model.run(LIMIT, LIMIT).0.is_err(),
            "accepted mutation {mutation}"
        );
    }
}

#[test]
fn conditional_descriptor_v5_component_missing_original_nominal_layout_is_required() {
    let mut model = Model::new(1, Profile::Gfx942);
    let _baseline = model.bytes();
    let mut arguments = model.roots[0].arguments.as_slice().to_vec();
    arguments[0].layout = None;
    model.roots[0].arguments = crate::collector::TypedArgumentListV1::new(arguments).unwrap();
    assert!(
        model.run(LIMIT, LIMIT).0.is_err(),
        "accepted missing original layout evidence"
    );
}

#[test]
fn conditional_descriptor_v5_component_rejects_valid_but_wrong_layout_donors() {
    use fe2o3_artifacts::{PointerWidth, RustScalarElementTypeV1 as Scalar};
    for profile in PROFILES {
        let mut model = Model::new(2, profile);
        let _baseline = model.bytes();
        for root in 0..model.roots.len() {
            let original = model.roots[root].arguments.clone();
            for argument in 0..original.len() {
                let output = argument == 2;
                for donor in [
                    fixture::slice_layout(!output, Scalar::F32, PointerWidth::Bits64),
                    fixture::slice_layout(output, Scalar::F64, PointerWidth::Bits64),
                    fixture::slice_layout(output, Scalar::F32, PointerWidth::Bits32),
                ] {
                    // These are constructor-checked Some values, not malformed
                    // evidence or a fabricated source/proof receipt.
                    let mut arguments = original.as_slice().to_vec();
                    assert_ne!(arguments[argument].layout.as_ref(), Some(&donor));
                    arguments[argument].layout = Some(donor);
                    model.roots[root].arguments =
                        crate::collector::TypedArgumentListV1::new(arguments).unwrap();
                    assert!(matches!(
                        model.run(LIMIT, LIMIT).0,
                        Err(E::Nominal(nominal_v3::NominalDescriptorErrorV3::Descriptor(
                            crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                                "whole-root source and physical layout join"
                            )
                        )))
                    ));
                }
            }
            model.roots[root].arguments = original;
        }
    }
}

#[test]
fn conditional_descriptor_v5_component_original_layout_checks_prepaid_work() {
    let mut model = Model::new(1, Profile::Gfx942);
    let mut arguments = model.roots[0].arguments.as_slice().to_vec();
    arguments[0].layout = None;
    model.roots[0].arguments = crate::collector::TypedArgumentListV1::new(arguments).unwrap();
    for limit in [
        ORIGINAL_LAYOUT_ARGUMENT_WORK - 1,
        ORIGINAL_LAYOUT_ARGUMENT_WORK,
    ] {
        let mut work = Work::new(PRIOR + limit);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        budget.charge_work(PRIOR).unwrap();
        let account = budget.work_ledger_identity_v1();
        let result = check_original_layouts(&model.roots, &mut budget);
        if limit < ORIGINAL_LAYOUT_ARGUMENT_WORK {
            assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
            assert_eq!(
                budget.failed_work(),
                Some(PRIOR + ORIGINAL_LAYOUT_ARGUMENT_WORK)
            );
            assert_eq!(budget.work(), PRIOR);
        } else {
            assert!(matches!(
                result,
                Err(E::Nominal(nominal_v3::NominalDescriptorErrorV3::Descriptor(
                    crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                        "whole-root Rust layout evidence"
                    )
                )))
            ));
            assert_eq!(budget.failed_work(), None);
            assert_eq!(budget.work(), PRIOR + ORIGINAL_LAYOUT_ARGUMENT_WORK);
        }
        assert!(budget.work_ledger_identity_v1() == account);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR);
    }
}

#[test]
fn conditional_descriptor_v5_component_original_layout_keeps_nominal_none_exception() {
    use crate::compiler_descriptor::AccessMode;
    use fe2o3_artifacts::RustcAbiClassV1;
    let semantic = fixture::source(1);
    let mut roots = fixture::roots(&semantic);
    for kind in [
        DescriptorArgumentKindV1::CompilerLaidOutUsize,
        DescriptorArgumentKindV1::CompilerLaidOutIsize,
    ] {
        let mut arguments = roots[0].arguments.as_slice().to_vec();
        let donor = arguments[1].layout.clone();
        arguments[0].kind = kind;
        arguments[0].access = AccessMode::ByValue;
        arguments[0].source_size = 8;
        arguments[0].rustc_abi_class = RustcAbiClassV1::Scalar;
        arguments[0].layout = None;
        roots[0].arguments = crate::collector::TypedArgumentListV1::new(arguments.clone()).unwrap();
        // Only the physical preflight is under test: the unchanged semantic
        // fixture is not claimed to authenticate this changed nominal argument.
        let mut work = Work::new(3 * ORIGINAL_LAYOUT_ARGUMENT_WORK);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let account = budget.work_ledger_identity_v1();
        check_original_layouts(&roots, &mut budget).unwrap();
        assert_eq!(budget.work(), 3 * ORIGINAL_LAYOUT_ARGUMENT_WORK);
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work_ledger_identity_v1() == account);
        arguments[0].layout = donor;
        roots[0].arguments = crate::collector::TypedArgumentListV1::new(arguments).unwrap();
        let mut work = Work::new(ORIGINAL_LAYOUT_ARGUMENT_WORK);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(matches!(
            check_original_layouts(&roots, &mut budget),
            Err(E::Nominal(nominal_v3::NominalDescriptorErrorV3::Descriptor(
                crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                    "nominal pointer-sized argument has no fixed source layout"
                )
            )))
        ));
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn conditional_descriptor_v5_component_original_nominal_layout_is_required() {
    use crate::{collector::TypedArgumentListV1, compiler_descriptor::AccessMode};
    use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1;
    for mutation in 1..9 {
        let mut model = Model::new(1, Profile::Gfx942);
        let mut arguments = model.roots[0].arguments.as_slice().to_vec();
        match mutation {
            1 => arguments[1].offset += 8,
            2 => arguments[0].source_size = 8,
            3 => arguments[0].source_alignment = 4,
            4 => {
                arguments[0].semantic_type_identity = SemanticTypeIdentityV1::from_sha256([99; 32])
            }
            5 => arguments[2].access = AccessMode::ReadOnly,
            6 => {
                arguments.pop();
            }
            7 => arguments[0].rustc_abi_class = fe2o3_artifacts::RustcAbiClassV1::Aggregate,
            8 => model.roots[0].explicit_argument_bytes += 8,
            _ => unreachable!(),
        }
        model.roots[0].arguments = TypedArgumentListV1::new(arguments).unwrap();
        assert!(
            model.run(LIMIT, LIMIT).0.is_err(),
            "accepted nominal mutation {mutation}"
        );
    }
}

#[test]
fn conditional_descriptor_v5_component_f_target_entry_signature_and_launch() {
    use fe2o3_kernel_ir::{AccessMode, AddressSpace, TargetCapability, WorkgroupSize};
    let semantic = fixture::source(1);
    let roots = fixture::roots(&semantic);
    for mutation in 0..7 {
        let mut module = fixture::module(1, Profile::Gfx942);
        match mutation {
            0 => {
                module.required_capabilities.clear();
            }
            1 => {
                module.kernels[0].required_capabilities.clear();
            }
            2 => {
                module.functions[0].required_capabilities.clear();
            }
            3 => {
                module.functions[0]
                    .required_capabilities
                    .insert(TargetCapability::Extension {
                        namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.into(),
                        name: Profile::Gfx950.device_target().into(),
                    });
            }
            4 => module.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1)),
            5 => {
                module.functions[0].signature.parameters[0] = Type::slice(
                    Type::Scalar(ScalarType::F32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                )
            }
            6 => module.kernels[0].entry = "other".into(),
            _ => unreachable!(),
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(
            check_output(&roots, &module, Profile::Gfx942, &mut budget).is_err(),
            "accepted F mutation {mutation}"
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn conditional_descriptor_v5_component_scope_err_unwind_and_floor_tampering_are_terminal() {
    for mutation in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(FLOOR).unwrap();
        b.charge_work(PRIOR).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            scope(&mut b, |b| {
                b.charge_work(7)?;
                let bytes = nominal_v3::vector::<u8>(31, b)?;
                let receipt =
                    ConditionalNativeDescriptorStorageV5(size_of::<Vec<u8>>() + bytes.capacity());
                match mutation {
                    0 => Err(E::Mismatch("terminal component error")),
                    1 => panic!("component unwind"),
                    _ => {
                        b.release_storage(b.storage())?;
                        Ok((bytes, receipt))
                    }
                }
            })
        }));
        match mutation {
            0 => assert!(matches!(
                result.unwrap(),
                Err(E::Mismatch("terminal component error"))
            )),
            1 => assert!(result.is_err()),
            _ => assert!(matches!(
                result.unwrap(),
                Err(E::Resource(Resource::Accounting))
            )),
        }
        assert!(b.work() > PRIOR);
        if mutation != 2 {
            assert!(b.storage() >= FLOOR + HEADER);
        }
    }
}

#[test]
fn conditional_descriptor_v5_component_scope_denial_history_and_unreserved_transfer() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(FLOOR).unwrap();
    assert!(b.reserve_storage(LIMIT).is_err());
    let denial = b.failed_storage();
    let (bytes, receipt) = scope(&mut b, |b| {
        let bytes = nominal_v3::vector::<u8>(31, b)?;
        let receipt = ConditionalNativeDescriptorStorageV5(size_of::<Vec<u8>>() + bytes.capacity());
        Ok((bytes, receipt))
    })
    .unwrap();
    assert_eq!(b.storage(), FLOOR);
    assert_eq!(b.failed_storage(), denial);
    b.reserve_storage(receipt.retained_storage()).unwrap();
    drop(bytes);
    b.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(b.storage(), FLOOR);
}

#[test]
fn conditional_descriptor_v5_component_shared_rows_preserve_ordinary_v3_recipe() {
    use fe2o3_kernel_descriptor::{
        DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3, encode_device_descriptor_table_v3,
        encoded_device_descriptor_table_v3_len,
    };
    for profile in PROFILES {
        let model = Model::new(2, profile);
        let run = |common: bool, work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut b = Budget::new(&mut work, storage_limit);
            b.reserve_storage(FLOOR).unwrap();
            b.charge_work(PRIOR).unwrap();
            let result = if !common {
                nominal_v3::encode_subject(
                    &model.roots,
                    &model.semantic,
                    model.owner.module(),
                    model.owner.canonical().canonical_bytes(),
                    model.roots.len(),
                    profile,
                    64,
                    b"ordinary-component",
                    "ordinary-component",
                    &mut b,
                )
            } else {
                // The old enclosing scope and encoder recipe remain unchanged.
                // This component count is a test roster, NOT conditional proof.
                nominal_v3::scoped(&mut b, |b| {
                    b.charge_work(5)?;
                    nominal_v3::with_subject_rows(
                        &model.roots,
                        &model.semantic,
                        model.owner.module(),
                        model.owner.canonical().canonical_bytes(),
                        profile,
                        64,
                        b"ordinary-component",
                        "ordinary-component",
                        b,
                        |input, b| {
                            b.reserve_storage(DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3)?;
                            let length = encoded_device_descriptor_table_v3_len(&input, &mut |w| {
                                b.charge_work(w)
                            })
                            .map_err(nominal_v3::NominalDescriptorErrorV3::Wire)?;
                            let mut bytes = nominal_v3::vector::<u8>(length, b)?;
                            b.charge_work(length)?;
                            bytes.resize(length, 0);
                            encode_device_descriptor_table_v3(&input, &mut bytes, &mut |w| {
                                b.charge_work(w)
                            })
                            .map_err(nominal_v3::NominalDescriptorErrorV3::Wire)?;
                            Ok(bytes)
                        },
                    )?
                })
            };
            (
                result.map_err(|e| format!("{e:?}")),
                b.work(),
                b.storage(),
                b.peak_storage(),
                b.failed_work(),
                b.failed_storage(),
            )
        };
        let measured = run(false, LIMIT, LIMIT);
        assert!(measured.0.is_ok());
        assert_eq!(measured, run(true, LIMIT, LIMIT));
        for (work, storage) in [
            (measured.1, measured.3),
            (measured.1 - 1, measured.3),
            (measured.1, measured.3 - 1),
        ] {
            assert_eq!(run(false, work, storage), run(true, work, storage));
        }
    }
}
