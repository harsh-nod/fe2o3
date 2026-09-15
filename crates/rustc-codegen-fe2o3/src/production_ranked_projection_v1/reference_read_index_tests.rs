// Component reconciliation tests only, not source or numerical proof receipts.
use super::*;

struct Fixture {
    kernel: ProductionRankedKernelV1,
    reads: ReferenceReadRosterV1,
    sources: Vec<ProjectedAccessSourceV1>,
    accesses: Vec<Access>,
}

fn fixture(count: u32) -> Fixture {
    let local = |id| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id));
    let mut operations = vec![
        ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(0),
            value: 0,
        },
        ProductionRankedOperationV1::ViewInSpace {
            result: ProductionRankedValueIdV1::new(1),
            element_width: 32,
            writable: false,
            shape: vec![64],
            dynamic_extents: vec![],
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: 1,
            noalias_class: 1,
        },
    ];
    let mut reads = ReferenceReadRosterV1::default();
    reads.reserve(count as usize).unwrap();
    let mut sources = Vec::new();
    for index in 0..count {
        let operation = index + 2;
        operations.push(ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read,
            view: local(1),
            indices: vec![local(0)],
        });
        let source = ProjectedAccessSourceV1 {
            block: 0,
            operation: operation as usize,
            access: AccessKindAttr::Read,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: 0,
                statement: Some(index as usize),
            }),
        };
        reads
            .record(
                &source,
                &ProductionSemanticLoadV2 {
                    block: 0,
                    operation,
                    scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
                    read_mode: fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedVolatile,
                    allocation_origin: 1,
                    view: local(1),
                    indices: vec![local(0)].into_boxed_slice(),
                },
                true,
            )
            .unwrap();
        sources.push(source);
    }
    let kernel = ProductionRankedKernelV1::new(
        "indexed_read_roster",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let accesses = production_access_sources(kernel.blocks(), &sources).unwrap();
    Fixture {
        kernel,
        reads,
        sources,
        accesses,
    }
}

fn reconcile(f: &Fixture, legacy: bool) -> bool {
    if production_access_sources(f.kernel.blocks(), &f.sources)
        .ok()
        .as_ref()
        != Some(&f.accesses)
    {
        return false;
    }
    if legacy {
        f.reads.reads.iter().all(|(source, _)| {
            f.sources
                .iter()
                .filter(|candidate| *candidate == source)
                .count()
                == 1
        }) && f.reads.legacy_validate(&f.kernel, &f.accesses, &[]).is_ok()
    } else {
        f.reads.validate_original_sources(&f.sources).is_ok()
            && f.reads.validate(&f.kernel, &f.accesses, &[]).is_ok()
    }
}

#[test]
fn reference_roster_index_matches_frozen_scan_oracle_on_source_and_load_mutations() {
    for mutation in 0..16 {
        let mut f = fixture(4);
        match mutation {
            0 => {}
            1 => {
                f.accesses.pop();
            }
            2 => f.accesses.push(f.accesses[0]),
            3 => {
                f.sources.pop();
            }
            4 => f.sources.push(f.sources[0]),
            5 => f.reads.reads.push(f.reads.reads[0].clone()),
            6 => f.reads.reads[0].0.semantic_site = None,
            7 => f.reads.reads[0].1.allocation_origin = 99,
            8 => f.reads.reads[0].1.view = ProductionRankedValueV1::Argument(0),
            9 => f.reads.reads[0].1.indices[0] = ProductionRankedValueV1::Argument(0),
            10 => f.reads.reads[0].0.access = AccessKindAttr::Write,
            11 => {
                let a = f.accesses[0];
                f.accesses[0] = Access::new(
                    a.semantic_block(),
                    a.semantic_statement(),
                    1,
                    a.ranked_block(),
                    a.ranked_operation(),
                );
            }
            12 => {
                let mut ignored = f.sources[0];
                ignored.operation = 0;
                ignored.semantic_site = None;
                f.sources.push(ignored);
            }
            13 => {
                // An excluded private row at the same site is NOT an exact
                // original-source duplicate. Preserve the old equality rule.
                let mut ignored = f.sources[0];
                ignored.memory_space = MemorySpaceAttr::Private;
                f.sources.push(ignored);
            }
            14 => {
                f.sources[0].semantic_site.as_mut().unwrap().statement = Some(99);
                f.accesses = production_access_sources(f.kernel.blocks(), &f.sources).unwrap();
            }
            15 => {
                f.sources.reverse();
                f.accesses = production_access_sources(f.kernel.blocks(), &f.sources).unwrap();
            }
            _ => unreachable!(),
        }
        let legacy = reconcile(&f, true);
        let indexed = reconcile(&f, false);
        assert_eq!(indexed, legacy, "mutation {mutation}");
        assert_eq!(
            indexed,
            matches!(mutation, 0 | 12 | 13 | 15),
            "mutation {mutation}"
        );
    }
}

