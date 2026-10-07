//! Real address-free packers with synthetic descriptor and signed condition fixtures.
use super::*;
use crate::{GeneratedKfdArgumentBinding, GeneratedKfdWriteSlice};
use fe2o3_artifacts::{
    AbiField, AbiKind, AddressSpace, AliasClass, ArgumentOwnership, Mutability, Name,
};
use fe2o3_kernel_descriptor::{
    BuildEvidenceV1, CanonicalCodeObjectDigest, CompilerIdentityV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1,
    LaunchConstraintsV1, LogicalArgumentV1, ProducerIdentityV1, SourceTypeDescriptorV1,
    SourceTypeRecordV1, Text, ValidName,
};
use fe2o3_kfd::Gfx942KfdDispatchPointerFixupV1 as Fixup;
use fe2o3_runtime::Gfx942RuntimeDispatchBufferV1;
use fe2o3_verifier::CanonicalConditionalOutputEvidenceV1;

#[path = "../../fe2o3-verifier/tests/support/conditional_output_evidence.rs"]
mod signed_fixture;

fn condition(changes: &[(usize, u64)]) -> CanonicalConditionalOutputEvidenceV1 {
    let mut bytes = signed_fixture::preimage([1, 2, 3, 4].map(signed_fixture::digest));
    for &(field, value) in [(0, 0), (3, 2)].iter().chain(changes) {
        let offset = signed_fixture::FIELDS + field * 8;
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    CanonicalConditionalOutputEvidenceV1::decode(&signed_fixture::signed(&bytes)).unwrap()
}

fn layout(
    output_index: usize,
    mapping: RustDisjointIndexSpaceV1,
) -> CompilerGeneratedArgumentLayoutV1 {
    typed_layout::<u32>(output_index, mapping)
}

fn typed_layout<T: GeneratedDeviceScalarV1>(
    output_index: usize,
    mapping: RustDisjointIndexSpaceV1,
) -> CompilerGeneratedArgumentLayoutV1 {
    let mut fields = Vec::new();
    let mut spaces = Vec::new();
    if output_index == 1 {
        fields.push(
            AbiField::new(
                Name::new("seed").unwrap(),
                0,
                4,
                4,
                AbiKind::Scalar(fe2o3_artifacts::ScalarType::U32),
                Mutability::Immutable,
                Access::ByValue,
                AddressSpace::Value,
                u32::scalar_type_identity_v1(PointerWidth::Bits64),
                ArgumentOwnership::ByValue,
                AliasClass::Value,
            )
            .unwrap(),
        );
        spaces.push(None);
    }
    fields.push(
        AbiField::new(
            Name::new("output").unwrap(),
            (output_index * 8) as u64,
            16,
            8,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            Mutability::Mutable,
            Access::WriteOnly,
            AddressSpace::Global,
            T::disjoint_slice_type_identity_for_index_space_v1(PointerWidth::Bits64, mapping),
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
        )
        .unwrap(),
    );
    spaces.push(Some(mapping));
    CompilerGeneratedArgumentLayoutV1::new_with_disjoint_index_spaces_v1(
        (16 + output_index * 8) as u64,
        8,
        PointerWidth::Bits64,
        fields,
        spaces,
    )
    .unwrap()
}

fn table(
    output_index: usize,
    scalar: ScalarTypeV1,
    access: AccessMode,
    max_grid: u32,
) -> DeviceDescriptorTableV1 {
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(scalar));
    let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(scalar));
    let mut types = vec![source.clone()];
    let mut layouts = vec![layout.clone()];
    let mut arguments = Vec::new();
    if output_index == 1 {
        let ty = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::U32));
        let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U32));
        arguments.push(
            LogicalArgumentV1::scalar(0, ValidName::new("seed").unwrap(), &ty, &layout, 0).unwrap(),
        );
        types.push(ty);
        layouts.push(layout);
    }
    arguments.push(
        LogicalArgumentV1::disjoint_slice(
            output_index as u16,
            ValidName::new("output").unwrap(),
            &source,
            &layout,
            access,
            (output_index * 8) as u32,
        )
        .unwrap(),
    );
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([1; 32]),
        EvidenceDigest::from_sha256_bytes([2; 32]),
    );
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([3; 32]),
        ValidName::new("fill").unwrap(),
        ValidName::new("fill").unwrap(),
        ValidName::new("fill.kd").unwrap(),
        evidence,
        evidence,
        vec![],
        KernelAbiLayoutV1::new(
            (16 + output_index * 8) as u32,
            (16 + output_index * 8) as u32,
            8,
        )
        .unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
            DimensionsV1::new(max_grid, 1, 1).unwrap(),
            64,
            0,
            0,
        )
        .unwrap(),
        arguments,
    )
    .unwrap();
    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new("rustc").unwrap(),
            Text::new("synthetic").unwrap(),
            [4; 20],
        ),
        ProducerIdentityV1::new(Text::new("test").unwrap(), Text::new("synthetic").unwrap()),
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        types,
        layouts,
        vec![kernel],
    )
    .unwrap()
}

