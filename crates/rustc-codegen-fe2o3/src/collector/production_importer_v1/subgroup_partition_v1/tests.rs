use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn identity(tag: u8) -> SemanticTypeIdentityV1 {
    SemanticTypeIdentityV1::from_sha256([tag; 32])
}

fn id(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn zst(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        identity(tag),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    )
}

fn lane_aggregate(tag: u8, fields: Vec<SemanticTypeIdV1>) -> SemanticTypeDeclV1 {
    let mut offsets = vec![4; fields.len()];
    offsets[0] = 0;
    SemanticTypeDeclV1::new(
        identity(tag),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(4),
            4,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}

fn reference(
    tag: u8,
    pointee: u32,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        identity(tag),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                id(pointee),
                kind,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn shared(tag: u8, pointee: u32) -> SemanticTypeDeclV1 {
    reference(
        tag,
        pointee,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    )
}

fn scalar(tag: u8, shape: SemanticScalarTypeV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        identity(tag),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
        SemanticTypeShapeV1::Scalar(shape),
    )
}

fn direct() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Direct(
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap(),
    )
}

fn abi_with(
    inputs: Vec<SemanticTypeIdV1>,
    output: SemanticTypeIdV1,
    output_mode: SemanticAbiPassModeV1,
    ownership: Vec<SemanticSourceArgumentOwnershipV1>,
) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([40; 32]),
        SemanticLayoutIdentityV1::from_sha256([41; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        u32::try_from(inputs.len()).unwrap(),
        inputs.clone(),
        output,
        inputs
            .into_iter()
            .map(|ty| SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(ty, direct())))
            .collect(),
        SemanticAbiValueV1::new(output, output_mode),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap()
}

struct Fixture {
    terminal: ProductionExecutionTerminalV1,
    types: Vec<SemanticTypeDeclV1>,
    abi: SemanticFunctionAbiV1,
    source: PartitionSourceV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    source_identity: SemanticFunctionIdentityV1,
}

impl Fixture {
    fn new(terminal: ProductionExecutionTerminalV1) -> Self {
        use ProductionExecutionTerminalV1 as T;
        let (marker, ownership) = partition_terminal_v1(terminal).unwrap();
        let types = vec![
            lane_aggregate(1, vec![id(8), id(9), id(9)]),
            shared(2, 0),
            zst(3),
            shared(4, 2),
            zst(5),
            shared(6, 4),
            scalar(7, SemanticScalarTypeV1::Float { bits: 32 }),
            scalar(
                8,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            lane_aggregate(9, vec![id(7), id(9), id(9), id(9)]),
            zst(10),
        ];
        let (inputs, output, receiver, epoch_borrow, mode) = match terminal {
            T::SubgroupPartitionDerive => (
                vec![id(1), id(3)],
                id(4),
                identity(1),
                Some(identity(3)),
                SemanticAbiPassModeV1::Ignore,
            ),
            T::SubgroupPartitionReduceSumF32 | T::SubgroupPartitionReduceMaxF32 => {
                (vec![id(5), id(6)], id(6), identity(5), None, direct())
            }
            T::SubgroupPartitionBroadcastF32 => (
                vec![id(5), id(6), id(7)],
                id(6),
                identity(5),
                None,
                direct(),
            ),
            _ => panic!("not a partition terminal"),
        };
        let source_identity = SemanticFunctionIdentityV1::from_sha256([30; 32]);
        let source = PartitionSourceV1 {
            terminal: marker,
            source_identity,
            inputs: inputs
                .iter()
                .map(|ty| types[ty.index() as usize].identity())
                .collect(),
            output: types[output.index() as usize].identity(),
            receiver,
            epoch_borrow,
            brand: identity(11),
            epoch: identity(12),
            kernel: identity(13),
        };
        Self {
            terminal,
            types,
            source,
            abi: abi_with(inputs, output, mode, ownership.to_vec()),
            provenance: SemanticKernelCapabilityProvenanceV1::new(
                SemanticFunctionIdV1::from_index(0),
                SemanticKernelBindingIdentityV1::from_sha256([20; 32]),
                SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([21; 32]),
                identity(13),
                SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([22; 32]),
                SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([23; 32]),
                SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([24; 32]),
            )
            .unwrap(),
            source_identity,
        }
    }

    fn expand(
        &self,
    ) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
        expand_partition_source_v1(
            self.terminal,
            &self.abi,
            &self.types,
            &self.source,
            self.provenance,
            self.source_identity,
        )
    }
}

