fn collect_contracts(
    context: &Context,
    inventory: &crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1,
    observer: OwnershipObserverV1<'_, '_, '_>,
) -> Result<Vec<ContractV1>, Box<HierarchicalOwnershipFindingV1>> {
    let mut contracts = Vec::new();
    let mut by_view = HashMap::new();
    for site in inventory.operations() {
        let block = site.block();
        let operation = site.operation();
        let op = Operation::get_op_dyn(site.pointer(), context);
        let Some(contract) = op.downcast_ref::<OwnershipContractOp>() else {
            continue;
        };
        if contracts.len() == MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1 {
            observe_ownership_quota_v1(observer, "ownership contract limit");
            return Err(Box::new(
                HierarchicalOwnershipFindingV1::ContractLimitExceeded {
                    actual: contracts.len() + 1,
                    limit: MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1,
                },
            ));
        }
        let location = HierarchicalOwnershipLocationV1 { block, operation };
        let raw = contract.get_operation().deref(context);
        if raw.get_num_operands() != 1
            || contract.coverage(context).is_none()
            || contract.partition(context).is_none()
        {
            return Err(Box::new(
                HierarchicalOwnershipFindingV1::MalformedContract {
                    location,
                    detail: "expected one ranked-view operand and closed coverage/partition attributes",
                },
            ));
        }
        let view = contract.view(context);
        let view_name = view.unique_name(context).to_string();
        if block != 0 {
            return Err(Box::new(
                HierarchicalOwnershipFindingV1::ContractOutsideEntry {
                    view: view_name,
                    location,
                },
            ));
        }
        if let Some(first) = by_view.insert(view, location) {
            return Err(Box::new(
                HierarchicalOwnershipFindingV1::DuplicateContract {
                    view: view_name,
                    first,
                    second: location,
                },
            ));
        }
        let Some(definition) = view.defining_op() else {
            return Err(Box::new(
                HierarchicalOwnershipFindingV1::SparseIndexAnalysisIncomplete {
                    detail: format!("contracted view {view_name} has no definition"),
                },
            ));
        };
        let definition = Operation::get_op_dyn(definition, context);
        let Some(view_op) = definition.downcast_ref::<RankedViewOp>() else {
            return Err(Box::new(
                HierarchicalOwnershipFindingV1::SparseIndexAnalysisIncomplete {
                    detail: format!("contracted value {view_name} is not a ranked view"),
                },
            ));
        };
        contracts.push(ContractV1 {
            location,
            view,
            view_name,
            view_op: *view_op,
            coverage: contract
                .coverage(context)
                .unwrap_or(OwnershipCoverageAttr::ExactView),
            partition: contract
                .partition(context)
                .unwrap_or(OwnershipPartitionAttr::ExactSets),
        });
    }
    Ok(contracts)
}

fn validate_effect_domain_site(
    context: &Context,
    inventory: &crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1,
    contract: &ContractV1,
) -> Result<(), Box<HierarchicalOwnershipFindingV1>> {
    let mut writes = 0_usize;
    for site in inventory.operations() {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let Some(access) = operation.downcast_ref::<RankedAccessOp>() else {
            continue;
        };
        if access.view(context) == contract.view
            && access
                .kind(context)
                .is_some_and(|kind| kind.writes_memory())
        {
            writes = writes.saturating_add(1);
        }
    }
    if writes != 1 {
        return Err(Box::new(
            HierarchicalOwnershipFindingV1::MalformedContract {
                location: contract.location,
                detail: "ExactEffectDomain requires exactly one bounds-clean, race-free write site on the contracted view",
            },
        ));
    }
    Ok(())
}