fn pack<'a>(
    plan: &GeneratedArgumentPackingPlanV1,
    values: &'a mut [u32],
) -> GeneratedKfdPackedArguments<'a> {
    let index = plan.argument_count() - 1;
    let scalars = if index == 1 {
        vec![plan.scalar(0, 17_u32).unwrap()]
    } else {
        vec![]
    };
    let output = GeneratedKfdWriteSlice::new(values)
        .bind_argument(plan, index)
        .unwrap();
    GeneratedKfdArgumentBinding::from_compiler_generated_parts(scalars, vec![output])
        .pack(plan)
        .unwrap()
}

fn geometry(x: u32) -> AqlDispatchGeometryV1 {
    AqlDispatchGeometryV1::new([x, 1, 1], [64, 1, 1]).unwrap()
}

pub(crate) struct PackingFixture {
    evidence: CanonicalConditionalOutputEvidenceV1,
    table: DeviceDescriptorTableV1,
    plan: GeneratedArgumentPackingPlanV1,
    output: OutputArgumentLayout,
}

impl PackingFixture {
    pub(crate) fn new() -> Self {
        let table = table(0, ScalarTypeV1::U32, AccessMode::WriteOnly, 128);
        Self::with_layout(table, &layout(0, RustDisjointIndexSpaceV1::Index1D))
    }

    // Test-only inert leaf data, deliberately not a ConditionalOutputArgumentBindingV1.
    pub(crate) fn with_layout(
        table: DeviceDescriptorTableV1,
        generated: &CompilerGeneratedArgumentLayoutV1,
    ) -> Self {
        let evidence = condition(&[]);
        let (plan, output) = validate_output_layout(
            evidence.obligation(),
            &table,
            &table.kernels()[0],
            generated,
        )
        .unwrap();
        Self {
            evidence,
            table,
            plan,
            output,
        }
    }

    pub(crate) fn plan(&self) -> &GeneratedArgumentPackingPlanV1 {
        &self.plan
    }

    pub(crate) fn check(
        &self,
        packed: GeneratedPackedArgumentsViewV1<'_>,
        grid_x: u32,
    ) -> Result<u64, ConditionalPackedCoverageErrorV1> {
        self.check_geometry(packed, geometry(grid_x))
    }

    pub(crate) fn check_geometry(
        &self,
        packed: GeneratedPackedArgumentsViewV1<'_>,
        geometry: AqlDispatchGeometryV1,
    ) -> Result<u64, ConditionalPackedCoverageErrorV1> {
        check_packed_coverage(
            self.evidence.obligation(),
            &self.table.kernels()[0],
            &self.plan,
            self.output,
            &packed,
            geometry,
        )
    }
}

