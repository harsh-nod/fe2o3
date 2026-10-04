#![cfg(test)]
//! Full field/finish components using real V5 decoding and semantic constructors.
//! These are inert fixtures, not rustc captures, admitted source owners, Requests,
//! executions, imports, or original collector custody. Setup is outside the
//! measured component scope; its borrowed backing is never fresh production custody.
use super::super::{Budget, E, R, Resource, account, nominal};
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::{
    AccessMode as KirAccess, AddressSpace, CanonicalKernelIrWorkBudgetV1 as Work,
    ScalarType as KirScalar, Type,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use std::mem::{size_of, size_of_val};

#[allow(dead_code)]
#[path = "../../../fe2o3-kernel-descriptor/tests/support/conditional_v5.rs"]
mod descriptor_fixture;

const PROFILES: [&str; 2] = ["gfx942", "gfx950"];
const WORK_CAP: usize = 4 * 1024 * 1024;
const STORAGE_CAP: usize = 4 * 1024 * 1024;
const PRIOR_WORK: usize = 23;
const SIBLING: [u8; 37] = [0x79; 37];
const SCRATCH: usize = DESCRIPTOR_QUERY_STORAGE_V5
    + nominal::STORAGE
    + size_of::<KernelDescriptorRefV5<'static, 'static>>();

#[derive(Clone, Copy, Debug)]
enum Wire {
    Ordinary,
    LeadingGap,
    SegmentAlignment,
    HiddenTail,
    OutputReadWrite,
}

/// Reuse the existing complete V5 fixture, including its strict V2 suffix.
/// Only public borrowed encoder inputs are changed; no private view is forged.
fn wire(profile: &str, mutation: Wire) -> Vec<u8> {
    descriptor_fixture::with_input(profile, 1, 2, |input| {
        let old = &input.nominal.kernels[0];
        let mut components = old
            .arguments
            .iter()
            .map(|a| a.components.to_vec())
            .collect::<Vec<_>>();
        if matches!(mutation, Wire::LeadingGap) {
            for argument in &mut components {
                for component in argument {
                    component.offset += 8;
                }
            }
        }
        if matches!(mutation, Wire::OutputReadWrite) {
            components[2][0].access = AccessMode::ReadWrite;
        }
        let arguments = old
            .arguments
            .iter()
            .enumerate()
            .map(|(index, a)| LogicalArgumentInputV3 {
                source_index: a.source_index,
                name: a.name,
                source_type: a.source_type,
                device_layout: a.device_layout,
                ownership: a.ownership,
                access: components[index][0].access,
                alias: a.alias,
                components: &components[index],
            })
            .collect::<Vec<_>>();
        let explicit = if matches!(mutation, Wire::LeadingGap) {
            56
        } else {
            48
        };
        let hidden = if matches!(mutation, Wire::HiddenTail) {
            264
        } else {
            256
        };
        let alignment = if matches!(mutation, Wire::SegmentAlignment) {
            16
        } else {
            8
        };
        let kernels = [KernelDescriptorInputV3 {
            kernel_id: old.kernel_id,
            logical_name: old.logical_name,
            entry_name: old.entry_name,
            descriptor_symbol: old.descriptor_symbol,
            source_evidence: old.source_evidence,
            executable_ir_evidence: old.executable_ir_evidence,
            capabilities: old.capabilities,
            abi_layout: KernelAbiLayoutV1::new(explicit, explicit + hidden, alignment).unwrap(),
            launch: old.launch,
            arguments: &arguments,
        }];
        let input = DeviceDescriptorTableInputV5 {
            nominal: DeviceDescriptorTableInputV3 {
                kernels: &kernels,
                ..input.nominal
            },
            contracts: input.contracts,
        };
        let mut bytes =
            vec![
                0;
                encoded_device_descriptor_table_v5_len(&input, &mut descriptor_fixture::free)
                    .unwrap()
            ];
        encode_device_descriptor_table_v5(&input, &mut bytes, &mut descriptor_fixture::free)
            .unwrap();
        bytes
    })
}

struct Field {
    ty: SemanticTypeDeclV1,
    adjusted: SemanticAbiArgumentV1,
    ownership: SemanticSourceArgumentOwnershipV1,
    canonical: Type,
}

#[derive(Clone, Copy, Debug)]
enum Change {
    None,
    Index,
    Ownership,
    Element,
    Access,
    Mode,
    SourceSize,
    SourceAlignment,
    MissingLastField,
}

/// Public component types with the same fat-slice ABI as ordinary Vecadd. The
/// output's aggregate shape describes pointer/length storage, not its Rust origin.
fn fields(change: Change) -> [Field; 3] {
    std::array::from_fn(|index| {
        let output = index == 2;
        let attrs = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap();
        let id = SemanticTypeIdV1::from_index(if output { 3 } else { 2 });
        let shape = if output {
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![
                    SemanticTypeIdV1::from_index(4),
                    SemanticTypeIdV1::from_index(5),
                ])
                .unwrap(),
            )
        } else {
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SemanticTypeIdV1::from_index(1),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
                )
                .unwrap(),
            )
        };
        let layout = if output && matches!(change, Change::SourceSize | Change::SourceAlignment) {
            SemanticTypeLayoutV1::new(
                Some(if matches!(change, Change::SourceSize) {
                    24
                } else {
                    16
                }),
                if matches!(change, Change::SourceAlignment) {
                    4
                } else {
                    8
                },
            )
            .unwrap()
        } else {
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    ),
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    ),
                ),
                false,
            )
            .unwrap()
        };
        let ty = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([if output { 31 } else { 30 }; 32]),
            SemanticLayoutIdentityV1::from_sha256([41; 32]),
            layout,
            shape,
        );
        let mode = if index == 1 && matches!(change, Change::Mode) {
            SemanticAbiPassModeV1::Direct(attrs)
        } else {
            SemanticAbiPassModeV1::Pair {
                first: attrs,
                second: attrs,
            }
        };
        let adjusted = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(id, mode));
        let ownership = if output && matches!(change, Change::Ownership) {
            SemanticSourceArgumentOwnershipV1::ByValue
        } else if output {
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner
        } else {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        };
        let element = if index == 1 && matches!(change, Change::Element) {
            KirScalar::F64
        } else {
            KirScalar::F32
        };
        let access = if index == 1 && matches!(change, Change::Access) {
            KirAccess::ReadWrite
        } else if output {
            KirAccess::WriteOnly
        } else {
            KirAccess::ReadOnly
        };
        Field {
            ty,
            adjusted,
            ownership,
            canonical: Type::slice(Type::Scalar(element), AddressSpace::Global, access),
        }
    })
}

