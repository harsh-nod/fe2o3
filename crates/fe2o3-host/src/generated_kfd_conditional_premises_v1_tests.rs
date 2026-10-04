mod conditional_premises_tests {
    use super::*;
    use fe2o3_artifacts::*;
    use fe2o3_kernel_descriptor::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    fn free(_: usize) -> Result<(), Resource> {
        Ok(())
    }
    fn wire(access_domain: ConditionalAddressDomainV1) -> Vec<u8> {
        wire_scalar(access_domain, ScalarTypeV1::U32, "count", 32)
    }

    fn wire_scalar(
        access_domain: ConditionalAddressDomainV1,
        scalar: ScalarTypeV1,
        name: &str,
        offset: u32,
    ) -> Vec<u8> {
        let sources = [
            SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::I32),
            SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::I32),
            SourceTypeDescriptorV3::Scalar(scalar),
        ]
        .map(|s| SourceTypeRecordV3::new(s, &mut free).unwrap());
        let layouts = [
            DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::I32),
            DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::I32),
            DeviceLayoutDescriptorV1::scalar(scalar),
        ]
        .map(|l| device_layout_record_v3(l, &mut free).unwrap());
        let arguments = [
            ConditionalArgumentBindingV1 {
                canonical_parameter: 7,
                source_argument: 0,
                adjusted_argument: 0,
                semantic_local: 1,
                semantic_type: 1,
                generated_field: 0,
                role: ConditionalArgumentRoleV1::Input,
                source_type_identity: *sources[0].identity().as_bytes(),
                device_layout_identity: *layouts[0].identity().as_bytes(),
            },
            ConditionalArgumentBindingV1 {
                canonical_parameter: 19,
                source_argument: 1,
                adjusted_argument: 1,
                semantic_local: 2,
                semantic_type: 2,
                generated_field: 1,
                role: ConditionalArgumentRoleV1::Output,
                source_type_identity: *sources[1].identity().as_bytes(),
                device_layout_identity: *layouts[1].identity().as_bytes(),
            },
        ];
        let subjects = ConditionalSubjectsV1 {
            kernel_id: [0x42; 32],
            exact_graph_identity: [1; 32],
            aggregate_statement_identity: [2; 32],
            source_semantic_identity: [3; 32],
            reference_kind: ConditionalReferenceKindV1::Mir,
            safe_reference_identity: [4; 32],
            safe_reference_source_hash: [0; 32],
            safe_reference_mir_hash: [5; 32],
            kernel_subject_identity: [6; 32],
            kernel_mir_hash: [7; 32],
        };
        let mut theorem = ConditionalTheoremV1 {
            statement_identity: [0; 32],
            generated_source_identity: [8; 32],
            execution_identity: [9; 32],
            receipt_identity: [10; 32],
            staging_receipt_identity: [11; 32],
            staging_obligation_identity: [12; 32],
            staging_signer_identity: [13; 32],
            staging_execution_identity: [14; 32],
        };
        let mut hash = Sha256::new();
        hash.update(CONDITIONAL_MEMORY_THEOREM_DOMAIN_V1);
        for d in [
            subjects.aggregate_statement_identity,
            theorem.generated_source_identity,
            theorem.staging_receipt_identity,
            theorem.staging_obligation_identity,
            theorem.staging_signer_identity,
            theorem.staging_execution_identity,
        ] {
            hash.update(d);
        }
        theorem.statement_identity = hash.finalize().into();
        let output = ConditionalOutputV1 {
            argument: 1,
            canonical_store: ConditionalCanonicalLocationV1 {
                block: 1,
                operation: 0,
            },
            ranked_store: ConditionalRankedLocationV1 {
                block: 1,
                operation: 1,
            },
            ranked_effect: ConditionalRankedLocationV1 {
                block: 1,
                operation: 2,
            },
            element_bytes: 4,
            alignment: 4,
            address_domain: ConditionalAddressDomainV1::GuardedOutput,
        };
        let reads = [ConditionalReadOccurrenceV1 {
            argument: 0,
            canonical: ConditionalCanonicalLocationV1 {
                block: 0,
                operation: 1,
            },
            slice: 0,
            pointer: 1,
            index: 2,
            value: 3,
            ranked: ConditionalRankedLocationV1 {
                block: 0,
                operation: 1,
            },
            ranked_view: ConditionalRankedValueV1::Argument(0),
            ranked_index: ConditionalRankedValueV1::Local(1),
            access_domain,
            address_domain: ConditionalAddressDomainV1::GlobalLaunch,
            element_bytes: 4,
            alignment: 4,
        }];
        let premises = [
            ConditionalRuntimePremiseV1::D1Launch,
            ConditionalRuntimePremiseV1::OutputWithinGlobalX { parameter: 19 },
            ConditionalRuntimePremiseV1::WritableOutput { parameter: 19 },
            ConditionalRuntimePremiseV1::RepresentableAddress {
                parameter: 19,
                domain: output.address_domain,
                element_bytes: 4,
                alignment: 4,
            },
            ConditionalRuntimePremiseV1::ReadableInput {
                parameter: 7,
                domain: access_domain,
            },
            ConditionalRuntimePremiseV1::SeparateInputOutput {
                input: 7,
                output: 19,
            },
            ConditionalRuntimePremiseV1::RepresentableAddress {
                parameter: 7,
                domain: reads[0].address_domain,
                element_bytes: 4,
                alignment: 4,
            },
        ];
        let contract_input = ConditionalInvocationContractInputV1 {
            numerical_domain: ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1,
            subjects,
            theorem,
            typed_roots: &[[1, 2, 3, 4]],
            arguments: &arguments,
            output,
            reads: &reads,
            premises: &premises,
        };
        let n = encoded_conditional_invocation_contract_v1_len(&contract_input, &mut free).unwrap();
        let mut contract_bytes = vec![0; n];
        encode_conditional_invocation_contract_v1(&contract_input, &mut contract_bytes, &mut free)
            .unwrap();
        let contract =
            decode_conditional_invocation_contract_v1(&contract_bytes, &mut free).unwrap();

        let pointer = |offset, access, alias| PhysicalComponentV3 {
            kind: PhysicalAbiComponentKind::GlobalPointer,
            offset,
            size: 8,
            alignment: 8,
            access,
            alias,
        };
        let length = |offset| PhysicalComponentV3 {
            kind: PhysicalAbiComponentKind::SliceLengthU64,
            offset,
            size: 8,
            alignment: 8,
            access: AccessMode::ByValue,
            alias: AliasSemantics::Value,
        };
        let input_components = [
            pointer(0, AccessMode::ReadOnly, AliasSemantics::SharedReadOnly),
            length(8),
        ];
        let output_components = [
            pointer(16, AccessMode::ReadWrite, AliasSemantics::Exclusive),
            length(24),
        ];
        let scalar_components = [PhysicalComponentV3 {
            kind: PhysicalAbiComponentKind::ScalarByValue(scalar),
            offset,
            size: 4,
            alignment: 4,
            access: AccessMode::ByValue,
            alias: AliasSemantics::Value,
        }];
        let fields = [
            LogicalArgumentInputV3 {
                source_index: 0,
                name: "input",
                source_type: sources[0].identity(),
                device_layout: layouts[0].identity(),
                ownership: OwnershipSemantics::SharedBorrow,
                access: AccessMode::ReadOnly,
                alias: AliasSemantics::SharedReadOnly,
                components: &input_components,
            },
            LogicalArgumentInputV3 {
                source_index: 1,
                name: "output",
                source_type: sources[1].identity(),
                device_layout: layouts[1].identity(),
                ownership: OwnershipSemantics::UniqueBorrow,
                access: AccessMode::ReadWrite,
                alias: AliasSemantics::Exclusive,
                components: &output_components,
            },
            LogicalArgumentInputV3 {
                source_index: 2,
                name,
                source_type: sources[2].identity(),
                device_layout: layouts[2].identity(),
                ownership: OwnershipSemantics::ByValue,
                access: AccessMode::ByValue,
                alias: AliasSemantics::Value,
                components: &scalar_components,
            },
        ];
        let launch = LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Any,
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            1024,
            0,
            0,
        )
        .unwrap();
        let id = KernelId::from_bytes([0x42; 32]);
        let evidence = BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([1; 32]),
            EvidenceDigest::from_sha256_bytes([2; 32]),
        );
        let kernels = [KernelDescriptorInputV3 {
            kernel_id: id,
            logical_name: "conditional",
            entry_name: "conditional",
            descriptor_symbol: "conditional.kd",
            source_evidence: evidence,
            executable_ir_evidence: evidence,
            capabilities: &[CapabilityV1::AmdWave],
            abi_layout: KernelAbiLayoutV1::new(40, 296, 8).unwrap(),
            launch: &launch,
            arguments: &fields,
        }];
        let requirements = [KernelTargetRequirementsV2::new(
            id,
            LdsRequirementsV2::new(0, 0).unwrap(),
            RequiredWavefrontWidthV2::Wave64,
            false,
            SynchronizationRequirementsV2::from_bits(0).unwrap(),
            AtomicRequirementsV2::from_bits(0).unwrap(),
        )];
        let mut sorted_sources = sources.to_vec();
        sorted_sources.sort_by_key(|r| r.identity());
        let mut sorted_layouts = layouts.to_vec();
        sorted_layouts.sort_by_key(|r| r.identity());
        let compiler = CompilerIdentityV1::new(
            Text::new("rustc").unwrap(),
            Text::new("conditional-test").unwrap(),
            [7; 20],
        );
        let producer = ProducerIdentityV1::new(
            Text::new("fe2o3").unwrap(),
            Text::new("conditional-test").unwrap(),
        );
        let input = DeviceDescriptorTableInputV4 {
            nominal: DeviceDescriptorTableInputV3 {
                canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
                code_object_version: CodeObjectVersion::V6,
                compiler: &compiler,
                producer: &producer,
                device_target: DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
                type_records: &sorted_sources,
                layout_records: &sorted_layouts,
                kernels: &kernels,
                requirements: &requirements,
            },
            contracts: &[contract],
        };
        let n = encoded_device_descriptor_table_v4_len(&input, &mut free).unwrap();
        let mut bytes = vec![0; n];
        encode_device_descriptor_table_v4(&input, &mut bytes, &mut free).unwrap();
        bytes
    }

    fn packed<'a>(
        input: &'a [i32],
        output: &'a mut [i32],
        budget: &mut Budget<'_>,
    ) -> GeneratedKfdPackedArguments<'a> {
        let plan = plan();
        let (packed, retained) = GeneratedKfdArgumentBinding::from_compiler_generated_parts(
            vec![plan.scalar(2, 9_u32).unwrap()],
            vec![
                GeneratedKfdReadSlice::new(input)
                    .bind_argument(&plan, 0)
                    .unwrap(),
                GeneratedKfdReadWriteSlice::new(output)
                    .bind_argument(&plan, 1)
                    .unwrap(),
            ],
        )
        .pack_with_conditional_plan_v1(&plan, budget)
        .unwrap();
        budget.reserve_storage(retained).unwrap();
        packed
    }
    fn geometry() -> AqlDispatchGeometryV1 {
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap()
    }

    #[test]
    fn actual_generated_pack_binds_v4_contract_and_moves_completion_custody() {
        let bytes = wire(ConditionalAddressDomainV1::GuardedOutput);
        let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
        let mut work = Work::new(100_000_000);
        let mut budget = Budget::new(&mut work, 4_000_000);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let value = packed(&input, &mut output, &mut budget);
        let plan_storage = value.source_plan_storage;
        let bound = value
            .bind_conditional_premises_v1(&table, geometry(), &mut budget)
            .unwrap();
        assert!(bound.retained_storage() > 0);
        let retained = bound.retained_storage();
        budget.reserve_storage(retained).unwrap();
        let (packed, premises) = bound.into_parts();
        let request = fe2o3_kfd::Gfx942KfdDispatchRequestV1::new(
            vec![0; 128],
            64,
            packed.explicit_kernarg.clone(),
            8,
            packed
                .buffers
                .iter()
                .map(|b| fe2o3_kfd::Gfx942KfdDispatchBufferV1::new(b.bytes().to_vec()).unwrap())
                .collect(),
            packed.pointer_fixups.clone(),
            geometry(),
            0,
            0,
            1000,
        )
        .unwrap();
        assert!(request.with_conditional_premises_v1(premises).is_ok());
        assert_eq!(packed.completion.buffers.len(), 2);
        drop(packed);
        budget.release_storage(retained).unwrap();
        budget.release_storage(plan_storage).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn runtime_transition_keeps_payload_completion_and_original_storage_custody() {
        use fe2o3_runtime::Gfx942RuntimeInvocationBindingV1 as Binding;
        let bytes = wire(ConditionalAddressDomainV1::GuardedOutput);
        let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
        let mut work = Work::new(100_000_000);
        let mut budget = Budget::new(&mut work, 4_000_000);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let value = packed(&input, &mut output, &mut budget);
        let plan_storage = value.source_plan_storage;
        let bound = value
            .bind_conditional_premises_v1(&table, geometry(), &mut budget)
            .unwrap();
        let retained = bound.retained_storage();
        let kernel = table
            .find_kernel(KernelId::from_bytes([0x42; 32]), &mut free)
            .unwrap();
        let contract = kernel.conditional_contract(&mut free).unwrap();
        let expected_contract = *contract.identity().as_bytes();
        budget.reserve_storage(retained).unwrap();
        let (runtime, completion) = bound.into_runtime_inputs(geometry(), 0, 1000).unwrap();
        budget.release_storage(plan_storage).unwrap();
        assert!(matches!(runtime.invocation_binding(),
            Binding::ConditionalNominalV4 { contract_identity, .. }
                if contract_identity == expected_contract
        ));
        assert_eq!(completion.buffers.len(), 2);
        assert_eq!(budget.storage(), retained);
        drop((runtime, completion));
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 0);

        let packed = packed(&input, &mut output, &mut budget);
        let plan_storage = packed.source_plan_storage;
        let (ordinary, completion) = packed.into_runtime_inputs(geometry(), 0, 1000);
        budget.release_storage(plan_storage).unwrap();
        assert_eq!(ordinary.invocation_binding(), Binding::OrdinaryV1);
        assert_eq!(completion.buffers.len(), 2);
    }

    #[test]
    fn generated_host_refuses_underlength_global_reads_but_allows_guarded_empty() {
        for (domain, length, accepted) in [
            (ConditionalAddressDomainV1::GlobalLaunch, 3, false),
            (ConditionalAddressDomainV1::GuardedOutput, 0, true),
            (ConditionalAddressDomainV1::GuardedOutput, 65, false),
        ] {
            let bytes = wire(domain);
            let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
            let mut work = Work::new(100_000_000);
            let mut budget = Budget::new(&mut work, 4_000_000);
            let input = vec![1; length];
            let mut output = vec![0; length];
            let value = packed(&input, &mut output, &mut budget);
            let plan_storage = value.source_plan_storage;
            let result = value.bind_conditional_premises_v1(&table, geometry(), &mut budget);
            assert_eq!(result.is_ok(), accepted);
            drop(result);
            budget.release_storage(plan_storage).unwrap();
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn generated_host_rejects_wrong_buffer_field_access_bytes_and_kernel() {
        let bytes = wire(ConditionalAddressDomainV1::GuardedOutput);
        let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
        for mutation in 0..5 {
            let mut work = Work::new(100_000_000);
            let mut budget = Budget::new(&mut work, 4_000_000);
            let input = [1, 2, 3];
            let mut output = [0; 3];
            let mut value = packed(&input, &mut output, &mut budget);
            let floor = budget.storage();
            match mutation {
                0 => value.packing_observation.buffers[0].buffer_index = Some(1),
                1 => value.packing_observation.buffers[0].argument_index = 1,
                2 => {
                    value.packing_observation.buffers[0].access =
                        Some(Gfx942RuntimeBufferAccessV1::ReadWrite)
                }
                3 => value.explicit_kernarg[8] = 1,
                _ => value.kernel_id = KernelId::from_bytes([3; 32]),
            }
            assert!(
                value
                    .bind_conditional_premises_v1(&table, geometry(), &mut budget)
                    .is_err()
            );
            assert_eq!(budget.storage(), floor);
            budget.release_storage(floor).unwrap();
        }
    }

    #[test]
    fn generated_host_fails_closed_on_work_or_storage_exhaustion() {
        let bytes = wire(ConditionalAddressDomainV1::GuardedOutput);
        let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
        for (work_limit, storage) in [(0, 4_000_000), (100_000_000, 0), (100, 4_000_000)] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage);
            let plan = plan();
            let input = [1];
            let mut output = [0];
            let binding = argument_binding(&plan, &input, &mut output);
            let result =
                budget.with_prepaid_scope(0, 1, 1, 0, |budget| -> conditional::Result<()> {
                    let (packed, retained) =
                        binding.pack_with_conditional_plan_v1(&plan, budget)?;
                    budget.reserve_storage(retained)?;
                    drop(packed.bind_conditional_premises_v1(&table, geometry(), budget)?);
                    Ok(())
                });
            assert!(result.is_err());
            assert_eq!(budget.storage(), 0);
        }
    }

    fn argument_binding<'a>(
        plan: &GeneratedArgumentPackingPlanV1,
        input: &'a [i32],
        output: &'a mut [i32],
    ) -> GeneratedKfdArgumentBinding<'a> {
        GeneratedKfdArgumentBinding::from_compiler_generated_parts(
            vec![plan.scalar(2, 9_u32).unwrap()],
            vec![
                GeneratedKfdReadSlice::new(input)
                    .bind_argument(plan, 0)
                    .unwrap(),
                GeneratedKfdReadWriteSlice::new(output)
                    .bind_argument(plan, 1)
                    .unwrap(),
            ],
        )
    }

    #[test]
    fn complete_abi_rejects_non_premise_scalar_type_name_and_offset_substitutions() {
        // All are structurally valid V4 tables with the same conditional slice
        // contract, total ABI size and field/component counts.
        for (scalar, name, offset) in [
            (ScalarTypeV1::I32, "count", 32),
            (ScalarTypeV1::F32, "count", 32),
            (ScalarTypeV1::U32, "other", 32),
            (ScalarTypeV1::U32, "count", 36),
        ] {
            let bytes = wire_scalar(
                ConditionalAddressDomainV1::GuardedOutput,
                scalar,
                name,
                offset,
            );
            let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
            let mut work = Work::new(100_000_000);
            let mut budget = Budget::new(&mut work, 4_000_000);
            let input = [1, 2, 3];
            let mut output = [71; 3];
            let value = packed(&input, &mut output, &mut budget);
            let floor = budget.storage();
            assert!(matches!(
                value.bind_conditional_premises_v1(&table, geometry(), &mut budget),
                Err(conditional::GeneratedConditionalPremiseErrorV1::Binding(_))
            ));
            assert_eq!(budget.storage(), floor);
            budget.release_storage(floor).unwrap();
            assert_eq!(output, [71; 3]);
        }
    }

    #[test]
    fn full_packing_observation_and_sealed_plan_are_required() {
        let bytes = wire(ConditionalAddressDomainV1::GuardedOutput);
        let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
        for mutation in 0..3 {
            let mut work = Work::new(100_000_000);
            let mut budget = Budget::new(&mut work, 4_000_000);
            let input = [1];
            let mut output = [0];
            let mut value = packed(&input, &mut output, &mut budget);
            let floor = budget.storage();
            match mutation {
                0 => {
                    value.source_plan = None;
                }
                1 => {
                    value.packing_observation.components.pop();
                }
                _ => {
                    value.packing_observation.components.swap(0, 4);
                }
            }
            assert!(matches!(
                value.bind_conditional_premises_v1(&table, geometry(), &mut budget),
                Err(conditional::GeneratedConditionalPremiseErrorV1::Binding(_))
            ));
            assert_eq!(budget.storage(), floor);
            budget.release_storage(floor).unwrap();
        }
    }

    #[test]
    fn retaining_plan_does_not_change_ordinary_bytes_observation_or_family() {
        let plan = plan();
        let input = [4, 5, 6];
        let mut output_a = [0; 3];
        let mut output_b = [0; 3];
        let ordinary = argument_binding(&plan, &input, &mut output_a)
            .pack(&plan)
            .unwrap();
        assert!(ordinary.source_plan.is_none());
        assert_eq!(ordinary.source_plan_storage, 0);
        let mut work = Work::new(100_000_000);
        let mut budget = Budget::new(&mut work, 4_000_000);
        budget.reserve_storage(73).unwrap();
        let (retained, addition) = argument_binding(&plan, &input, &mut output_b)
            .pack_with_conditional_plan_v1(&plan, &mut budget)
            .unwrap();
        assert_eq!(budget.storage(), 73);
        budget.reserve_storage(addition).unwrap();
        assert_eq!(ordinary.explicit_kernarg(), retained.explicit_kernarg());
        assert_eq!(ordinary.pointer_fixups(), retained.pointer_fixups());
        assert_eq!(
            ordinary.packing_observation.identity(),
            retained.packing_observation.identity()
        );
        let (ordinary, completion_a) = ordinary.into_runtime_inputs(geometry(), 0, 1000);
        let (retained, completion_b) = retained.into_runtime_inputs(geometry(), 0, 1000);
        assert_eq!(ordinary.invocation_binding(), retained.invocation_binding());
        assert_eq!(
            retained.invocation_binding(),
            fe2o3_runtime::Gfx942RuntimeInvocationBindingV1::OrdinaryV1
        );
        budget.release_storage(addition).unwrap();
        assert_eq!(budget.storage(), 73);
        drop((ordinary, retained, completion_a, completion_b));
    }

    #[test]
    fn exact_and_one_short_conditional_work_and_storage_preserve_inherited_account() {
        fn run(work_limit: usize, storage_limit: usize) -> (bool, usize, usize, bool, bool) {
            let bytes = wire(ConditionalAddressDomainV1::GuardedOutput);
            let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
            let plan = plan();
            let input = [1, 2, 3];
            let mut output = [81; 3];
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(19).unwrap();
            budget.reserve_storage(73).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result =
                budget.with_prepaid_scope(73, 1, 1, 0, |budget| -> conditional::Result<()> {
                    let (packed, plan_storage) = argument_binding(&plan, &input, &mut output)
                        .pack_with_conditional_plan_v1(&plan, budget)?;
                    budget.reserve_storage(plan_storage)?;
                    let bound = packed.bind_conditional_premises_v1(&table, geometry(), budget)?;
                    let retained = bound.retained_storage();
                    budget.reserve_storage(retained)?;
                    let runtime = bound
                        .into_runtime_inputs(geometry(), 0, 1000)
                        .map_err(|_| {
                            conditional::GeneratedConditionalPremiseErrorV1::Binding(
                                "runtime transition",
                            )
                        })?;
                    // The generated owner drops its plan during this move. Payload
                    // and completion remain covered until their explicit drop.
                    budget.release_storage(plan_storage)?;
                    assert_eq!(budget.storage(), 73 + retained);
                    drop(runtime);
                    budget.release_storage(retained)?;
                    Ok(())
                });
            assert!(ledger == budget.work_ledger_identity_v1());
            assert_eq!(budget.storage(), 73);
            assert_eq!(output, [81; 3]);
            (
                result.is_ok(),
                budget.work(),
                budget.peak_storage(),
                budget.failed_work().is_some(),
                budget.failed_storage().is_some(),
            )
        }
        let (ok, work, storage, _, _) = run(100_000_000, 4_000_000);
        assert!(ok);
        assert_eq!(run(work, storage), (true, work, storage, false, false));
        let short_work = run(work - 1, storage);
        assert!(!short_work.0 && short_work.3);
        let short_storage = run(work, storage - 1);
        assert!(!short_storage.0 && short_storage.4);
    }

    #[test]
    fn packing_error_and_unwind_restore_scope_without_copyback_or_work_refund() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let bytes = wire(ConditionalAddressDomainV1::GuardedOutput);
        let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
        let plan = plan();
        let input = [1];
        let mut output = [91];
        let mut work = Work::new(100_000_000);
        let mut budget = Budget::new(&mut work, 4_000_000);
        budget.reserve_storage(73).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let mut bad_binding = argument_binding(&plan, &input, &mut output);
        bad_binding.scalar_inputs.clear();
        assert!(matches!(
            bad_binding.pack_with_conditional_plan_v1(&plan, &mut budget),
            Err(conditional::GeneratedConditionalPremiseErrorV1::Packing(_))
        ));
        assert_eq!(budget.storage(), 73);
        let work_after_error = budget.work();
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _: conditional::Result<()> = budget.with_prepaid_scope(73, 1, 1, 0, |budget| {
                let (packed, storage) = argument_binding(&plan, &input, &mut output)
                    .pack_with_conditional_plan_v1(&plan, budget)?;
                budget.reserve_storage(storage)?;
                let bound = packed.bind_conditional_premises_v1(&table, geometry(), budget)?;
                budget.reserve_storage(bound.retained_storage())?;
                let _owner = bound;
                panic!("caller unwind while retaining generated plan, premises and completion")
            });
        }));
        assert!(unwind.is_err());
        assert!(ledger == budget.work_ledger_identity_v1());
        assert_eq!(budget.storage(), 73);
        assert!(budget.work() > work_after_error);
        assert_eq!(output, [91]);
    }
}