const TERMINALS: [ProductionExecutionTerminalV1; 4] = [
    ProductionExecutionTerminalV1::SubgroupPartitionDerive,
    ProductionExecutionTerminalV1::SubgroupPartitionReduceSumF32,
    ProductionExecutionTerminalV1::SubgroupPartitionBroadcastF32,
    ProductionExecutionTerminalV1::SubgroupPartitionReduceMaxF32,
];

#[test]
fn partition_source_preserves_borrows_scalars_lane_and_epoch() {
    for terminal in TERMINALS {
        let fixture = Fixture::new(terminal);
        let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } =
            fixture.expand().unwrap()
        else {
            panic!("partition source must retain typed execution transport");
        };
        assert_eq!(
            contract.signature().arguments().collect::<Vec<_>>(),
            fixture.abi.source_input_types()
        );
        assert_eq!(
            contract.signature().output(),
            fixture.abi.source_output_type()
        );
        assert_eq!(contract.provenance(), fixture.provenance);
        assert_eq!(contract.source_identity(), fixture.source_identity);
        assert_eq!(contract.workgroup_brand(), Some(fixture.source.brand));
        assert_eq!(contract.epoch_before(), Some(fixture.source.epoch));
        assert_eq!(contract.epoch_after(), None);
        let SemanticExecutionCapabilityOperationV1::SubgroupPartition(operation) =
            contract.operation()
        else {
            panic!("partition source must not become an argument-free collective");
        };
        assert_eq!(operation.widths(), (64, 16));
        assert_eq!(matches!(operation, Partition::ReduceMaxF32 { .. }), terminal == ProductionExecutionTerminalV1::SubgroupPartitionReduceMaxF32);
        assert_eq!(contract.obligations().bits(), operation.obligations());
        match operation {
            Partition::Derive {
                subgroup_reference,
                subgroup,
                epoch,
                partition,
                ..
            } => {
                assert_eq!(
                    (subgroup_reference, subgroup, epoch, partition),
                    (id(1), id(0), id(3), id(4))
                );
            }
            Partition::ReduceSumF32 {
                partition_reference,
                partition,
                element,
                ..
            }
            | Partition::ReduceMaxF32 {
                partition_reference, partition, element, ..
            } => {
                assert_eq!(
                    (partition_reference, partition, element),
                    (id(5), id(4), id(6))
                );
            }
            Partition::BroadcastF32 {
                partition_reference,
                partition,
                element,
                source_lane,
                ..
            } => {
                assert_eq!(
                    (partition_reference, partition, element, source_lane),
                    (id(5), id(4), id(6), id(7))
                );
            }
        }
    }
}

#[test]
fn partition_source_rejects_missing_arguments_and_wrong_borrow_ownership() {
    for terminal in TERMINALS {
        let mut fixture = Fixture::new(terminal);
        fixture.abi = abi_with(
            vec![],
            fixture.abi.source_output_type(),
            fixture.abi.return_value().mode().clone(),
            vec![],
        );
        assert!(fixture.expand().is_err());
        let mut fixture = Fixture::new(terminal);
        fixture.abi = abi_with(
            fixture.abi.source_input_types().to_vec(),
            fixture.abi.source_output_type(),
            fixture.abi.return_value().mode().clone(),
            vec![
                SemanticSourceArgumentOwnershipV1::ByValue;
                fixture.abi.source_input_types().len()
            ],
        );
        assert!(fixture.expand().is_err());
    }
}

