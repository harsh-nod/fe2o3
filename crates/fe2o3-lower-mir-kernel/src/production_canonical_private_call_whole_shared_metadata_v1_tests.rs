pub(super) struct Metadata {
    associations: Vec<(u32, u32)>,
    blocks: Vec<(u32, u32)>,
    spans: Vec<(usize, (u32, u32), std::ops::Range<usize>)>,
    groups: Vec<std::ops::Range<usize>>,
    operations: usize,
}

impl Metadata {
    pub(super) fn root(spans: &[std::ops::Range<usize>], operations: usize) -> Self {
        Self {
            associations: vec![(0, 0)],
            blocks: vec![(0, 0)],
            spans: spans.iter().map(|span| (0, (0, 0), span.clone())).collect(),
            groups: vec![0..operations],
            operations,
        }
    }

    pub(super) fn source(owner: &ProductionPreRankedKirOwnerV1) -> Self {
        let module = owner.executable().module();
        let rows = &owner.correspondence;
        let mut blocks = Vec::new();
        let mut block_operations = Vec::new();
        let mut functions = Vec::new();
        let mut operations = 0;
        for (index, function) in module.functions.iter().enumerate() {
            let start = operations;
            for block in &function.body.as_ref().unwrap().blocks {
                blocks.push((index as u32, block.id.0));
                block_operations.push(operations..operations + block.operations.len());
                operations += block.operations.len();
            }
            functions.push(start..operations);
        }
        let associations: Vec<_> = rows
            .lowered_functions
            .iter()
            .map(|row| {
                (
                    row.correspondence_owner.index(),
                    row.semantic_function.index(),
                )
            })
            .collect();
        let physical: Vec<_> = rows
            .lowered_functions
            .iter()
            .map(|row| {
                module
                    .functions
                    .iter()
                    .position(|f| f.id == row.kernel_ir_function)
                    .unwrap()
            })
            .collect();
        let groups = physical
            .iter()
            .map(|&index| functions[index].clone())
            .collect();
        let raw = rows
            .statement_operation_spans
            .iter()
            .map(|s| {
                (
                    s.correspondence_owner.index(),
                    s.semantic_function.index(),
                    s.kernel_ir_block.0,
                    s.first_operation_ordinal as usize,
                    s.operation_count as usize,
                )
            })
            .chain(rows.terminator_operation_spans.iter().map(|s| {
                (
                    s.correspondence_owner.index(),
                    s.semantic_function.index(),
                    s.kernel_ir_block.0,
                    s.first_operation_ordinal as usize,
                    s.operation_count as usize,
                )
            }))
            .chain(rows.synthetic_operation_spans.iter().map(|s| {
                (
                    s.correspondence_owner.index(),
                    s.semantic_function.index(),
                    s.kernel_ir_block.0,
                    s.first_operation_ordinal as usize,
                    s.operation_count as usize,
                )
            }));
        let spans = raw
            .map(|(root, function, block, first, count)| {
                let association = associations
                    .iter()
                    .position(|key| *key == (root, function))
                    .unwrap();
                let block = (physical[association] as u32, block);
                let range = &block_operations[blocks.iter().position(|key| *key == block).unwrap()];
                let start = range.start + first;
                assert!(start + count <= range.end);
                (association, block, start..start + count)
            })
            .collect();
        blocks.sort_unstable();
        Self {
            associations,
            blocks,
            spans,
            groups,
            operations,
        }
    }

    pub(super) fn origins(&self) -> Vec<(usize, usize)> {
        let mut origins: Vec<_> = self
            .spans
            .iter()
            .flat_map(|(association, _, range)| {
                range.clone().map(|operation| (operation, *association))
            })
            .collect();
        origins.sort_unstable();
        assert!(origins.windows(2).all(|pair| pair[0] < pair[1]));
        for operation in 0..self.operations {
            assert!(origins.iter().any(|row| row.0 == operation));
        }
        origins
    }

    pub(super) fn build(&self, trace: &mut Trace) -> usize {
        units(trace, self.spans.len());
        let retained = sum(&[
            vector::<((u32, u32), usize)>(self.associations.len(), trace),
            vector::<ProductionCanonicalRankedSpanV1<'_>>(self.spans.len(), trace),
            vector::<CrOriginV1>(self.origins().len(), trace),
            vector::<std::ops::Range<usize>>(self.operations, trace),
        ]);
        units(trace, self.associations.len());
        let mut keys = self.associations.clone();
        heap_schedule(&mut keys, trace, false, false, |_, _| 1);
        units(trace, keys.len().saturating_sub(1));
        let mut origins = Vec::new();
        for (association, block, range) in &self.spans {
            find(&keys, &self.associations[*association], trace, |_, _| 2);
            find(&self.blocks, block, trace, |_, _| 1);
            trace.work(3);
            for operation in range.clone() {
                trace.work(1);
                origins.push((operation, *association));
            }
            trace.work(1);
        }
        heap_schedule(&mut origins, trace, false, false, |_, _| 1);
        for operation in 0..self.operations {
            for _ in origins.iter().filter(|row| row.0 == operation) {
                trace.work(2);
            }
            trace.work(1);
        }
        retained
    }