#[derive(Default, Debug)]
struct Visits {
    fields: usize,
    finish: bool,
}

/// No production checker result is replaced: every field and finish call below
/// goes through the actual private implementation with decoded descriptor rows.
fn walk(
    table: &DeviceDescriptorTableV5<'_>,
    fields: &[Field; 3],
    change: Change,
    budget: &mut Budget<'_>,
    visited: &mut Visits,
) -> R<()> {
    let kernel = table.kernel(0, &mut |w| budget.charge_work(w))?;
    let mut arguments = kernel.arguments();
    let mut state = nominal::Layout::default();
    let count = if matches!(change, Change::MissingLastField) {
        2
    } else {
        3
    };
    for (index, field) in fields[..count].iter().enumerate() {
        let argument = arguments.next(&mut |w| budget.charge_work(w))?.unwrap();
        nominal::field(
            table,
            &argument,
            if index == 1 && matches!(change, Change::Index) {
                2
            } else {
                index
            },
            &field.ty,
            &field.adjusted,
            field.ownership,
            &field.canonical,
            &mut state,
            budget,
        )?;
        visited.fields += 1;
    }
    visited.finish = true;
    nominal::finish(&kernel, state, budget)
}

struct Run {
    result: R<()>,
    visited: Visits,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
    floor: usize,
}