fn resolve_extents(
    context: &Context,
    sparse: &crate::SparseIndexAnalysisV1,
    contract: &ContractV1,
) -> Result<Vec<u64>, Box<HierarchicalOwnershipFindingV1>> {
    let Some(view_type) = contract.view_op.view_type(context) else {
        return Err(Box::new(
            HierarchicalOwnershipFindingV1::SparseIndexAnalysisIncomplete {
                detail: format!("contracted view {} has no ranked type", contract.view_name),
            },
        ));
    };
    view_type
        .deref(context)
        .shape()
        .iter()
        .copied()
        .enumerate()
        .map(|(dimension, extent)| {
            if extent != DYNAMIC_EXTENT {
                return Ok(extent);
            }
            contract
                .view_op
                .dynamic_extent(context, dimension)
                .and_then(|value| sparse.fact(value).constant_value())
                .ok_or_else(|| {
                    Box::new(HierarchicalOwnershipFindingV1::DynamicExtentIncomplete {
                        view: contract.view_name.clone(),
                        dimension,
                    })
                })
        })
        .collect()
}

fn bounded_element_count(
    view: &str,
    extents: &[u64],
) -> Result<usize, Box<HierarchicalOwnershipFindingV1>> {
    bounded_element_count_with_observation_v1(view, extents, None)
}

fn bounded_element_count_with_observation_v1(
    view: &str,
    extents: &[u64],
    observer: OwnershipObserverV1<'_, '_, '_>,
) -> Result<usize, Box<HierarchicalOwnershipFindingV1>> {
    let actual = extents
        .iter()
        .try_fold(1_u64, |total, extent| total.checked_mul(*extent))
        .unwrap_or(u64::MAX);
    if actual > MAX_HIERARCHICAL_OWNERSHIP_ELEMENTS_V1 as u64 {
        observe_ownership_quota_v1(observer, "ownership element limit");
        return Err(Box::new(
            HierarchicalOwnershipFindingV1::ElementLimitExceeded {
                view: view.to_owned(),
                actual,
                limit: MAX_HIERARCHICAL_OWNERSHIP_ELEMENTS_V1,
            },
        ));
    }
    Ok(actual as usize)
}

#[allow(clippy::too_many_arguments)]
fn analyze_contract(
    context: &Context,
    contract: &ContractV1,
    extents: Option<&[u64]>,
    element_count: Option<usize>,
    traces: &[PlironInvocationTraceV1],
    grid: u64,
    findings: &mut Vec<HierarchicalOwnershipFindingV1>,
    regions: &mut Vec<HierarchicalOwnershipRegionV1>,
) {
    analyze_contract_with_observation_v1(
        context,
        contract,
        extents,
        element_count,
        traces,
        grid,
        (findings, regions, None),
    );
}