    pub(super) fn check(&self, trace: &mut Trace) {
        let mut keys = self.associations.clone();
        keys.sort_unstable();
        trace.work(4);
        for key in &self.associations {
            find(&keys, key, trace, |_, _| 2);
        }
        units(trace, keys.len().saturating_sub(1));
        for (association, block, _) in &self.spans {
            trace.work(5);
            find(&keys, &self.associations[*association], trace, |_, _| 2);
            find(&self.blocks, block, trace, |_, _| 1);
            trace.work(3);
        }
        let origins = self.origins();
        for operation in 0..self.operations {
            trace.work(3);
            for _ in origins.iter().filter(|row| row.0 == operation) {
                trace.work(6);
            }
        }
        for (association, operations) in self.groups.iter().enumerate() {
            for operation in operations.clone() {
                let aliases: Vec<_> = origins
                    .iter()
                    .filter(|row| row.0 == operation)
                    .map(|row| row.1)
                    .collect();
                find(&aliases, &association, trace, |_, _| 1);
            }
        }
    }
}

pub(super) struct Contracts {
    pub(super) spans: usize,
    pub(super) terminators: usize,
    pub(super) operations: usize,
    pub(super) callables: usize,
    pub(super) associations: Vec<(u32, u32)>,
    pub(super) launch_names: Vec<usize>,
}

impl Contracts {
    pub(super) fn source(owner: &ProductionPreRankedKirOwnerV1) -> Self {
        let semantic = owner.semantic_ssa().source_semantic();
        assert_eq!(owner.assert_origins().source_site_count(), 0);
        assert!(semantic.callables().iter().all(|c| matches!(
            c,
            fe2o3_mir_model::semantic_mir_v1::SemanticCallableDeclV1::Defined { .. }
        )));
        let rows = &owner.correspondence;
        let launch_names = owner
            .executable()
            .module()
            .kernels
            .iter()
            .zip(semantic.roots())
            .map(|(kernel, root)| {
                kernel.id.as_str().len()
                    + semantic.functions()[root.index() as usize]
                        .kernel_entry()
                        .unwrap()
                        .export_symbol()
                        .as_bytes()
                        .len()
            })
            .collect();
        Self {
            spans: sum(&[
                rows.statement_operation_spans.len(),
                rows.terminator_operation_spans.len(),
                rows.synthetic_operation_spans.len(),
            ]),
            terminators: rows.terminator_operation_spans.len(),
            operations: Graph::module(owner.executable().module()).operations,
            callables: semantic.callables().len(),
            associations: rows
                .lowered_functions
                .iter()
                .map(|row| {
                    (
                        row.correspondence_owner.index(),
                        row.semantic_function.index(),
                    )
                })
                .collect(),
            launch_names,
        }
    }

    pub(super) fn build(&self, trace: &mut Trace) -> usize {
        use fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1 as Catalog;
        let launches =
            vector::<ProductionCanonicalRankedLaunchV1<'_>>(self.launch_names.len(), trace);
        units(trace, self.launch_names.len());
        vector::<ProductionCanonicalRankedAssertionV1>(0, trace);
        units(trace, self.spans);
        let source = trace.enter(&[]);
        let headers = product(4, size_of::<Vec<()>>());
        trace.reserve(headers);
        units(trace, self.callables);
        trace.reserve(0);
        units(trace, product(2, self.callables));
        trace.reserve(0);
        units(trace, self.callables);
        let functions = exact_capacity::<SourceCatalogFunctionV1<'_>>(self.associations.len());
        trace.reserve(functions);
        units(trace, self.associations.len());
        let mut keys = self.associations.clone();
        heap_schedule(&mut keys, trace, false, false, |_, _| 1);
        units(trace, keys.len().saturating_sub(1));
        units(trace, self.terminators);
        trace.reserve(0);
        units(trace, self.terminators);
        let codec = trace.enter(&[]);
        trace.work(1);
        let catalog = sum(&[size_of::<Catalog>(), exact_capacity::<u8>(56)]);
        trace.reserve(catalog);
        trace.work(sum(&[
            2 * 56,
            b"FE2O3/NATIVE-KIR-IMPORT-CONTRACT-CATALOG/V1\0".len(),
            8,
        ]));
        trace.work(0);
        trace.leave(codec);
        trace.reserve(catalog);
        trace.release(headers + functions);
        trace.leave(source);
        trace.reserve(catalog);
        launches + catalog
    }

    pub(super) fn check(&self, trace: &mut Trace) {
        trace.work(4);
        for &name in &self.launch_names {
            trace.work(160);
            trace.work(name);
        }
        units(trace, self.spans);
        let source = trace.enter(&[]);
        trace.reserve(size_of::<Vec<(bool, bool)>>() + size_of::<Vec<bool>>());
        vector::<(bool, bool)>(0, trace);
        trace.work(0);
        vector::<bool>(0, trace);
        trace.work(0);
        units(trace, self.callables);
        units(trace, self.terminators);
        trace.work(0);
        trace.leave(source);
        let scope = trace.enter(&[]);
        units(trace, self.operations);
        let header = size_of::<fe2o3_kernel_analysis::CheckedKernelIrContractCatalogV1<'_, '_>>();
        trace.reserve(header);
        trace.leave(scope);
        trace.reserve(header);
        trace.release(header);
    }
}
