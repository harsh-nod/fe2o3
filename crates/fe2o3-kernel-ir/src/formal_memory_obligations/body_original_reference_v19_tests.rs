#[allow(clippy::too_many_arguments)]
fn original_body_report_v19<R: effect_reader_v19::EffectReaderV19>(
    module: &Module,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
    canonical_v19: Option<&crate::VerifiedCanonicalKernelIrModuleV19>,
    composition: Option<&crate::VerifiedOrderedProgramCompositionV1>,
    effect_reader: &mut R,
    interpretation: PhysicalLaunchInterpretationV2,
) -> Result<FormalMemoryObligationAnalysis, effect_reader_v19::FormalEffectEngineErrorV19<R::Error>>
{
    let ordered_composition =
        composition.is_some_and(|owner| ordered_composition_v1::contains(owner, module, kernel_id));
    let complete_body_v19 = canonical_v19.is_some_and(|owner| {
        std::ptr::eq(module, owner.module())
            && complete_body_v19::contains_verified_complete_body(owner, kernel_id)
    });
    let kernel = module
        .kernels
        .iter()
        .find(|kernel| &kernel.id == kernel_id)
        .ok_or_else(|| FormalMemoryObligationError::MissingKernel {
            kernel: kernel_id.clone(),
        })?;
    let function = module
        .function(&kernel.entry)
        .expect("verified kernel entry must exist");
    let body = function
        .body
        .as_ref()
        .expect("verified kernel entry must be a definition");

    let mut reasons = BTreeSet::new();
    let index_width_supported = index_width == FormalIndexWidth::Bits64;
    if !index_width_supported {
        reasons.insert(FormalMemoryIncompleteReason::UnsupportedIndexWidth { width: index_width });
    }
    let invocations = original_resolve_invocations_v19(
        &kernel.domain,
        launch_extent,
        &mut reasons,
        interpretation,
    )?;
    let access_invocations = index_width_supported.then_some(invocations).flatten();
    let allocations = formal_allocations(function);
    let allocation_by_value: BTreeMap<_, _> = allocations
        .iter()
        .map(|allocation| (allocation.value, allocation.identity))
        .collect();
    let (definitions, guarded_control) = collect_definitions(function)?;
    if !body.blocks[0].parameters.is_empty() {
        reasons.insert(
            FormalMemoryIncompleteReason::UnsupportedEntryBlockParameters {
                block: definitions.entry,
            },
        );
    }
    let value_types = collect_types(function);
    let eligible_private_slots =
        classify_eligible_private_slots(function, &definitions, &value_types, &mut reasons);
    let private_load_sources = collect_private_load_sources(
        function,
        &definitions,
        &value_types,
        &eligible_private_slots,
    );
    let guarded = guarded_control
        .map(|control| {
            GuardedAnalysisV1::new(control, &definitions, function, kernel.domain.rank() == 1)
        })
        .transpose()?;
    let mut context = AccessDerivationContext::new(
        &definitions,
        &value_types,
        &allocations,
        &allocation_by_value,
        &private_load_sources,
        guarded,
    );
    let mut accesses = Vec::new();

    for block in &body.blocks {
        if !definitions.is_reachable(block.id) {
            continue;
        }
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, operation_index);
            let proven_private = match operation.kind {
                OperationKind::Load { pointer, access }
                | OperationKind::Store {
                    pointer, access, ..
                }
                | OperationKind::GuardedLoad {
                    pointer, access, ..
                }
                | OperationKind::GuardedStore {
                    pointer, access, ..
                } if access.address_space == AddressSpace::Generic => {
                    let exact = definitions.exact_ssa_origin(pointer, &value_types)
                            .and_then(|origin| value_types.get(&origin))
                            .is_some_and(|ty| matches!(ty, Type::Pointer(p) if p.address_space == AddressSpace::Private));
                    exact
                        || if let Some(guarded) = &mut context.guarded {
                            guarded.proven_pointer_space_v18(pointer)?
                                == Some(AddressSpace::Private)
                        } else {
                            false
                        }
                }
                _ => false,
            };
            match &operation.kind {
                OperationKind::Call { callee, .. }
                    if !ordered_composition
                        && !effect_reader
                            .is_complete_and_pure(operation)
                            .map_err(effect_reader_v19::FormalEffectEngineErrorV19::Reader)? =>
                {
                    reasons.insert(FormalMemoryIncompleteReason::CallEffectsUnavailable {
                        location,
                        callee: callee.clone(),
                    });
                }
                OperationKind::Call { .. } => {}
                OperationKind::Load { access, .. }
                    if access.address_space == AddressSpace::Private || proven_private => {}
                OperationKind::Load { pointer, access } => {
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Read,
                            *access,
                            invocations,
                            None,
                            &mut context,
                        ) {
                            Ok(access) => guarded_access_v1::report_push(
                                &mut context.guarded,
                                &mut accesses,
                                access,
                            )?,
                            Err(AccessDerivationError::Incomplete(reason)) => {
                                reasons.insert(reason);
                            }
                            Err(AccessDerivationError::Resource(error)) => return Err(error.into()),
                        }
                    }
                }
                OperationKind::Store { access, .. }
                    if access.address_space == AddressSpace::Private || proven_private => {}
                OperationKind::Store {
                    pointer, access, ..
                }
                | OperationKind::GuardedStore {
                    pointer, access, ..
                } => {
                    if proven_private {
                        continue;
                    }
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Write,
                            *access,
                            invocations,
                            match operation.kind {
                                OperationKind::GuardedStore { predicate, .. } => Some(predicate),
                                _ => None,
                            },
                            &mut context,
                        ) {
                            Ok(access) => guarded_access_v1::report_push(
                                &mut context.guarded,
                                &mut accesses,
                                access,
                            )?,
                            Err(AccessDerivationError::Incomplete(reason)) => {
                                reasons.insert(reason);
                            }
                            Err(AccessDerivationError::Resource(error)) => return Err(error.into()),
                        }
                    }
                }
                OperationKind::Alloca {
                    address_space: AddressSpace::Private,
                    ..
                } => {}
                OperationKind::Matrix(matrix) if matrix.memory_effects().is_empty() => {}
                // The verified gfx950 transpose chain owns its static LDS, accepts only a
                // read-only global U8 slice, and defines guarded zero-fill for every source
                // coordinate. It creates no caller-visible write or alias obligation.
                OperationKind::Gfx950LdsTranspose(_) => {}
                OperationKind::GuardedLoad { access, .. }
                    if access.address_space == AddressSpace::Private || proven_private => {}
                OperationKind::GuardedLoad {
                    pointer,
                    access,
                    predicate,
                    ..
                } => {
                    let mut checked_guard = false;
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Read,
                            *access,
                            invocations,
                            Some(*predicate),
                            &mut context,
                        ) {
                            Ok(access) => {
                                checked_guard =
                                    matches!(access.domain, FormalAccessDomainV1::SliceBounded(_));
                                guarded_access_v1::report_push(
                                    &mut context.guarded,
                                    &mut accesses,
                                    access,
                                )?;
                            }
                            Err(AccessDerivationError::Incomplete(exact_reason)) => {
                                match derive_conservative_guarded_access(
                                    location,
                                    *pointer,
                                    *access,
                                    invocations,
                                    &mut context,
                                ) {
                                    Ok(access) => guarded_access_v1::report_push(
                                        &mut context.guarded,
                                        &mut accesses,
                                        access,
                                    )?,
                                    Err(_) => {
                                        reasons.insert(exact_reason);
                                    }
                                }
                            }
                            Err(AccessDerivationError::Resource(error)) => return Err(error.into()),
                        }
                    }
                    if !checked_guard {
                        reasons.insert(
                            FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof {
                                location,
                            },
                        );
                    }
                }
                OperationKind::Atomic(atomic) => {
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            atomic.pointer,
                            FormalMemoryAccessKind::Atomic,
                            atomic.access,
                            invocations,
                            None,
                            &mut context,
                        ) {
                            Ok(access) => guarded_access_v1::report_push(
                                &mut context.guarded,
                                &mut accesses,
                                access,
                            )?,
                            Err(AccessDerivationError::Incomplete(reason)) => {
                                reasons.insert(reason);
                            }
                            Err(AccessDerivationError::Resource(error)) => return Err(error.into()),
                        }
                    }
                }
                OperationKind::InlineAssembly(_)
                    if gfx942_inline_u32_v30::has_closed_memory_effects(
                        operation,
                        &value_types,
                    ) => {}
                // U32 declaration/steps have no address or memory effect only
                // in this actual immutable whole-profile V19 context. Their
                // compiler ordering is unchanged; the real tail is above.
                OperationKind::Gfx942CompleteBodyDeclaration(_)
                | OperationKind::Gfx942CompleteBodyStep(_)
                    if complete_body_v19 => {}
                OperationKind::Gfx942OrderedProgram(_) if ordered_composition => {}
                OperationKind::Storage(_)
                | OperationKind::Execution(_)
                | OperationKind::Gfx942OrderedRegion(_)
                | OperationKind::Gfx942OrderedProgram(_)
                | OperationKind::Gfx942CompleteBodyDeclaration(_)
                | OperationKind::Gfx942CompleteBodyStep(_)
                | OperationKind::Gfx942PhysicalEntryDeclaration(_)
                | OperationKind::Gfx942PhysicalEntryStep(_)
                | OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_)
                | OperationKind::Gfx942PhysicalGlobalCopyStep(_)
                | OperationKind::Gfx942PhysicalLdsExchangeDeclaration(_)
                | OperationKind::Gfx942PhysicalLdsExchangeStep(_)
                | OperationKind::VerificationContract(_)
                | OperationKind::VectorLoad(_)
                | OperationKind::VectorStore(_)
                | OperationKind::VectorLayoutConvert(_)
                | OperationKind::Alloca { .. }
                | OperationKind::Barrier(_)
                | OperationKind::Fence(_)
                | OperationKind::Matrix(_)
                | OperationKind::InlineAssembly(_)
                | OperationKind::WorkgroupBarrier(_)
                | OperationKind::WorkgroupMemory(_) => {
                    reasons
                        .insert(FormalMemoryIncompleteReason::UnsupportedMemoryEffect { location });
                }
                OperationKind::Constant(_)
                | OperationKind::Intrinsic(_)
                | OperationKind::MemoryIntrinsic(_)
                | OperationKind::Wave(_)
                | OperationKind::Unary { .. }
                | OperationKind::Binary { .. }
                | OperationKind::Compare { .. }
                | OperationKind::Cast { .. }
                | OperationKind::Select { .. }
                | OperationKind::SliceLength { .. }
                | OperationKind::SliceData { .. }
                | OperationKind::GetElementPointer { .. } => {
                    if !operation.memory_effects().is_empty() {
                        reasons.insert(FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
                            location,
                        });
                    }
                }
            }
        }
    }

    let bounds_requirements =
        derive_bounds_requirements(&accesses, &mut reasons, &mut context.guarded)?;
    let runtime_alias_requirements = derive_alias_requirements(&accesses, &mut context.guarded)?;
    let inter_invocation_conflicts =
        derive_inter_invocation_conflicts(&accesses, &mut context.guarded)?;
    let obligations = FormalMemoryObligations {
        kernel: kernel.id.clone(),
        entry: kernel.entry.clone(),
        index_width,
        invocations,
        allocations,
        accesses,
        bounds_requirements,
        runtime_alias_requirements,
        inter_invocation_conflicts,
    };

    if reasons.is_empty() {
        Ok(FormalMemoryObligationAnalysis::Complete(obligations))
    } else {
        Ok(FormalMemoryObligationAnalysis::Incomplete {
            partial: obligations,
            reasons: reasons.into_iter().collect(),
        })
    }
}