fn analyze_contract_with_observation_v1(
    context: &Context,
    contract: &ContractV1,
    extents: Option<&[u64]>,
    element_count: Option<usize>,
    traces: &[PlironInvocationTraceV1],
    grid: u64,
    output: (
        &mut Vec<HierarchicalOwnershipFindingV1>,
        &mut Vec<HierarchicalOwnershipRegionV1>,
        OwnershipObserverV1<'_, '_, '_>,
    ),
) {
    let (findings, regions, observer) = output;
    let mut owners = BTreeMap::<Vec<u64>, HierarchicalOwnerWitnessV1>::new();
    let mut sets = BTreeMap::<HierarchicalRegionIdentityV1, BTreeSet<Vec<u64>>>::new();
    let collective = contract.coverage == OwnershipCoverageAttr::CollectiveContributions;
    for trace in traces {
        if matches!(
            contract.coverage,
            OwnershipCoverageAttr::TotalView | OwnershipCoverageAttr::CollectiveContributions
        ) && let Some(PlironTraceEventV1::Trap { location }) = trace
            .events
            .iter()
            .find(|event| matches!(event, PlironTraceEventV1::Trap { .. }))
        {
            findings.push(HierarchicalOwnershipFindingV1::AbnormalCompletion {
                view: contract.view_name.clone(),
                invocation: HierarchicalInvocationWitnessV1 {
                    invocation: trace.invocation.clone(),
                    workgroup: trace.workgroup,
                    subgroup: trace.subgroup,
                    lane: trace.lane,
                },
                location: (*location).into(),
            });
            return;
        }
        let mut contributions = Vec::<HierarchicalOwnerWitnessV1>::new();
        for event in &trace.events {
            let PlironTraceEventV1::Memory {
                location,
                view,
                access,
                indices,
                ..
            } = event
            else {
                continue;
            };
            if *view != contract.view || !access.writes_memory() {
                continue;
            }
            let witness = HierarchicalOwnerWitnessV1 {
                invocation: trace.invocation.clone(),
                workgroup: trace.workgroup,
                subgroup: trace.subgroup,
                lane: trace.lane,
                location: (*location).into(),
            };
            let Some(coordinate) = indices.iter().copied().collect::<Option<Vec<_>>>() else {
                let dimension = indices
                    .iter()
                    .position(Option::is_none)
                    .expect("failed coordinate contains an unresolved dimension");
                findings.push(HierarchicalOwnershipFindingV1::UnresolvedCoordinate {
                    view: contract.view_name.clone(),
                    location: witness.location,
                    invocation: witness.invocation,
                    dimension,
                });
                return;
            };
            let rank = contract
                .view_op
                .view_type(context)
                .map(|ty| ty.deref(context).shape().len())
                .unwrap_or(0);
            if coordinate.len() != rank
                || extents.is_some_and(|extents| {
                    coordinate
                        .iter()
                        .zip(extents)
                        .any(|(coordinate, extent)| coordinate >= extent)
                })
            {
                findings.push(HierarchicalOwnershipFindingV1::OutOfRange {
                    view: contract.view_name.clone(),
                    coordinate,
                    extents: extents.unwrap_or_default().to_vec(),
                    owner: witness,
                });
                return;
            }
            if collective {
                if !matches!(
                    access,
                    AccessKindAttr::AtomicWrite | AccessKindAttr::AtomicReadModifyWrite
                ) {
                    findings.push(HierarchicalOwnershipFindingV1::NonAtomicContribution {
                        view: contract.view_name.clone(),
                        owner: witness,
                    });
                    return;
                }
                if let Some(first) = contributions.first() {
                    findings.push(HierarchicalOwnershipFindingV1::DuplicateContribution {
                        view: contract.view_name.clone(),
                        invocation: HierarchicalInvocationWitnessV1 {
                            invocation: trace.invocation.clone(),
                            workgroup: trace.workgroup,
                            subgroup: trace.subgroup,
                            lane: trace.lane,
                        },
                        first: first.clone(),
                        second: witness,
                    });
                    return;
                }
                contributions.push(witness.clone());
            } else if let Some(first) = owners.get(&coordinate) {
                if first.invocation != witness.invocation {
                    let class = if first.workgroup != witness.workgroup {
                        HierarchicalOverlapClassV1::AcrossWorkgroups
                    } else if first.subgroup != witness.subgroup {
                        HierarchicalOverlapClassV1::AcrossSubgroups
                    } else {
                        HierarchicalOverlapClassV1::WithinSubgroup
                    };
                    findings.push(HierarchicalOwnershipFindingV1::OverlappingOwners {
                        view: contract.view_name.clone(),
                        coordinate,
                        class,
                        first: first.clone(),
                        second: witness,
                    });
                    return;
                }
                if contract.coverage == OwnershipCoverageAttr::TotalView {
                    findings.push(HierarchicalOwnershipFindingV1::OutputOverwritten {
                        view: contract.view_name.clone(),
                        coordinate,
                        first: first.clone(),
                        overwrite: witness,
                    });
                    return;
                }
            }
            owners.entry(coordinate.clone()).or_insert(witness);
            for identity in [
                HierarchicalRegionIdentityV1::Invocation(trace.invocation.clone()),
                HierarchicalRegionIdentityV1::Subgroup {
                    workgroup: trace.workgroup,
                    subgroup: trace.subgroup,
                },
                HierarchicalRegionIdentityV1::Workgroup(trace.workgroup),
                HierarchicalRegionIdentityV1::Grid(grid),
            ] {
                sets.entry(identity).or_default().insert(coordinate.clone());
            }
        }
        if collective && contributions.is_empty() {
            findings.push(HierarchicalOwnershipFindingV1::MissingContribution {
                view: contract.view_name.clone(),
                invocation: HierarchicalInvocationWitnessV1 {
                    invocation: trace.invocation.clone(),
                    workgroup: trace.workgroup,
                    subgroup: trace.subgroup,
                    lane: trace.lane,
                },
            });
            return;
        }
    }

    if contract.partition == OwnershipPartitionAttr::DenseRectangles {
        for (identity, coordinates) in &sets {
            if matches!(
                identity,
                HierarchicalRegionIdentityV1::Subgroup { .. }
                    | HierarchicalRegionIdentityV1::Workgroup(_)
            ) && let Some(missing) = first_rectangle_hole(coordinates)
            {
                findings.push(HierarchicalOwnershipFindingV1::NonRectangularRegion {
                    view: contract.view_name.clone(),
                    region: identity.clone(),
                    missing,
                });
                return;
            }
        }
    }

    if !collective
        && let (Some(extents), Some(element_count)) = (extents, element_count)
        && owners.len() != element_count
    {
        let image = PresburgerFiniteImageV1::new(
            extents.len(),
            owners.keys().map(|coordinate| {
                coordinate
                    .iter()
                    .map(|coordinate| i128::from(*coordinate))
                    .collect()
            }),
        );
        match image.map(|image| image.find_uncovered(extents)) {
            Ok(PresburgerCoverageDecisionV1::Hole { point }) => {
                let coordinate = point
                    .into_iter()
                    .map(|coordinate| {
                        u64::try_from(coordinate)
                            .expect("zero-based u64 ownership box produced a u64 coordinate")
                    })
                    .collect();
                findings.push(HierarchicalOwnershipFindingV1::CoverageHole {
                    view: contract.view_name.clone(),
                    coordinate,
                    extents: extents.to_vec(),
                });
                return;
            }
            Ok(PresburgerCoverageDecisionV1::Proved) => {}
            Ok(PresburgerCoverageDecisionV1::Incomplete(failure)) | Err(failure) => {
                if matches!(
                    failure,
                    fe2o3_kernel_analysis::PresburgerFailureV1::ResourceLimit { .. }
                ) {
                    observe_ownership_quota_v1(
                        observer,
                        "ownership Presburger coverage work limit",
                    );
                }
                findings.push(HierarchicalOwnershipFindingV1::EffectDomainIncomplete {
                    detail: format!("Presburger coverage query failed: {failure}"),
                });
                return;
            }
        }
    }

    regions.extend(sets.into_iter().map(|(identity, coordinates)| {
        let bounds = coordinate_bounds(&coordinates);
        let dense_rectangle = rectangle_volume(&bounds) == Some(coordinates.len());
        HierarchicalOwnershipRegionV1 {
            view: contract.view_name.clone(),
            coverage: contract.coverage,
            identity,
            element_count: coordinates.len(),
            bounds,
            dense_rectangle,
        }
    }));
}