#[test]
fn reference_roster_index_rejects_duplicate_origins_and_missing_or_extra_ranked_sites() {
    for mutation in 0..4 {
        let mut f = fixture(3);
        match mutation {
            0 => f.accesses[1] = Access::new(0, Some(0), 0, 0, 3),
            1 => {
                f.accesses.pop();
            }
            2 => f.accesses.push(Access::new(0, Some(3), 0, 0, 99)),
            3 => f.accesses.push(f.accesses[0]),
            _ => unreachable!(),
        }
        assert!(
            f.reads
                .legacy_validate(&f.kernel, &f.accesses, &[])
                .is_err()
        );
        assert!(f.reads.validate(&f.kernel, &f.accesses, &[]).is_err());
    }
}

#[test]
fn reference_roster_index_scales_with_indexed_operations_not_read_access_product() {
    for count in [256, 512, 1_024] {
        let f = fixture(count);
        f.reads.charge(f.sources.len()).unwrap(); // Existing expected-source allocation.
        f.reads.validate_original_sources(&f.sources).unwrap();
        f.reads.validate(&f.kernel, &f.accesses, &[]).unwrap();
        // Reservation/record=3R, expected sources=R, source index=4R,
        // validation=15R+6 (complete view prescan plus the access-site pass).
        assert_eq!(f.reads.work.get(), 23 * count as usize + 6);
        assert!(f.reads.work.get() < crate::reference_effect_v1::MAX_REFERENCE_STATEMENTS_V1);
        assert!(
            2 * count as usize * count as usize
                > crate::reference_effect_v1::MAX_REFERENCE_STATEMENTS_V1
        );
    }
}

#[test]
fn reference_roster_index_exact_ceiling_and_one_less_reject_without_reset() {
    let f = fixture(8);
    let start = f.reads.work.get();
    f.reads.validate_original_sources(&f.sources).unwrap();
    f.reads.validate(&f.kernel, &f.accesses, &[]).unwrap();
    let cost = f.reads.work.get() - start;
    for insufficient in [false, true] {
        let f = fixture(8);
        let limit = crate::reference_effect_v1::MAX_REFERENCE_STATEMENTS_V1;
        f.reads
            .charge(limit - cost - f.reads.work.get() + usize::from(insufficient))
            .unwrap();
        let outcome = f
            .reads
            .validate_original_sources(&f.sources)
            .and_then(|()| f.reads.validate(&f.kernel, &f.accesses, &[]));
        assert_eq!(outcome.is_err(), insufficient);
        if insufficient {
            assert_eq!(
                outcome,
                Err("reference read roster exceeds the existing reference work limit")
            );
        } else {
            assert_eq!(f.reads.work.get(), limit);
            assert!(f.reads.charge(1).is_err());
        }
    }
}

#[test]
fn reference_roster_index_charges_before_first_entry_allocation() {
    for source_index in [false, true] {
        let f = fixture(1);
        let limit = crate::reference_effect_v1::MAX_REFERENCE_STATEMENTS_V1;
        f.reads.charge(limit - f.reads.work.get()).unwrap();
        let result = if source_index {
            f.reads.validate_original_sources(&f.sources)
        } else {
            f.reads.validate(&f.kernel, &f.accesses, &[])
        };
        assert_eq!(
            result,
            Err("reference read roster exceeds the existing reference work limit")
        );
        assert_eq!(f.reads.work.get(), limit);
    }
}

#[test]
fn reference_roster_index_generated_coverage_is_not_source_read_authority() {
    use fe2o3_lower_mir_kernel::ProductionRankedExecutableEffectOriginV1;
    let generated = |operation| {
        Generated::new(
            0,
            0,
            0,
            operation,
            ProductionRankedExecutableEffectOriginV1::GeneratedFromSemanticTerminator,
            [7; 32],
        )
    };
    for mutation in 0..4 {
        let mut f = fixture(2);
        let sources = match mutation {
            0 => vec![generated(2)],  // Access/generated overlap.
            1 => vec![generated(99)], // Absent ranked occurrence.
            2 => {
                f.accesses.remove(0);
                vec![generated(2)] // Coverage cannot authenticate the retained read.
            }
            3 => {
                f.accesses.remove(0);
                f.reads.reads.remove(0);
                vec![generated(2)] // Generated coverage itself remains supported.
            }
            _ => unreachable!(),
        };
        let old = f
            .reads
            .legacy_validate(&f.kernel, &f.accesses, &sources)
            .is_ok();
        let new = f.reads.validate(&f.kernel, &f.accesses, &sources).is_ok();
        assert_eq!(new, old, "mutation {mutation}");
        assert_eq!(new, mutation == 3, "mutation {mutation}");
    }
}