#[test]
fn selected_kir_output_requires_exact_element_access_address_and_entry() {
    use fe2o3_kernel_ir as kir;
    let table = table(0, ScalarTypeV1::U32, AccessMode::WriteOnly, 128);
    let slice = kir::Type::slice(
        kir::Type::Scalar(kir::ScalarType::U32),
        kir::AddressSpace::Global,
        kir::AccessMode::WriteOnly,
    );
    let mut module = kir::Module::new("test");
    let mut kernel = kir::Kernel::new(
        "fill",
        "fill",
        kir::LaunchDomain::D1 {
            x: kir::LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(kir::WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    let mut function = kir::Function::definition(
        "fill",
        kir::Signature::new(vec![slice.clone()], vec![]),
        vec![kir::ValueId(0)],
        vec![],
    );
    function.role = kir::FunctionRole::KernelEntry;
    module.functions.push(function);
    validate_kir_output(&module, &table.kernels()[0], 0).unwrap();
    for parameter in [
        kir::Type::pointer(
            kir::Type::Scalar(kir::ScalarType::U32),
            kir::AddressSpace::Global,
            kir::AccessMode::WriteOnly,
        ),
        kir::Type::slice(
            kir::Type::Scalar(kir::ScalarType::I32),
            kir::AddressSpace::Global,
            kir::AccessMode::WriteOnly,
        ),
        kir::Type::slice(
            kir::Type::Scalar(kir::ScalarType::U32),
            kir::AddressSpace::Global,
            kir::AccessMode::ReadWrite,
        ),
        kir::Type::slice(
            kir::Type::Scalar(kir::ScalarType::U32),
            kir::AddressSpace::Private,
            kir::AccessMode::WriteOnly,
        ),
    ] {
        module.functions[0].signature.parameters[0] = parameter;
        assert!(matches!(
            validate_kir_output(&module, &table.kernels()[0], 0),
            Err(ConditionalPackedCoverageErrorV1::SourceBinding)
        ));
    }
    module.functions[0].signature.parameters[0] = slice;
    module.functions[0].role = kir::FunctionRole::InternalHelper;
    assert!(validate_kir_output(&module, &table.kernels()[0], 0).is_err());
}

#[test]
fn fixups_straddling_selected_components_are_rejected() {
    let evidence = condition(&[(1, 1), (2, 2), (3, 3)]);
    let table = table(1, ScalarTypeV1::U32, AccessMode::WriteOnly, 128);
    let (plan, output) = validate_output_layout(
        evidence.obligation(),
        &table,
        &table.kernels()[0],
        &layout(1, RustDisjointIndexSpaceV1::Index1D),
    )
    .unwrap();
    let mut values = [0_u32; 1];
    let packed = pack(&plan, &mut values);
    for offset in [1, 17] {
        let fixups = [Fixup::new(8, 0, 0, 4), Fixup::new(offset, 1, 0, 4)];
        let mut view = packed.packed_view_v1();
        view.pointer_fixups = &fixups;
        assert!(matches!(
            check_packed_coverage(
                evidence.obligation(),
                &table.kernels()[0],
                &plan,
                output,
                &view,
                geometry(64)
            ),
            Err(ConditionalPackedCoverageErrorV1::OutputBinding)
        ));
    }
}

#[test]
fn real_packing_covers_empty_edges_and_independent_source_ordinal() {
    for output_index in [0, 1] {
        let evidence = condition(&[
            (1, output_index as u64),
            (2, output_index as u64 + 1),
            (3, output_index as u64 + 2),
        ]);
        let table = table(
            output_index,
            ScalarTypeV1::U32,
            AccessMode::WriteOnly,
            u32::MAX,
        );
        let (plan, output) = validate_output_layout(
            evidence.obligation(),
            &table,
            &table.kernels()[0],
            &layout(output_index, RustDisjointIndexSpaceV1::Index1D),
        )
        .unwrap();
        for n in [0, 1, 63, 64, 65, 4097] {
            let mut values = vec![0xabab_abab; n];
            let packed = pack(&plan, &mut values);
            let grid = ((n.max(1) as u32).div_ceil(64)) * 64;
            assert_eq!(
                check_packed_coverage(
                    evidence.obligation(),
                    &table.kernels()[0],
                    &plan,
                    output,
                    &packed.packed_view_v1(),
                    geometry(grid)
                )
                .unwrap(),
                n as u64
            );
            assert_eq!(packed.buffers().len(), usize::from(n != 0));
            assert_eq!(packed.pointer_fixups().len(), usize::from(n != 0));
            drop(packed);
            assert!(values.iter().all(|v| *v == 0xabab_abab));
        }
    }
}

#[test]
fn packed_owner_retains_exact_types_after_original_plan_drop() {
    let fixture = PackingFixture::new();
    let signed_table = table(0, ScalarTypeV1::I32, AccessMode::WriteOnly, 128);
    for n in [0, 65] {
        let independent = PackingFixture::new();
        let mut values = vec![0_u32; n];
        let packed = pack(independent.plan(), &mut values);
        drop(independent);
        assert_eq!(
            fixture.check(packed.packed_view_v1(), 128).unwrap(),
            n as u64
        );
        let mut view = packed.packed_view_v1();
        view.pointer_width = PointerWidth::Bits32;
        assert!(matches!(
            fixture.check(view, 128),
            Err(ConditionalPackedCoverageErrorV1::PackedIdentity)
        ));

        let signed_plan = validate_worker_v3_argument_packing(
            &signed_table,
            &signed_table.kernels()[0],
            &typed_layout::<i32>(0, RustDisjointIndexSpaceV1::Index1D),
        )
        .unwrap();
        let mut signed_values = vec![0_i32; n];
        let output = GeneratedKfdWriteSlice::new(&mut signed_values)
            .bind_argument(&signed_plan, 0)
            .unwrap();
        let signed =
            GeneratedKfdArgumentBinding::from_compiler_generated_parts(vec![], vec![output])
                .pack(&signed_plan)
                .unwrap();
        drop(signed_plan);
        assert_eq!(signed.packing_observation(), packed.packing_observation());
        assert!(matches!(
            fixture.check(signed.packed_view_v1(), 128),
            Err(ConditionalPackedCoverageErrorV1::PackedIdentity)
        ));
    }
}

#[test]
fn packed_identity_checks_non_output_argument_types() {
    let evidence = condition(&[(1, 1), (2, 2), (3, 3)]);
    let table = table(1, ScalarTypeV1::U32, AccessMode::WriteOnly, 128);
    let (plan, output) = validate_output_layout(
        evidence.obligation(),
        &table,
        &table.kernels()[0],
        &layout(1, RustDisjointIndexSpaceV1::Index1D),
    )
    .unwrap();
    let mut values = [0_u32];
    let packed = pack(&plan, &mut values);
    let mut fields = packed.packed_view_v1().argument_fields.to_vec();
    fields[0] = AbiField::new(
        Name::new("seed").unwrap(),
        0,
        4,
        4,
        AbiKind::Scalar(fe2o3_artifacts::ScalarType::I32),
        Mutability::Immutable,
        Access::ByValue,
        AddressSpace::Value,
        i32::scalar_type_identity_v1(PointerWidth::Bits64),
        ArgumentOwnership::ByValue,
        AliasClass::Value,
    )
    .unwrap();
    let mut view = packed.packed_view_v1();
    view.argument_fields = &fields;
    assert!(matches!(
        check_packed_coverage(
            evidence.obligation(),
            &table.kernels()[0],
            &plan,
            output,
            &view,
            geometry(64),
        ),
        Err(ConditionalPackedCoverageErrorV1::PackedIdentity)
    ));
}

#[test]
fn source_profile_rejects_ordinal_alias_type_access_and_mapping_substitution() {
    let table = table(0, ScalarTypeV1::U32, AccessMode::WriteOnly, u32::MAX);
    let generated = layout(0, RustDisjointIndexSpaceV1::Index1D);
    for changes in [
        vec![(0, 2)],
        vec![(1, 1)],
        vec![(2, 2)],
        vec![(3, 1)],
        vec![(7, 32)],
        vec![(10, 32)],
    ] {
        let evidence = condition(&changes);
        assert!(
            validate_output_layout(
                evidence.obligation(),
                &table,
                &table.kernels()[0],
                &generated
            )
            .is_err()
        );
    }
    let evidence = condition(&[]);
    let mapped = layout(0, RustDisjointIndexSpaceV1::ShiftedIndex1D { offset: 1 });
    assert!(matches!(
        validate_output_layout(evidence.obligation(), &table, &table.kernels()[0], &mapped),
        Err(ConditionalPackedCoverageErrorV1::UnsupportedProfile)
    ));
    for (scalar, access) in [
        (ScalarTypeV1::I32, AccessMode::WriteOnly),
        (ScalarTypeV1::U32, AccessMode::ReadWrite),
    ] {
        let other = self::table(0, scalar, access, u32::MAX);
        assert!(matches!(
            validate_output_layout(
                evidence.obligation(),
                &other,
                &other.kernels()[0],
                &generated
            ),
            Err(ConditionalPackedCoverageErrorV1::UnsupportedProfile)
        ));
    }
}

#[test]
fn actual_grid_and_conditional_geometry_requirements_are_enforced() {
    let evidence = condition(&[]);
    let table = table(0, ScalarTypeV1::U32, AccessMode::WriteOnly, 128);
    let (plan, output) = validate_output_layout(
        evidence.obligation(),
        &table,
        &table.kernels()[0],
        &layout(0, RustDisjointIndexSpaceV1::Index1D),
    )
    .unwrap();
    let mut values = vec![0_u32; 65];
    let packed = pack(&plan, &mut values);
    let check = |condition: &InertConditionalOutputObligationV1, geometry| {
        check_packed_coverage(
            condition,
            &table.kernels()[0],
            &plan,
            output,
            &packed.packed_view_v1(),
            geometry,
        )
    };
    assert!(matches!(
        check(evidence.obligation(), geometry(64)),
        Err(ConditionalPackedCoverageErrorV1::Underlaunch {
            elements: 65,
            grid_x: 64
        })
    ));
    for geometry in [
        geometry(192),
        AqlDispatchGeometryV1::new([128, 1, 1], [32, 1, 1]).unwrap(),
        AqlDispatchGeometryV1::new([128, 2, 1], [64, 1, 1]).unwrap(),
        AqlDispatchGeometryV1::new([128, 1, 2], [64, 1, 1]).unwrap(),
    ] {
        assert!(matches!(
            check(evidence.obligation(), geometry),
            Err(ConditionalPackedCoverageErrorV1::Geometry)
        ));
    }
    assert_eq!(check(evidence.obligation(), geometry(65)).unwrap(), 65);
    let full = condition(&[(11, 1)]);
    assert!(matches!(
        check(full.obligation(), geometry(65)),
        Err(ConditionalPackedCoverageErrorV1::Geometry)
    ));
    let fixed = condition(&[(6, 64)]);
    assert!(matches!(
        check(fixed.obligation(), geometry(128)),
        Err(ConditionalPackedCoverageErrorV1::Geometry)
    ));
}

#[test]
fn actual_fixups_and_buffer_extent_access_are_checked() {
    let evidence = condition(&[]);
    let table = table(0, ScalarTypeV1::U32, AccessMode::WriteOnly, 128);
    let (plan, output) = validate_output_layout(
        evidence.obligation(),
        &table,
        &table.kernels()[0],
        &layout(0, RustDisjointIndexSpaceV1::Index1D),
    )
    .unwrap();
    let mut values = [0_u32; 64];
    let packed = pack(&plan, &mut values);
    for fixups in [
        vec![],
        vec![Fixup::new(0, 0, 0, 4); 2],
        vec![Fixup::new(8, 0, 0, 4)],
        vec![Fixup::new(0, 1, 0, 4)],
        vec![Fixup::new(0, 0, 4, 4)],
        vec![Fixup::new(0, 0, 0, 8)],
        vec![Fixup::new(0, 0, 0, 4), Fixup::new(32, 0, 0, 4)],
    ] {
        let mut view = packed.packed_view_v1();
        view.pointer_fixups = &fixups;
        assert!(matches!(
            check_packed_coverage(
                evidence.obligation(),
                &table.kernels()[0],
                &plan,
                output,
                &view,
                geometry(64)
            ),
            Err(ConditionalPackedCoverageErrorV1::OutputBinding)
        ));
    }
    for (size, access) in [
        (252, Gfx942RuntimeBufferAccessV1::WriteOnly),
        (260, Gfx942RuntimeBufferAccessV1::WriteOnly),
        (256, Gfx942RuntimeBufferAccessV1::ReadWrite),
    ] {
        let buffers = [Gfx942RuntimeDispatchBufferV1::new(vec![0; size], access).unwrap()];
        let mut view = packed.packed_view_v1();
        view.buffers = &buffers;
        assert!(matches!(
            check_packed_coverage(
                evidence.obligation(),
                &table.kernels()[0],
                &plan,
                output,
                &view,
                geometry(64)
            ),
            Err(ConditionalPackedCoverageErrorV1::OutputBinding)
        ));
    }
}

#[test]
fn packed_bytes_overflow_pointer_and_identity_mutations_reach_their_checks() {
    let evidence = condition(&[]);
    let table = table(0, ScalarTypeV1::U32, AccessMode::WriteOnly, 128);
    let (plan, output) = validate_output_layout(
        evidence.obligation(),
        &table,
        &table.kernels()[0],
        &layout(0, RustDisjointIndexSpaceV1::Index1D),
    )
    .unwrap();
    let mut values = [0_u32; 64];
    let packed = pack(&plan, &mut values);
    for (offset, value, expected) in [
        (0, 1_u64, "binding"),
        (8, u64::MAX, "extent"),
        (8, 63, "binding"),
    ] {
        let mut bytes = packed.explicit_kernarg().to_vec();
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        let observation = packed
            .packing_observation()
            .with_explicit_kernarg_for_test(&bytes);
        let mut view = packed.packed_view_v1();
        view.explicit_kernarg = &bytes;
        view.observation = &observation;
        let error = check_packed_coverage(
            evidence.obligation(),
            &table.kernels()[0],
            &plan,
            output,
            &view,
            geometry(64),
        )
        .unwrap_err();
        assert!(matches!(
            (expected, error),
            ("binding", ConditionalPackedCoverageErrorV1::OutputBinding)
                | ("extent", ConditionalPackedCoverageErrorV1::OutputExtent)
        ));
    }
    for mutation in 0..4 {
        let mut view = packed.packed_view_v1();
        match mutation {
            0 => view.kernel_id = KernelId::from_bytes([9; 32]),
            1 => view.alignment = 16,
            2 => view.explicit_kernarg = &view.explicit_kernarg[..15],
            _ => view.explicit_kernarg = &[0; 16],
        }
        assert!(matches!(
            check_packed_coverage(
                evidence.obligation(),
                &table.kernels()[0],
                &plan,
                output,
                &view,
                geometry(64)
            ),
            Err(ConditionalPackedCoverageErrorV1::PackedIdentity)
        ));
    }
}

#[test]
fn empty_output_rejects_dummy_fixups_and_foreign_observations() {
    let evidence = condition(&[]);
    let table = table(0, ScalarTypeV1::U32, AccessMode::WriteOnly, 128);
    let (plan, output) = validate_output_layout(
        evidence.obligation(),
        &table,
        &table.kernels()[0],
        &layout(0, RustDisjointIndexSpaceV1::Index1D),
    )
    .unwrap();
    let mut empty = [];
    let packed = pack(&plan, &mut empty);
    let fixups = [Fixup::new(0, 0, 0, 4)];
    let mut view = packed.packed_view_v1();
    view.pointer_fixups = &fixups;
    assert!(matches!(
        check_packed_coverage(
            evidence.obligation(),
            &table.kernels()[0],
            &plan,
            output,
            &view,
            geometry(64)
        ),
        Err(ConditionalPackedCoverageErrorV1::OutputBinding)
    ));
    let mut other = [0_u32; 1];
    let other = pack(&plan, &mut other);
    let mut view = packed.packed_view_v1();
    view.observation = other.packing_observation();
    assert!(matches!(
        check_packed_coverage(
            evidence.obligation(),
            &table.kernels()[0],
            &plan,
            output,
            &view,
            geometry(64)
        ),
        Err(ConditionalPackedCoverageErrorV1::PackedIdentity)
    ));
}

#[test]
fn empty_and_nonempty_outputs_require_the_exact_address_free_pointer_encoding() {
    let fixture = PackingFixture::new();
    for (n, expected_pointer) in [(0, 4_u64), (1, 0)] {
        let mut values = vec![0_u32; n];
        let packed = pack(fixture.plan(), &mut values);
        assert_eq!(
            u64::from_le_bytes(packed.explicit_kernarg()[..8].try_into().unwrap()),
            expected_pointer
        );
        assert_eq!(
            fixture.check(packed.packed_view_v1(), 64).unwrap(),
            n as u64
        );
        for pointer in [0_u64, 1, 2, 4, 8, 4096, u64::MAX] {
            if pointer == expected_pointer {
                continue;
            }
            let mut bytes = packed.explicit_kernarg().to_vec();
            bytes[..8].copy_from_slice(&pointer.to_le_bytes());
            let observation = packed
                .packing_observation()
                .with_explicit_kernarg_for_test(&bytes);
            let mut view = packed.packed_view_v1();
            view.explicit_kernarg = &bytes;
            view.observation = &observation;
            assert!(matches!(
                fixture.check(view, 64),
                Err(ConditionalPackedCoverageErrorV1::OutputBinding)
            ));
        }
    }
}
