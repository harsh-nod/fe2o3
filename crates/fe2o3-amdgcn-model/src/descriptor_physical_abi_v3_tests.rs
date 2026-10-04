//! Physical fixtures deliberately carry no Rust nominal or ownership authority.
use super::*;
use crate::native_v12_text_descriptor_replay_v3::tests::{self as fixture, KINDS, PROFILES, S, W};
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_TABLE_VIEW_STORAGE_V3, decode_device_descriptor_table_v3,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn run(module: &fe2o3_kernel_ir::Module, profile: Profile, wire: &[u8], accept: bool) {
    let bound = if module.kernels.is_empty() {
        None
    } else {
        Some(crate::bind_production_target_v1(module, profile).unwrap())
    };
    let (owner, retained) = fixture::admit(bound.as_ref().map_or(module, |b| b.module()));
    let table = decode_device_descriptor_table_v3(wire, &mut fixture::free).unwrap();
    let floor = retained + wire.len() + DESCRIPTOR_TABLE_VIEW_STORAGE_V3 + fixture::SIBLING;
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(fixture::PRIOR).unwrap();
    let result =
        check_canonical_v12_descriptor_physical_abi_v3(&owner, profile, &table, &mut budget);
    assert_eq!(result.is_ok(), accept);
    if let Ok(relation) = result {
        assert_eq!(
            relation.retained_storage(),
            size_of::<ReplayedDescriptorPhysicalAbiV3<'_, '_, '_>>()
        );
        budget.reserve_storage(relation.retained_storage()).unwrap();
        assert!(!relation.grants_source_ownership());
        assert!(!relation.grants_authority());
        assert!(std::ptr::eq(relation.output(), &owner));
        assert!(std::ptr::eq(relation.descriptors(), &table));
        let bytes = relation.retained_storage();
        drop(relation);
        budget.release_storage(bytes).unwrap();
    }
    assert_eq!(budget.storage(), floor);
}

#[test]
fn descriptor_physical_v3_all_scalar_representations_and_both_profiles() {
    let scalars = [
        Scalar::I8,
        Scalar::U8,
        Scalar::I16,
        Scalar::U16,
        Scalar::I32,
        Scalar::U32,
        Scalar::I64,
        Scalar::U64,
        Scalar::F16,
        Scalar::F32,
        Scalar::F64,
    ];
    for profile in PROFILES {
        for element in scalars {
            let mut module = fixture::module();
            let mut kinds = KINDS;
            for index in 0..4 {
                module.functions[0].signature.parameters[index] = Type::Scalar(scalar(element));
                kinds[index] = Source::Scalar(element);
            }
            let wire = fixture::wire(profile, &kinds, "kernel", 64, 256);
            run(&module, profile, &wire, true);
        }
    }
}

#[test]
fn descriptor_physical_v3_pointer_slice_access_and_width_donors() {
    for profile in PROFILES {
        for (kind, ty) in [
            (
                Source::SharedSlice(Scalar::U64),
                Type::slice(
                    Type::Scalar(KirScalar::U64),
                    AddressSpace::Global,
                    KirAccess::ReadOnly,
                ),
            ),
            (
                Source::DisjointSlice(Scalar::U64),
                Type::slice(
                    Type::Scalar(KirScalar::U64),
                    AddressSpace::Global,
                    KirAccess::ReadWrite,
                ),
            ),
            (
                Source::GlobalMutPointer(Scalar::U64),
                Type::pointer(
                    Type::Scalar(KirScalar::U64),
                    AddressSpace::Global,
                    KirAccess::ReadWrite,
                ),
            ),
        ] {
            let mut module = fixture::module();
            module.functions[0].signature.parameters[4] = ty;
            let mut kinds = KINDS;
            kinds[4] = kind;
            let wire = fixture::wire(profile, &kinds, "kernel", 64, 256);
            run(&module, profile, &wire, true);
            module.functions[0].signature.parameters[4] = Type::slice(
                Type::Scalar(KirScalar::U32),
                AddressSpace::Global,
                KirAccess::ReadWrite,
            );
            run(&module, profile, &wire, false);
        }
        let wire = fixture::wire(profile, &KINDS, "kernel", 64, 256);
        for ty in [
            Type::Scalar(KirScalar::I64),
            Type::Scalar(KirScalar::U32),
            Type::Scalar(KirScalar::Bool),
        ] {
            let mut module = fixture::module();
            module.functions[0].signature.parameters[0] = ty;
            run(&module, profile, &wire, false);
        }
        for (space, access) in [
            (AddressSpace::Global, KirAccess::ReadOnly),
            (AddressSpace::Workgroup, KirAccess::ReadWrite),
        ] {
            let mut module = fixture::module();
            module.functions[0].signature.parameters[4] =
                Type::slice(Type::Scalar(KirScalar::U64), space, access);
            run(&module, profile, &wire, false);
        }
    }
}

#[test]
fn descriptor_physical_v3_equal_representation_is_explicitly_not_nominal_identity() {
    for profile in PROFILES {
        let nominal = fixture::wire(profile, &KINDS, "kernel", 64, 256);
        let mut fixed = KINDS;
        fixed[0] = Source::Scalar(Scalar::U64);
        fixed[1] = Source::Scalar(Scalar::I64);
        let fixed_wire = fixture::wire(profile, &fixed, "kernel", 64, 256);
        assert_ne!(nominal, fixed_wire);
        // Both representations match KIR. Only the consuming source ABI join
        // decides which nominal descriptor belongs to genuine source history.
        run(&fixture::module(), profile, &nominal, true);
        run(&fixture::module(), profile, &fixed_wire, true);
    }
}

#[test]
fn descriptor_physical_v3_exact_roster_and_cov6_tail() {
    for profile in PROFILES {
        for tail in [0, 8, 248, 264] {
            run(
                &fixture::module(),
                profile,
                &fixture::wire(profile, &KINDS, "kernel", 64, tail),
                false,
            );
        }
        run(
            &fixture::module(),
            profile,
            &fixture::wire(profile, &KINDS, "different", 64, 256),
            false,
        );
        let mut module = fixture::module();
        module.kernels.clear();
        module.functions[0].role = FunctionRole::InternalHelper;
        run(
            &module,
            profile,
            &fixture::wire(profile, &KINDS, "kernel", 64, 256),
            false,
        );
    }
}