fn coordinate_bounds(coordinates: &BTreeSet<Vec<u64>>) -> Vec<HierarchicalDimensionRangeV1> {
    let Some(first) = coordinates.first() else {
        return Vec::new();
    };
    let mut bounds = first
        .iter()
        .map(|coordinate| HierarchicalDimensionRangeV1 {
            minimum: *coordinate,
            maximum: *coordinate,
        })
        .collect::<Vec<_>>();
    for coordinate in coordinates.iter().skip(1) {
        for (range, coordinate) in bounds.iter_mut().zip(coordinate) {
            range.minimum = range.minimum.min(*coordinate);
            range.maximum = range.maximum.max(*coordinate);
        }
    }
    bounds
}

fn rectangle_volume(bounds: &[HierarchicalDimensionRangeV1]) -> Option<usize> {
    bounds.iter().try_fold(1_usize, |volume, range| {
        let extent = range.maximum.checked_sub(range.minimum)?.checked_add(1)?;
        volume.checked_mul(usize::try_from(extent).ok()?)
    })
}

fn first_rectangle_hole(coordinates: &BTreeSet<Vec<u64>>) -> Option<Vec<u64>> {
    let bounds = coordinate_bounds(coordinates);
    if rectangle_volume(&bounds) == Some(coordinates.len()) {
        return None;
    }
    first_coordinate_matching(
        &bounds.iter().map(|range| range.minimum).collect::<Vec<_>>(),
        &bounds.iter().map(|range| range.maximum).collect::<Vec<_>>(),
        |coordinate| !coordinates.contains(coordinate),
    )
}

