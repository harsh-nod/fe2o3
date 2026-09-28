//! Inert descriptor/source-field components, not reconstructed source or proof.
use super::*;
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_lower_mir_kernel::ProductionSourceLaunchInputV1 as Launch;

#[path = "../../../fe2o3-kernel-descriptor/tests/support/conditional_v5.rs"]
mod inert;

fn wire(target: &str, block: BlockSizeV1) -> Vec<u8> {
    inert::with_input(target, 2, 2, |input| {
        let launch =
            LaunchConstraintsV1::new(1, block, DimensionsV1::new(1024, 1, 1).unwrap(), 256, 0, 0)
                .unwrap();
        let kernels = input
            .nominal
            .kernels
            .iter()
            .map(|row| KernelDescriptorInputV3 {
                kernel_id: row.kernel_id,
                logical_name: row.logical_name,
                entry_name: row.entry_name,
                descriptor_symbol: row.descriptor_symbol,
                source_evidence: row.source_evidence,
                executable_ir_evidence: row.executable_ir_evidence,
                capabilities: row.capabilities,
                abi_layout: row.abi_layout,
                launch: &launch,
                arguments: row.arguments,
            })
            .collect::<Vec<_>>();
        let input = DeviceDescriptorTableInputV5 {
            nominal: DeviceDescriptorTableInputV3 {
                kernels: &kernels,
                ..input.nominal
            },
            contracts: input.contracts,
        };
        let mut bytes =
            vec![0; encoded_device_descriptor_table_v5_len(&input, &mut inert::free).unwrap()];
        encode_device_descriptor_table_v5(&input, &mut bytes, &mut inert::free).unwrap();
        bytes
    })
}

fn fields<'a>(descriptor: &KernelDescriptorRefV5<'_, 'a>) -> ProductionSourceLaunchRootInputV1<'a> {
    ProductionSourceLaunchRootInputV1::new(
        descriptor.logical_name(),
        *descriptor.kernel_id().as_bytes(),
        Launch::new(1, Some([64, 1, 1]), [1024, 1, 1]),
    )
}

#[test]
fn conditional_final_component_both_target_full_binding_and_launch_rows() {
    for target in ["gfx942", "gfx950"] {
        let bytes = wire(
            target,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
        );
        let table = decode_device_descriptor_table_v5(&bytes, &mut inert::free).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, usize::MAX);
        let floor = bytes.len() + DESCRIPTOR_TABLE_VIEW_STORAGE_V5 + DESCRIPTOR_QUERY_STORAGE_V5;
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        for ordinal in 0..2 {
            let descriptor = table.kernel(ordinal, &mut |n| b.charge_work(n)).unwrap();
            let source = fields(&descriptor);
            let export = descriptor.entry_name().as_bytes();
            check_descriptor(source, export, &descriptor, &mut b).unwrap();
            let mut other_binding = source.kernel_binding();
            other_binding[31] ^= 1; // Same first diagnostic 64 bits is insufficient.
            let changed = [
                ProductionSourceLaunchRootInputV1::new(
                    source.logical_name(),
                    other_binding,
                    source.launch(),
                ),
                ProductionSourceLaunchRootInputV1::new(
                    "other-logical",
                    source.kernel_binding(),
                    source.launch(),
                ),
                ProductionSourceLaunchRootInputV1::new(
                    source.logical_name(),
                    source.kernel_binding(),
                    Launch::new(2, Some([64, 1, 1]), [1024, 1, 1]),
                ),
                ProductionSourceLaunchRootInputV1::new(
                    source.logical_name(),
                    source.kernel_binding(),
                    Launch::new(1, Some([32, 1, 1]), [1024, 1, 1]),
                ),
                ProductionSourceLaunchRootInputV1::new(
                    source.logical_name(),
                    source.kernel_binding(),
                    Launch::new(1, None, [1024, 1, 1]),
                ),
                ProductionSourceLaunchRootInputV1::new(
                    source.logical_name(),
                    source.kernel_binding(),
                    Launch::new(1, Some([64, 1, 1]), [512, 1, 1]),
                ),
            ];
            for row in changed {
                assert!(matches!(
                    check_descriptor(row, export, &descriptor, &mut b),
                    Err(Error(Cause::Mismatch(_)))
                ));
            }
            assert!(check_descriptor(source, b"another_export", &descriptor, &mut b).is_err());
            let other = table
                .kernel(1 - ordinal, &mut |n| b.charge_work(n))
                .unwrap();
            assert!(check_descriptor(source, export, &other, &mut b).is_err());
            assert_eq!(b.storage(), floor);
            assert!(b.work_ledger_identity_v1() == ledger);
        }
    }
}

#[test]
fn conditional_final_component_nonexact_block_modes_refused() {
    for block in [
        BlockSizeV1::Any,
        BlockSizeV1::AtMost(DimensionsV1::new(64, 1, 1).unwrap()),
    ] {
        let bytes = wire("gfx942", block);
        let table = decode_device_descriptor_table_v5(&bytes, &mut inert::free).unwrap();
        let descriptor = table.kernel(0, &mut inert::free).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, usize::MAX);
        b.reserve_storage(
            bytes.len() + DESCRIPTOR_TABLE_VIEW_STORAGE_V5 + DESCRIPTOR_QUERY_STORAGE_V5,
        )
        .unwrap();
        for exact in [None, Some([64, 1, 1])] {
            let base = fields(&descriptor);
            let source = ProductionSourceLaunchRootInputV1::new(
                base.logical_name(),
                base.kernel_binding(),
                Launch::new(1, exact, [1024, 1, 1]),
            );
            assert!(matches!(
                check_descriptor(
                    source,
                    descriptor.entry_name().as_bytes(),
                    &descriptor,
                    &mut b
                ),
                Err(Error(Cause::Mismatch(_)))
            ));
        }
    }
}

#[test]
fn conditional_final_component_descriptor_comparison_has_exact_work_boundary() {
    let bytes = wire(
        "gfx942",
        BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
    );
    let table = decode_device_descriptor_table_v5(&bytes, &mut inert::free).unwrap();
    let descriptor = table.kernel(0, &mut inert::free).unwrap();
    let source = fields(&descriptor);
    let export = descriptor.entry_name().as_bytes();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    let floor = bytes.len() + DESCRIPTOR_TABLE_VIEW_STORAGE_V5 + DESCRIPTOR_QUERY_STORAGE_V5;
    b.reserve_storage(floor).unwrap();
    check_descriptor(source, export, &descriptor, &mut b).unwrap();
    let exact = b.work();
    for work_limit in [exact - 1, exact] {
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, floor);
        b.reserve_storage(floor).unwrap();
        let result = check_descriptor(source, export, &descriptor, &mut b);
        if work_limit == exact {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(Error(Cause::Resource(Resource::Work(_))))
            ));
            assert!(b.failed_work().is_some());
        }
        assert_eq!(b.storage(), floor);
    }
}