#[test]
fn partition_source_rejects_substituted_identity_marker_brand_epoch_and_root() {
    for terminal in TERMINALS {
        for mutation in 0..8 {
            let mut fixture = Fixture::new(terminal);
            match mutation {
                0 => fixture.source.terminal = TrustedDeviceItem::KernelContextIssue,
                1 => {
                    fixture.source.source_identity =
                        SemanticFunctionIdentityV1::from_sha256([31; 32])
                }
                2 => fixture.source.kernel = identity(14),
                3 => fixture.source.brand = identity(0),
                4 => fixture.source.epoch = identity(0),
                5 => fixture.source.receiver = identity(14),
                6 => fixture.source.inputs[0] = identity(14),
                7 => fixture.source.output = identity(14),
                _ => unreachable!(),
            }
            assert!(fixture.expand().is_err(), "accepted mutation {mutation}");
        }
    }
}

#[test]
fn partition_source_rejects_raw_mutable_or_substituted_receivers() {
    for terminal in TERMINALS {
        for mutation in 0..3 {
            let mut fixture = Fixture::new(terminal);
            let (reference_index, tag, pointee) =
                if terminal == ProductionExecutionTerminalV1::SubgroupPartitionDerive {
                    (1, 2, 0)
                } else {
                    (5, 6, 4)
                };
            fixture.types[reference_index] = match mutation {
                0 => reference(
                    tag,
                    pointee,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Immutable,
                ),
                1 => reference(
                    tag,
                    pointee,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
                ),
                2 => shared(tag, 2),
                _ => unreachable!(),
            };
            assert!(fixture.expand().is_err());
        }
    }
}

#[test]
fn partition_source_rejects_epoch_substitution_and_duplicate_nominal_types() {
    let mut fixture = Fixture::new(ProductionExecutionTerminalV1::SubgroupPartitionDerive);
    fixture.types[3] = shared(4, 0);
    assert!(fixture.expand().is_err());
    let mut fixture = Fixture::new(ProductionExecutionTerminalV1::SubgroupPartitionDerive);
    fixture.source.epoch_borrow = None;
    assert!(fixture.expand().is_err());
    for terminal in TERMINALS {
        let mut fixture = Fixture::new(terminal);
        fixture
            .types
            .push(fixture.types[fixture.abi.source_input_types()[0].index() as usize].clone());
        assert!(fixture.expand().is_err());
    }
}

#[test]
fn partition_consumers_reject_non_f32_values_and_non_u32_lanes() {
    for terminal in &TERMINALS[1..] {
        let mut fixture = Fixture::new(*terminal);
        fixture.types[6] = scalar(
            7,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        );
        assert!(fixture.expand().is_err());
    }
    let mut fixture = Fixture::new(ProductionExecutionTerminalV1::SubgroupPartitionBroadcastF32);
    fixture.types[7] = scalar(
        8,
        SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 32,
        },
    );
    assert!(fixture.expand().is_err());
}

#[test]
fn partition_derive_requires_live_lane_without_relaxing_partition_zst() {
    let mut fixture = Fixture::new(ProductionExecutionTerminalV1::SubgroupPartitionDerive);
    assert!(fixture.expand().is_ok());
    assert_eq!(fixture.types[0].layout().size_bytes(), Some(4));
    fixture.types[0] = zst(1);
    assert!(
        fixture.expand().is_err(),
        "a lane-free subgroup is not the source type"
    );
    for terminal in TERMINALS {
        let mut fixture = Fixture::new(terminal);
        fixture.types[4] = lane_aggregate(5, vec![id(8), id(9), id(9)]);
        assert!(fixture.expand().is_err(), "partition must still be a ZST");
    }
}