fn first_coordinate_matching(
    minima: &[u64],
    maxima: &[u64],
    mut predicate: impl FnMut(&Vec<u64>) -> bool,
) -> Option<Vec<u64>> {
    if minima.is_empty() || minima.len() != maxima.len() {
        return None;
    }
    let mut coordinate = minima.to_vec();
    loop {
        if predicate(&coordinate) {
            return Some(coordinate);
        }
        let mut dimension = 0;
        loop {
            if dimension == coordinate.len() {
                return None;
            }
            if coordinate[dimension] < maxima[dimension] {
                coordinate[dimension] += 1;
                coordinate[..dimension].copy_from_slice(&minima[..dimension]);
                break;
            }
            dimension += 1;
        }
    }
}

struct BoundedOwnershipDetailWriterV1 {
    detail: String,
    truncated: bool,
}

impl fmt::Write for BoundedOwnershipDetailWriterV1 {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Ok(());
        }
        let payload_limit = MAX_HIERARCHICAL_OWNERSHIP_DIAGNOSTIC_BYTES_V1 - 3;
        let remaining = payload_limit.saturating_sub(self.detail.len());
        if value.len() <= remaining {
            self.detail.push_str(value);
            return Ok(());
        }
        let mut end = remaining;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.detail.push_str(&value[..end]);
        self.truncated = true;
        Ok(())
    }
}

fn bounded_nested_findings_detail_v1<T: fmt::Display>(prefix: &str, findings: &[T]) -> String {
    let mut writer = BoundedOwnershipDetailWriterV1 {
        detail: String::with_capacity(MAX_HIERARCHICAL_OWNERSHIP_DIAGNOSTIC_BYTES_V1),
        truncated: false,
    };
    let _ = writer.write_str(prefix);
    for (index, finding) in findings.iter().enumerate() {
        if index != 0 {
            let _ = writer.write_str("; ");
        }
        let _ = write!(&mut writer, "{finding}");
    }
    if writer.truncated {
        writer.detail.push_str("...");
    }
    writer.detail
}

fn clean() -> HierarchicalOwnershipReportV1 {
    HierarchicalOwnershipReportV1 {
        findings: Vec::new(),
        regions: Vec::new(),
        coverage_summary: HierarchicalCoverageProofSummaryV1::default(),
    }
}

fn one(finding: HierarchicalOwnershipFindingV1) -> HierarchicalOwnershipReportV1 {
    one_with_summary(finding, HierarchicalCoverageProofSummaryV1::default())
}

fn one_with_summary(
    finding: HierarchicalOwnershipFindingV1,
    coverage_summary: HierarchicalCoverageProofSummaryV1,
) -> HierarchicalOwnershipReportV1 {
    HierarchicalOwnershipReportV1 {
        findings: vec![finding],
        regions: Vec::new(),
        coverage_summary,
    }
}

fn declared_coverage_summary(contracts: &[ContractV1]) -> HierarchicalCoverageProofSummaryV1 {
    let mut summary = HierarchicalCoverageProofSummaryV1::default();
    for contract in contracts {
        match contract.coverage {
            OwnershipCoverageAttr::TotalView => summary.total_view_declared += 1,
            OwnershipCoverageAttr::CollectiveContributions => {
                summary.collective_contributions_declared += 1;
            }
            OwnershipCoverageAttr::ExactView | OwnershipCoverageAttr::ExactEffectDomain => {}
        }
    }
    summary
}

#[cfg(test)]
mod observed_ownership_coverage_tests {
    use super::*;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
    use dialect_kernel::{
        IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp, InvocationIndexOp, RankedViewType,
        ReturnOp,
    };
    use fe2o3_kernel_analysis::{MAX_PRESBURGER_WORK_UNITS_V1 as CAP, PresburgerFailureV1};
    use pliron::{builtin::types::FunctionType, dialect::DialectName};