fn view_order_fixture(
    later_serialized_declaration: bool,
    memory_space: MemorySpaceAttr,
) -> Result<ProductionRankedKernelV1, fe2o3_pliron::ProductionRankedKernelErrorV1> {
    let local = |id| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id));
    let view = ProductionRankedOperationV1::ViewInSpace {
        result: ProductionRankedValueIdV1::new(1),
        element_width: 32,
        writable: true,
        shape: vec![64],
        dynamic_extents: vec![],
        memory_space,
        allocation_origin: 1,
        noalias_class: 1,
    };
    let mut entry = vec![ProductionRankedOperationV1::IndexConstant {
        result: ProductionRankedValueIdV1::new(0),
        value: 0,
    }];
    let mut producer = Vec::new();
    if later_serialized_declaration {
        producer.push(view);
    } else {
        entry.push(view);
    }
    // Actual CFG: entry0 -> producer2 -> access1. Both producer2 and entry0
    // dominate access1, independently of the vector's serialized order.
    ProductionRankedKernelV1::new(
        "reference_view_order",
        0,
        vec![
            ProductionRankedBlockV1::new(entry, ProductionRankedTerminatorV1::Branch { target: 2 }),
            ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: local(1),
                    indices: vec![local(0)],
                }],
                ProductionRankedTerminatorV1::Return,
            ),
            ProductionRankedBlockV1::new(
                producer,
                ProductionRankedTerminatorV1::Branch { target: 1 },
            ),
        ],
    )
}

#[test]
fn reference_roster_later_dominating_view_declaration_constructs_without_reordering() {
    for space in [MemorySpaceAttr::Private, MemorySpaceAttr::Global] {
        let kernel = view_order_fixture(true, space).unwrap();
        assert!(matches!(kernel.blocks()[2].operations()[0],
            ProductionRankedOperationV1::ViewInSpace { result, memory_space, .. }
            if result == ProductionRankedValueIdV1::new(1) && memory_space == space));
        let mut session = fe2o3_pliron::ProductionPlironSessionV1::new(
            fe2o3_pliron::ProductionSessionLimitsV1::default(),
            [dialect_kernel::dialect_registration().unwrap()],
        )
        .unwrap();
        let registered = session
            .register_construction(
                fe2o3_pliron::ProductionConstructionV1::ranked_kernel("later_view", kernel)
                    .unwrap(),
            )
            .unwrap();
        let _constructed = session
            .construct_registered(registered)
            .expect("native CFG proves view dominance");
    }
}

fn view_order_source(space: MemorySpaceAttr) -> ProjectedAccessSourceV1 {
    ProjectedAccessSourceV1 {
        block: 1,
        operation: 0,
        access: AccessKindAttr::Write,
        memory_space: space,
        source: SemanticSourceProvenanceV1::unavailable(),
        semantic_site: Some(ProjectedSemanticAccessSiteV1 {
            block: 1,
            statement: Some(0),
        }),
    }
}

#[test]
fn reference_roster_private_access_keeps_non_numeric_cfg_order_and_exclusion() {
    for later in [false, true] {
        let kernel = view_order_fixture(later, MemorySpaceAttr::Private).unwrap();
        assert_eq!(kernel.blocks().len(), 3);
        assert!(matches!(
            kernel.blocks()[0].terminator(),
            ProductionRankedTerminatorV1::Branch { target: 2 }
        ));
        assert!(matches!(
            kernel.blocks()[2].terminator(),
            ProductionRankedTerminatorV1::Branch { target: 1 }
        ));
        let sources = [view_order_source(MemorySpaceAttr::Private)];
        let accesses = production_access_sources(kernel.blocks(), &sources).unwrap();
        assert!(accesses.is_empty());
        ProjectedReferenceInputV2::new(
            kernel,
            SemanticFunctionIdentityV1::from_sha256([2; 32]),
            vec![],
            ReferenceReadRosterV1::default(),
            &sources,
            accesses,
            vec![],
        )
        .unwrap_or_else(|error| panic!("{error:?}"));
    }
}