fn floor(bytes: &Vec<u8>, fields: &[Field; 3]) -> usize {
    // Nonzero inherited fixture/sibling extent, not a receipt for source custody.
    // Existing semantic constructors keep their own setup allocation domain.
    79 + SIBLING.len()
        + size_of_val(bytes)
        + bytes.capacity()
        + DESCRIPTOR_TABLE_VIEW_STORAGE_V5
        + size_of_val(fields)
}

fn run(
    bytes: &Vec<u8>,
    fields: &[Field; 3],
    change: Change,
    work_limit: usize,
    storage_limit: usize,
) -> Run {
    // Strict decoding must succeed before testing a nominal mismatch. A parser
    // rejection cannot be mistaken for field/finish coverage.
    let table = decode_device_descriptor_table_v5(bytes, &mut descriptor_fixture::free).unwrap();
    let floor = floor(bytes, fields);
    let sibling = SIBLING;
    let mut work = Work::new(work_limit);
    let mut visited = Visits::default();
    let (result, used, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(PRIOR_WORK).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = account::scope(&mut budget, SCRATCH, |budget| {
            walk(&table, fields, change, budget, &mut visited)
        });
        assert!(ledger == budget.work_ledger_identity_v1());
        assert_eq!(budget.storage(), floor);
        assert_eq!(sibling, SIBLING);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Run {
        result,
        visited,
        work: used,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
        floor,
    }
}

#[test]
fn ordinary_three_field_vecadd_layout_calls_field_and_finish_for_both_profiles() {
    for profile in PROFILES {
        let bytes = wire(profile, Wire::Ordinary);
        let fields = fields(Change::None);
        let table =
            decode_device_descriptor_table_v5(&bytes, &mut descriptor_fixture::free).unwrap();
        assert_eq!(
            table.device_target(),
            DeviceTargetV1::parse(profile).unwrap()
        );
        let kernel = table.kernel(0, &mut descriptor_fixture::free).unwrap();
        assert_eq!((kernel.argument_count(), kernel.component_count()), (3, 6));
        assert_eq!(
            (
                kernel.abi_layout().explicit_argument_size(),
                kernel.abi_layout().kernarg_segment_size(),
                kernel.abi_layout().kernarg_segment_alignment()
            ),
            (48, 304, 8)
        );
        let mut cursor = kernel.arguments();
        for (i, offset) in [0, 16, 32].into_iter().enumerate() {
            let argument = cursor.next(&mut descriptor_fixture::free).unwrap().unwrap();
            assert_eq!(argument.source_index(), i as u16);
            assert_eq!(argument.component_count(), 2);
            assert_eq!(
                argument
                    .component(0, &mut descriptor_fixture::free)
                    .unwrap()
                    .offset,
                offset
            );
            assert_eq!(
                argument
                    .component(1, &mut descriptor_fixture::free)
                    .unwrap()
                    .offset,
                offset + 8
            );
        }
        assert!(
            cursor
                .next(&mut descriptor_fixture::free)
                .unwrap()
                .is_none()
        );
        let result = run(&bytes, &fields, Change::None, WORK_CAP, STORAGE_CAP);
        result.result.unwrap();
        assert_eq!(result.visited.fields, 3);
        assert!(result.visited.finish);
        assert_eq!((result.failed_work, result.failed_storage), (None, None));
    }
}

#[test]
fn valid_v5_physical_mutations_reach_field_or_finish_not_just_decoder() {
    for profile in PROFILES {
        for (mutation, fields_ok, finish, message) in [
            (Wire::LeadingGap, 0, false, "exact physical component"),
            (
                Wire::SegmentAlignment,
                3,
                true,
                "complete physical kernarg layout",
            ),
            (
                Wire::HiddenTail,
                3,
                true,
                "complete physical kernarg layout",
            ),
            (
                Wire::OutputReadWrite,
                2,
                false,
                "nominal source/N/physical argument",
            ),
        ] {
            let bytes = wire(profile, mutation);
            let result = run(
                &bytes,
                &fields(Change::None),
                Change::None,
                WORK_CAP,
                STORAGE_CAP,
            );
            assert!(
                matches!(result.result, Err(E::Mismatch(actual)) if actual == message),
                "{profile} {mutation:?}"
            );
            assert_eq!(result.visited.fields, fields_ok, "{profile} {mutation:?}");
            assert_eq!(result.visited.finish, finish, "{profile} {mutation:?}");
        }
    }
}

#[test]
fn field_checks_separate_source_abi_type_ownership_and_ordinals() {
    for profile in PROFILES {
        let bytes = wire(profile, Wire::Ordinary);
        for (change, fields_ok) in [
            (Change::Index, 1),
            (Change::Ownership, 2),
            (Change::Element, 1),
            (Change::Access, 1),
            (Change::Mode, 1),
            (Change::SourceSize, 2),
            (Change::SourceAlignment, 2),
        ] {
            let result = run(&bytes, &fields(change), change, WORK_CAP, STORAGE_CAP);
            assert!(
                matches!(
                    result.result,
                    Err(E::Mismatch("nominal source/N/physical argument"))
                ),
                "{profile} {change:?}"
            );
            assert_eq!(result.visited.fields, fields_ok, "{profile} {change:?}");
            assert!(!result.visited.finish);
        }
    }
}

#[test]
fn finish_rejects_an_incomplete_actual_component_walk() {
    for profile in PROFILES {
        let bytes = wire(profile, Wire::Ordinary);
        let result = run(
            &bytes,
            &fields(Change::None),
            Change::MissingLastField,
            WORK_CAP,
            STORAGE_CAP,
        );
        assert!(matches!(
            result.result,
            Err(E::Mismatch("complete physical kernarg layout"))
        ));
        assert_eq!(result.visited.fields, 2);
        assert!(result.visited.finish);
    }
}

#[test]
fn complete_nominal_walk_exact_and_one_short_inherited_work_and_storage() {
    for profile in PROFILES {
        let bytes = wire(profile, Wire::Ordinary);
        let fields = fields(Change::None);
        let baseline = run(&bytes, &fields, Change::None, WORK_CAP, STORAGE_CAP);
        baseline.result.unwrap();
        assert!(baseline.work > PRIOR_WORK + 5);
        assert_eq!(baseline.peak, baseline.floor + account::HEADER + SCRATCH);
        let exact = run(&bytes, &fields, Change::None, baseline.work, baseline.peak);
        exact.result.unwrap();
        assert_eq!((exact.work, exact.peak), (baseline.work, baseline.peak));
        assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
        let short = run(
            &bytes,
            &fields,
            Change::None,
            baseline.work - 1,
            baseline.peak,
        );
        let Err(E::Resource(Resource::Work(denied))) = short.result else {
            panic!("finish work debit");
        };
        assert_eq!(
            (denied.actual(), denied.limit()),
            (baseline.work, baseline.work - 1)
        );
        assert_eq!((short.visited.fields, short.visited.finish), (3, true));
        // finish charges its existing five units atomically, not one invented unit.
        assert_eq!((short.work, short.peak), (baseline.work - 5, baseline.peak));
        assert_eq!(
            (short.failed_work, short.failed_storage),
            (Some(baseline.work), None)
        );
        let short = run(
            &bytes,
            &fields,
            Change::None,
            baseline.work,
            baseline.peak - 1,
        );
        let Err(E::Resource(Resource::Storage(denied))) = short.result else {
            panic!("nominal scratch extent");
        };
        assert_eq!(
            (denied.actual(), denied.limit()),
            (baseline.peak, baseline.peak - 1)
        );
        assert_eq!((short.visited.fields, short.visited.finish), (0, false));
        assert_eq!((short.work, short.peak), (PRIOR_WORK, baseline.floor));
        assert_eq!(
            (short.failed_work, short.failed_storage),
            (None, Some(baseline.peak))
        );
    }
}