    fn fixture(extents: Vec<u64>) -> (Context, FuncOp) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        dialect_gpu::register_dialect(&mut context).unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "observed_ownership_coverage".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let launch = (extents[0] - 1) / 2;
        let rank = extents.len();
        let layout = ExecutionLayoutOp::new_with_domain(
            &mut context,
            41,
            [launch, 1, 1],
            [1, 1, 1],
            1,
            ExecutionDomainAttr::FullPhysicalWorkgroups,
        );
        let ty = RankedViewType::new(&context, 32, true, extents).unwrap();
        let view = RankedViewOp::new_in_space_with_allocation_contract(
            &mut context,
            ty,
            vec![],
            MemorySpaceAttr::Global,
            17,
            17,
        )
        .unwrap();
        let view_value = view.result(&context);
        let contract = OwnershipContractOp::new(
            &mut context,
            view_value,
            OwnershipCoverageAttr::ExactView,
            OwnershipPartitionAttr::ExactSets,
        )
        .unwrap();
        let lane = InvocationIndexOp::new(&mut context, 0, launch);
        let zero = IndexConstantOp::new(&mut context, 0);
        let offset = IndexConstantOp::new(&mut context, launch);
        let lane_value = lane.result(&context);
        let offset_value = offset.result(&context);
        let shifted = IndexBinaryOp::new(
            &mut context,
            IndexBinaryKindAttr::Add,
            lane_value,
            offset_value,
        );
        for operation in [
            layout.get_operation(),
            view.get_operation(),
            contract.get_operation(),
            lane.get_operation(),
            zero.get_operation(),
            offset.get_operation(),
            shifted.get_operation(),
        ] {
            operation.insert_at_back(entry, &context);
        }
        for index in [lane_value, shifted.result(&context)] {
            let mut indices = vec![zero.result(&context); rank];
            indices[0] = index;
            RankedAccessOp::new(&mut context, AccessKindAttr::Write, view_value, indices)
                .unwrap()
                .get_operation()
                .insert_at_back(entry, &context);
        }
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        (context, function)
    }

    fn run(extents: Vec<u64>) -> (HierarchicalOwnershipReportV1, Receipt<'static>) {
        let (context, function) = fixture(extents);
        let ordinary = {
            let mut manager = PlironAnalysisManagerV1::new(&function);
            run_pliron_hierarchical_ownership_check_with_analyses_v1(
                &context,
                &function,
                &mut manager,
            )
        };
        let mut manager = PlironAnalysisManagerV1::new(&function);
        let mut receipt = Receipt::new(
            Default::default(),
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        let phase = receipt
            .phase(ProductionAnalysisResourcePhaseV1::HierarchicalOwnership, 0)
            .unwrap();
        let observed = run_pliron_hierarchical_ownership_with_observation_v1(
            &context,
            &function,
            &mut manager,
            Some(&phase.observer(&Ok)),
        );
        drop(phase);
        assert_eq!(observed, ordinary);
        assert!(!receipt.snapshot().caught_panic);
        // Denial classification only: no admission or owner transfer is exercised.
        assert_eq!(receipt.snapshot().committed, Default::default());
        (observed, receipt)
    }

    #[test]
    fn actual_coverage_hole_is_not_resource_denial() {
        let (report, receipt) = run(vec![9]);
        assert!(matches!(report.findings(),
            [HierarchicalOwnershipFindingV1::CoverageHole { coordinate, extents, .. }]
            if coordinate == &[8] && extents == &[9]));
        assert_eq!(receipt.complete(), Ok(Default::default()));
    }

    #[test]
    fn actual_coverage_solver_quota_survives_incomplete_conversion() {
        let (report, receipt) = run(vec![131_073, 1, 1, 1, 1, 1, 1, 1]);
        let failure = PresburgerFailureV1::ResourceLimit {
            limit: CAP,
            actual: CAP + 1,
        };
        assert!(matches!(report.findings(),
            [HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail }]
            if detail == &format!("Presburger coverage query failed: {failure}")));
        let error = ProductionAnalysisResourceLimitV1 {
            phase: ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
            resource: "ownership Presburger coverage work limit",
        };
        assert_eq!(receipt.snapshot().first_denial, Some(error));
        assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
    }
}