#[test]
fn reference_roster_non_numeric_cfg_global_access_still_requires_correspondence() {
    for later in [false, true] {
        let kernel = view_order_fixture(later, MemorySpaceAttr::Global).unwrap();
        let reads = ReferenceReadRosterV1::default();
        assert_eq!(
            reads.validate(&kernel, &[], &[]),
            Err("reference source roster is incomplete")
        );
        let sources = [view_order_source(MemorySpaceAttr::Global)];
        let accesses = production_access_sources(kernel.blocks(), &sources).unwrap();
        assert_eq!(accesses.len(), 1);
        reads.validate(&kernel, &accesses, &[]).unwrap();
    }
}

// Exact frozen scan validator, test-only differential oracle.
impl ReferenceReadRosterV1 {
    fn legacy_validate(
        &self,
        kernel: &ProductionRankedKernelV1,
        accesses: &[Access],
        generated: &[Generated],
    ) -> Result<(), &'static str> {
        // The complete pre-remap roster has one owner and one occurrence key.
        self.charge(
            accesses
                .len()
                .checked_add(generated.len())
                .ok_or("reference source count overflow")?,
        )?;
        let mut sites = BTreeMap::new();
        let mut origins = BTreeSet::new();
        for access in accesses {
            let site = (access.ranked_block(), access.ranked_operation());
            if sites.insert(site, Some(access)).is_some()
                || !origins.insert((
                    access.semantic_block(),
                    access.semantic_statement(),
                    access.semantic_access_ordinal(),
                ))
            {
                return Err("reference source roster contains a duplicate occurrence");
            }
        }
        for source in generated {
            if sites
                .insert((source.ranked_block(), source.ranked_operation()), None)
                .is_some()
            {
                return Err("reference source roster contains a duplicate occurrence");
            }
        }
        let mut views = BTreeMap::new();
        let mut private_views = BTreeSet::new();
        for (block_index, block) in kernel.blocks().iter().enumerate() {
            self.charge(block.operations().len())?;
            for (operation_index, operation) in block.operations().iter().enumerate() {
                match operation {
                    ProductionRankedOperationV1::View {
                        result,
                        allocation_origin,
                        writable,
                        ..
                    }
                    | ProductionRankedOperationV1::ViewInSpace {
                        result,
                        allocation_origin,
                        writable,
                        memory_space: MemorySpaceAttr::Global,
                        ..
                    } => {
                        if views
                            .insert(
                                ProductionRankedValueV1::Local(*result),
                                (*allocation_origin, *writable),
                            )
                            .is_some()
                        {
                            return Err("reference read view is multiply defined");
                        }
                    }
                    ProductionRankedOperationV1::ViewInSpace {
                        result,
                        memory_space: MemorySpaceAttr::Private,
                        ..
                    } => {
                        private_views.insert(ProductionRankedValueV1::Local(*result));
                    }
                    _ => {}
                }
                if reference_observable_access(operation, &private_views) {
                    if sites
                        .remove(&(block_index as u32, operation_index as u32))
                        .is_none()
                    {
                        return Err("reference source roster is incomplete");
                    }
                }
            }
        }
        if !sites.is_empty() {
            return Err("reference source roster contains an absent access");
        }
        let mut seen = BTreeSet::new();
        for (source, load) in &self.reads {
            self.charge(1 + accesses.len() + load.indices.len())?;
            if !seen.insert((load.block, load.operation)) {
                return Err("reference read roster contains a duplicate occurrence");
            }
            let site = source
                .semantic_site
                .ok_or("reference read source site is absent")?;
            let access = accesses
                .iter()
                .find(|access| {
                    (access.ranked_block(), access.ranked_operation())
                        == (load.block, load.operation)
                })
                .ok_or("reference read has no retained access correspondence")?;
            if access.semantic_block() as usize != site.block
                || access.semantic_statement().map(|value| value as usize) != site.statement
                || (source.block, source.operation)
                    != (load.block as usize, load.operation as usize)
                || views.get(&load.view) != Some(&(load.allocation_origin, false))
                || !matches!(kernel.blocks().get(load.block as usize)
                    .and_then(|block| block.operations().get(load.operation as usize)),
                    Some(ProductionRankedOperationV1::Access { kind: AccessKindAttr::Read, view, indices })
                        if *view == load.view && indices.as_slice() == load.indices.as_ref())
            {
                return Err("reference read differs from its retained source or ranked access");
            }
        }
        Ok(())
    }
}
