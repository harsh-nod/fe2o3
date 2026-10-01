type ScalarSite = (usize, kir::BlockId, u32);

#[derive(Clone, Debug, Eq, PartialEq)]
enum ScalarStep {
    Read(usize, u32),
    Marker(u32),
    Parts,
}

#[derive(Default)]
struct LaneTrace {
    steps: Vec<ScalarStep>,
    addresses: usize,
    parts: BTreeMap<usize, Vec<Option<sim::ScalarBitsV1>>>,
}

struct ScalarObserver {
    elements: u16,
    parts: BTreeMap<ScalarSite, Vec<(usize, usize, kir::ValueId)>>,
    addresses: BTreeSet<ScalarSite>,
    lanes: BTreeMap<u64, LaneTrace>,
}

impl ScalarObserver {
    fn new(
        candidate: &ScopedTileScalarCandidateV29,
        expected_lanes: u16,
        expected_elements: u16,
    ) -> Self {
        let mut source_parts = BTreeMap::new();
        let mut part_count = 0;
        let mut load_count = 0;
        for (function, item) in candidate
            .input
            .pending
            .pending_module()
            .functions
            .iter()
            .enumerate()
        {
            let Some(body) = &item.body else { continue };
            for block in &body.blocks {
                for operation in &block.operations {
                    assert!(
                        !matches!(
                            operation.kind,
                            kir::OperationKind::SliceData { .. }
                                | kir::OperationKind::GetElementPointer { .. }
                        ),
                        "fixture gained a non-tile address operation"
                    );
                    match &operation.kind {
                        kir::OperationKind::Execution(
                            kir::ExecutionOperationV15::MaskedTileLoadU32 {
                                lanes, elements, ..
                            },
                        ) => {
                            assert_eq!((*lanes, *elements), (expected_lanes, expected_elements));
                            load_count += 1;
                        }
                        kir::OperationKind::Execution(
                            kir::ExecutionOperationV15::FragmentIntoPartsU32 {
                                lanes,
                                elements,
                                ..
                            },
                        ) => {
                            assert_eq!((*lanes, *elements), (expected_lanes, expected_elements));
                            assert_eq!(operation.results.len(), 2 * usize::from(expected_elements));
                            for (component, result) in operation.results.iter().enumerate() {
                                assert!(
                                    source_parts
                                        .insert((function, result.id), (part_count, component))
                                        .is_none()
                                );
                            }
                            part_count += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        assert_eq!(load_count, 2);
        let expected_results = source_parts.len();
        let mut result_count = 0;
        let mut parts = BTreeMap::new();
        let mut addresses = BTreeSet::new();
        for (function, item) in candidate.output.module().functions.iter().enumerate() {
            let Some(body) = &item.body else { continue };
            for block in &body.blocks {
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    let site = (function, block.id, u32::try_from(ordinal).unwrap());
                    if matches!(
                        operation.kind,
                        kir::OperationKind::SliceData { .. }
                            | kir::OperationKind::GetElementPointer { .. }
                    ) {
                        addresses.insert(site);
                    }
                    let mut observed = Vec::new();
                    for result in &operation.results {
                        if let Some(&(part, component)) = source_parts.get(&(function, result.id)) {
                            observed.push((part, component, result.id));
                            result_count += 1;
                        }
                    }
                    if !observed.is_empty() {
                        assert!(parts.insert(site, observed).is_none());
                    }
                }
            }
        }
        assert_eq!(
            result_count, expected_results,
            "all original Parts results survive"
        );
        Self {
            elements: expected_elements,
            parts,
            addresses,
            lanes: BTreeMap::new(),
        }
    }
}

impl sim::SimulationDebugSinkV1 for ScalarObserver {
    fn record(
        &mut self,
        record: sim::SimulationDebugRecordV1,
    ) -> sim::SimulationDebugSinkControlV1 {
        let result_width = 2 * usize::from(self.elements);
        let global = record.invocation.global[0];
        let site = (
            record.site.function_ordinal,
            record.site.block,
            record.site.operation,
        );
        match record.kind {
            sim::SimulationDebugRecordKindV1::Memory {
                access,
                byte_offset,
                byte_len,
                address_space,
                value,
                ..
            } => {
                let step = match (access, address_space) {
                    (sim::SimulationDebugMemoryAccessV1::Read, kir::AddressSpace::Global) => {
                        let sim::SimulationDebugValueV1::Scalar(value) = value else {
                            panic!("scalar tile read");
                        };
                        assert_eq!(byte_len, 4);
                        assert_eq!(value.ty(), kir::ScalarType::U32);
                        Some(ScalarStep::Read(
                            byte_offset,
                            u32::try_from(value.bits()).unwrap(),
                        ))
                    }
                    (
                        sim::SimulationDebugMemoryAccessV1::WriteCommitted,
                        kir::AddressSpace::Private,
                    ) => {
                        let sim::SimulationDebugValueV1::Scalar(value) = value else {
                            panic!("scalar marker write");
                        };
                        assert_eq!(byte_len, 4);
                        assert_eq!(value.ty(), kir::ScalarType::U32);
                        Some(ScalarStep::Marker(u32::try_from(value.bits()).unwrap()))
                    }
                    _ => None,
                };
                if let Some(step) = step {
                    self.lanes.entry(global).or_default().steps.push(step);
                }
            }
            sim::SimulationDebugRecordKindV1::Checkpoint {
                phase: sim::SimulationDebugCheckpointPhaseV1::AfterOperation,
                stack,
                ..
            } => {
                let lane = self.lanes.entry(global).or_default();
                if self.addresses.contains(&site) {
                    lane.addresses += 1;
                }
                if let Some(parts) = self.parts.get(&site) {
                    let sim::SimulationDebugCollectionV1::Captured(frames) = stack else {
                        panic!("Parts frame capture unavailable");
                    };
                    let frame = frames
                        .iter()
                        .find(|frame| frame.function_ordinal == site.0)
                        .expect("Parts frame");
                    let sim::SimulationDebugCollectionV1::Captured(values) = &frame.values else {
                        panic!("Parts values capture unavailable");
                    };
                    for &(part, component, value_id) in parts {
                        let binding = values
                            .iter()
                            .find(|binding| binding.value == value_id)
                            .expect("preserved Parts value");
                        let sim::SimulationDebugValueV1::Scalar(value) = &binding.observed else {
                            panic!("scalar Parts result");
                        };
                        if !lane.parts.contains_key(&part) {
                            lane.steps.push(ScalarStep::Parts);
                        }
                        let values = lane
                            .parts
                            .entry(part)
                            .or_insert_with(|| vec![None; result_width]);
                        assert!(
                            values[component].replace(*value).is_none(),
                            "Parts result executed twice"
                        );
                    }
                }
            }
            _ => {}
        }
        sim::SimulationDebugSinkControlV1::Continue
    }
}

fn expected_sample(
    order: ScopedTileOrderV29,
    lane: u32,
    lanes: u16,
    elements: u16,
    component: u16,
    base: u64,
    input: &[u32],
) -> (u32, bool, Option<usize>) {
    let offset = match order {
        ScopedTileOrderV29::Blocked => {
            u128::from(lane) * u128::from(elements) + u128::from(component)
        }
        ScopedTileOrderV29::Striped => u128::from(component) * u128::from(lanes) + u128::from(lane),
    };
    let index = u128::from(base) + offset;
    if lane >= u32::from(lanes)
        || offset > u128::from(u64::MAX)
        || index > u128::from(u64::MAX)
        || index >= input.len() as u128
    {
        (0, false, None)
    } else {
        let index = usize::try_from(index).unwrap();
        (input[index], true, Some(index))
    }
}

#[derive(Clone, Copy)]
struct ScalarCase {
    lanes: u16,
    elements: u16,
    base: u64,
    length: usize,
    prefix: usize,
    groups: u64,
    initial_seed: u32,
}

fn assert_source_simulation(
    candidate: &ScopedTileScalarCandidateV29,
    case: SourceCase,
    order: ScopedTileOrderV29,
    budget: &mut ArgumentBudgetV1<'_>,
    input_case: ScalarCase,
) {
    let ScalarCase {
        lanes,
        elements,
        base,
        length,
        prefix,
        groups,
        initial_seed,
    } = input_case;
    let target = sim::SimulationTargetV1::amdgpu_64();
    let limits = sim::SimulationLimitsV1::default();
    let floor = budget.storage();
    let (simulation, receipt) =
        sim::AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
            &candidate.output,
            limits,
            budget,
        )
        .unwrap();
    let paid = receipt.retained_storage();
    budget.reserve_storage(paid).unwrap();
    let input: Vec<u32> = (0..length)
        .map(|index| 11 + 17 * u32::try_from(index).unwrap())
        .collect();
    let mut backing = vec![0xdad0_0001_u32; prefix];
    backing.extend_from_slice(&input);
    backing.extend([0xfafa_0001, 0xfafa_0002]);
    let bytes: Vec<u8> = backing.into_iter().flat_map(u32::to_le_bytes).collect();
    let initialized = vec![true; bytes.len()];
    let backing_id = sim::BufferBackingIdV1(7);
    let request = sim::SimulationRequestV1::new(
        candidate.output.module().kernels[0].id.clone(),
        [u64::from(lanes) * groups, 1, 1],
        [u32::from(lanes), 1, 1],
        vec![
            sim::SimulationArgumentV1::Scalar(sim::ScalarBitsV1::u32(initial_seed)),
            sim::SimulationArgumentV1::BufferView(
                sim::BufferViewArgumentV1::new(
                    backing_id,
                    kir::ScalarType::U32,
                    kir::AccessMode::ReadOnly,
                    4,
                    4 * prefix,
                    length,
                    target,
                )
                .unwrap(),
            ),
        ],
    )
    .with_shared_buffers(vec![sim::SharedBufferV1 {
        id: backing_id,
        buffer: sim::BufferArgumentV1::new(
            kir::ScalarType::U32,
            kir::AccessMode::ReadOnly,
            4,
            bytes,
            initialized,
            target,
        )
        .unwrap(),
    }]);
    let original = request.clone();
    let mut observer = ScalarObserver::new(candidate, lanes, elements);
    simulation
        .simulate_debugged_with_sink(
            &request,
            target,
            limits,
            sim::SimulationDebugCaptureLimitsV1::new(64, 4096, 64, 4096).unwrap(),
            &mut observer,
        )
        .unwrap();
    assert_eq!(request, original);
    assert_eq!(
        observer.lanes.len(),
        usize::try_from(u64::from(lanes) * groups).unwrap()
    );
    for global in 0..u64::from(lanes) * groups {
        let trace = &observer.lanes[&global];
        let lane = u32::try_from(global % u64::from(lanes)).unwrap();
        let samples: Vec<_> = (0..elements)
            .map(|component| expected_sample(order, lane, lanes, elements, component, base, &input))
            .collect();
        let mut expected_steps = Vec::new();
        let mut seed = initial_seed;
        for _ in 0..2 {
            if matches!(case, SourceCase::Slots) {
                expected_steps.push(ScalarStep::Marker(seed));
            }
            for &(value, _, index) in &samples {
                if let Some(index) = index {
                    expected_steps.push(ScalarStep::Read(4 * (prefix + index), value));
                }
            }
            if matches!(case, SourceCase::Shifted) {
                expected_steps.push(ScalarStep::Marker(seed));
            }
            if !matches!(case, SourceCase::Discard) {
                expected_steps.push(ScalarStep::Parts);
                seed += samples[0].0;
            }
        }
        assert_eq!(
            trace.steps, expected_steps,
            "{case:?} {order:?} length={length} global={global}"
        );
        let reads = expected_steps
            .iter()
            .filter(|step| matches!(step, ScalarStep::Read(..)))
            .count();
        assert_eq!(
            trace.addresses,
            2 * reads,
            "inactive paths must form no addresses"
        );
        assert_eq!(
            trace.parts.len(),
            if matches!(case, SourceCase::Discard) {
                0
            } else {
                2
            }
        );
        let expected_parts: Vec<_> = samples
            .iter()
            .map(|sample| Some(sim::ScalarBitsV1::u32(sample.0)))
            .chain(
                samples
                    .iter()
                    .map(|sample| Some(sim::ScalarBitsV1::boolean(sample.1))),
            )
            .collect();
        for values in trace.parts.values() {
            assert_eq!(
                values.as_slice(),
                expected_parts.as_slice(),
                "{case:?} {order:?} length={length} global={global}"
            );
        }
    }
    drop(simulation);
    budget.release_storage(paid).unwrap();
    assert_eq!(budget.storage(), floor);
}

// The existing configured_source_owner still supplies genuine E2 source with
// the requested launch geometry/base. This helper resizes it before readmission.
struct ElementFixtureV29 {
    arrays: [SemanticTypeIdV1; 2],
    elements: u16,
    rewritten_indices: usize,
}

impl ElementFixtureV29 {
    fn place(
        &mut self,
        place: &SemanticPlaceV1,
        locals: &[SemanticLocalDeclV1],
    ) -> SemanticPlaceV1 {
        let mut input_type = locals[place.local().index() as usize].ty();
        let mut projections = Vec::with_capacity(place.projections().len());
        for projection in place.projections() {
            let kind = match projection.kind() {
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                } if self.arrays.contains(&input_type) => {
                    assert_eq!(minimum_length, 2);
                    assert_eq!((offset, from_end), (0, false));
                    self.rewritten_indices += 1;
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length: u64::from(self.elements),
                        from_end,
                    }
                }
                kind => kind,
            };
            projections.push(SemanticProjectionV1::new(kind, projection.result_type()).unwrap());
            input_type = projection.result_type();
        }
        SemanticPlaceV1::new(place.local(), projections, place.ty()).unwrap()
    }

    fn operand(
        &mut self,
        operand: &SemanticOperandV1,
        locals: &[SemanticLocalDeclV1],
    ) -> SemanticOperandV1 {
        match operand {
            SemanticOperandV1::Copy(place) => SemanticOperandV1::Copy(self.place(place, locals)),
            SemanticOperandV1::Move(place) => SemanticOperandV1::Move(self.place(place, locals)),
            SemanticOperandV1::Constant(value) => SemanticOperandV1::Constant(value.clone()),
        }
    }

    fn rvalue(
        &mut self,
        value: &SemanticRvalueV1,
        locals: &[SemanticLocalDeclV1],
    ) -> SemanticRvalueV1 {
        let kind = match value.kind() {
            SemanticRvalueKindV1::Use(value) => {
                SemanticRvalueKindV1::Use(self.operand(value, locals))
            }
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } => SemanticRvalueKindV1::Binary {
                operation: *operation,
                left: self.operand(left, locals),
                right: self.operand(right, locals),
            },
            SemanticRvalueKindV1::Borrow { kind, place } => SemanticRvalueKindV1::Borrow {
                kind: *kind,
                place: self.place(place, locals),
            },
            SemanticRvalueKindV1::AddressOf { mutability, place } => {
                SemanticRvalueKindV1::AddressOf {
                    mutability: *mutability,
                    place: self.place(place, locals),
                }
            }
            SemanticRvalueKindV1::Load(load) => {
                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                    self.place(load.source(), locals),
                    load.volatility(),
                    load.atomic(),
                ))
            }
            other => panic!("element fixture gained unsupported source rvalue: {other:?}"),
        };
        SemanticRvalueV1::new(value.result_type(), kind)
    }

    fn statement(
        &mut self,
        statement: &SemanticStatementV1,
        locals: &[SemanticLocalDeclV1],
    ) -> SemanticStatementV1 {
        let kind = match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    self.place(assignment.destination(), locals),
                    self.rvalue(assignment.value(), locals),
                ))
            }
            SemanticStatementKindV1::Store(store) => {
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    self.place(store.destination(), locals),
                    self.operand(store.value(), locals),
                    store.volatility(),
                    store.atomic(),
                ))
            }
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => statement.kind().clone(),
            other => panic!("element fixture gained unsupported source statement: {other:?}"),
        };
        SemanticStatementV1::new(statement.source(), kind)
    }

    fn terminator(
        &mut self,
        terminator: &SemanticTerminatorV1,
        locals: &[SemanticLocalDeclV1],
    ) -> SemanticTerminatorV1 {
        let kind = match terminator.kind() {
            SemanticTerminatorKindV1::Call(call) => {
                let arguments = call
                    .arguments()
                    .iter()
                    .map(|operand| self.operand(operand, locals))
                    .collect();
                let destination = call.destination().map(|destination| {
                    SemanticCallDestinationV1::new(
                        self.place(destination.place(), locals),
                        destination.edge(),
                    )
                });
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        call.callee(),
                        arguments,
                        destination,
                        call.unwind(),
                    )
                    .unwrap(),
                )
            }
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } => SemanticTerminatorKindV1::SwitchInt {
                discriminant: self.operand(discriminant, locals),
                targets: targets.clone(),
            },
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::Unreachable => terminator.kind().clone(),
            other => panic!("element fixture gained unsupported source terminator: {other:?}"),
        };
        SemanticTerminatorV1::new(terminator.source(), kind)
    }
}

fn element_type_v29(
    original: &SemanticTypeDeclV1,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
    kind: SemanticRustTypeKindV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        original.identity(),
        original.layout_identity(),
        layout,
        shape,
    )
    .with_rustc_abi_properties(original.abi_properties())
    .with_rust_type_kind(kind)
}

fn element_aggregate_layout_v29(elements: u16, nominal: bool) -> SemanticTypeLayoutV1 {
    let width = u64::from(elements);
    let used = 5 * width;
    let size = (used + 3) & !3;
    let offsets = if nominal {
        vec![0, 4 * width, used, used]
    } else {
        vec![0, 4 * width]
    };
    let padding = if used == size {
        vec![]
    } else {
        vec![SemanticPaddingV1::new(used, size - used).unwrap()]
    };
    SemanticTypeLayoutV1::aggregate_with_backend_repr(
        Some(size),
        4,
        SemanticBackendReprV1::memory(true),
        false,
        SemanticAggregateLayoutV1::new(offsets, padding).unwrap(),
    )
    .unwrap()
}

fn element_abi_v29(
    original: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    changed: &[SemanticTypeIdV1],
) -> SemanticFunctionAbiV1 {
    let rustic = matches!(
        original.canon_abi(),
        SemanticCanonAbiV1::Rust
            | SemanticCanonAbiV1::RustCold
            | SemanticCanonAbiV1::RustPreserveNone
    );
    let value = |original: &SemanticAbiValueV1| {
        if changed.contains(&original.source_ty()) {
            let ty = &types[original.source_ty().index() as usize];
            if rustic
                && matches!(
                    ty.layout().backend_repr(),
                    SemanticBackendReprV1::Memory { sized: true }
                )
                && matches!(ty.layout().size_bytes(), Some(1..=8))
            {
                // Exact Rust ABI uses an integer Cast for small memory aggregates.
                let size = ty.layout().size_bytes().unwrap();
                let register =
                    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, size).unwrap();
                let attributes = SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        false,
                        None,
                        false,
                        false,
                        false,
                        ty.abi_properties().rustc_layout_is_noundef(),
                    ),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap();
                SemanticAbiValueV1::new(
                    original.source_ty(),
                    SemanticAbiPassModeV1::cast(
                        false,
                        SemanticAbiCastV1::new(
                            [None; 8],
                            None,
                            SemanticAbiUniformV1::new(register, size).unwrap(),
                            attributes,
                        ),
                    ),
                )
            } else {
                value_abi(types, original.source_ty())
            }
        } else {
            original.clone()
        }
    };
    let arguments = original
        .arguments()
        .iter()
        .map(|argument| {
            let value = value(argument.value());
            match argument.role() {
                SemanticAbiArgumentRoleV1::Source => SemanticAbiArgumentV1::source(value),
                SemanticAbiArgumentRoleV1::Hidden(role) => {
                    SemanticAbiArgumentV1::hidden(role, value)
                }
                SemanticAbiArgumentRoleV1::RustCallTupleField(field) => {
                    SemanticAbiArgumentV1::rust_call_tuple_field(field, value)
                }
            }
        })
        .collect();
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        original.identity(),
        original.layout_identity(),
        original.canon_abi(),
        original.extern_abi(),
        original.can_unwind(),
        original.c_variadic(),
        original.fixed_count(),
        original.source_input_types().to_vec(),
        original.source_output_type(),
        arguments,
        value(original.return_value()),
    )
    .unwrap()
    .with_source_argument_ownership(original.source_argument_ownership().to_vec())
    .unwrap()
}

fn elements_source_owner(
    case: SourceCase,
    lanes: u16,
    elements: u16,
    base: u64,
) -> ProductionSemanticSsaOwnerV1 {
    assert!(matches!(elements, 1 | 2 | 3));
    let template = configured_source_owner(case, lanes, base);
    if elements == 2 {
        return template;
    }
    let semantic = template.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut tiles = types.iter().enumerate().filter_map(|(index, ty)| {
        matches!(
            ty.rust_type_kind(),
            SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::MaskedTileU32 { .. })
        )
        .then_some(SemanticTypeIdV1::from_index(u32::try_from(index).unwrap()))
    });
    let tile = tiles.next().expect("tile source type");
    assert!(tiles.next().is_none());
    let mut fragments = types.iter().enumerate().filter_map(|(index, ty)| {
        matches!(
            ty.rust_type_kind(),
            SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::LaneFragmentU32 { .. })
        )
        .then_some(SemanticTypeIdV1::from_index(u32::try_from(index).unwrap()))
    });
    let fragment = fragments.next().expect("fragment source type");
    assert!(fragments.next().is_none());
    let SemanticTypeShapeV1::Aggregate(fields) = types[tile.index() as usize].shape() else {
        panic!("nominal tile aggregate");
    };
    assert_eq!(fields.fields().len(), 4);
    let arrays = [fields.fields()[0], fields.fields()[1]];
    let mut parts_types = types.iter().enumerate().filter_map(|(index, ty)| {
        matches!(ty.shape(), SemanticTypeShapeV1::Tuple(fields)
            if fields.fields() == arrays.as_slice())
        .then_some(SemanticTypeIdV1::from_index(u32::try_from(index).unwrap()))
    });
    let parts = parts_types.next().expect("Parts source tuple");
    assert!(parts_types.next().is_none());
    let changed = [arrays[0], arrays[1], tile, fragment, parts];
    for array in arrays {
        let old = &types[array.index() as usize];
        let SemanticTypeShapeV1::Array { element, length } = old.shape() else {
            panic!("tile values and masks are source arrays");
        };
        assert_eq!(*length, 2);
        let element = *element;
        let layout = types[element.index() as usize].layout();
        let stride = layout.size_bytes().unwrap();
        let bytes = stride * u64::from(elements);
        let layout = SemanticTypeLayoutV1::with_exact_rustc_layout(
            bytes,
            layout.alignment_bytes(),
            SemanticFieldsShapeV1::array(stride, u64::from(elements)),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            layout.alignment_bytes(),
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap();
        types[array.index() as usize] = element_type_v29(
            old,
            layout,
            SemanticTypeShapeV1::Array {
                element,
                length: u64::from(elements),
            },
            old.rust_type_kind(),
        );
    }
    for (id, role) in [
        (
            tile,
            SemanticExecutionRoleV29::MaskedTileU32 { lanes, elements },
        ),
        (
            fragment,
            SemanticExecutionRoleV29::LaneFragmentU32 { lanes, elements },
        ),
    ] {
        let old = &types[id.index() as usize];
        types[id.index() as usize] = element_type_v29(
            old,
            element_aggregate_layout_v29(elements, true),
            old.shape().clone(),
            SemanticRustTypeKindV1::Execution(role),
        );
    }
    let old = &types[parts.index() as usize];
    types[parts.index() as usize] = element_type_v29(
        old,
        element_aggregate_layout_v29(elements, false),
        old.shape().clone(),
        old.rust_type_kind(),
    );

    let mut rewrite = ElementFixtureV29 {
        arrays,
        elements,
        rewritten_indices: 0,
    };
    let mut functions = Vec::with_capacity(semantic.functions().len());
    for function in semantic.functions() {
        let blocks = function
            .blocks()
            .iter()
            .map(|block| {
                let statements = block
                    .statements()
                    .iter()
                    .map(|statement| rewrite.statement(statement, function.locals()))
                    .collect();
                SemanticBasicBlockV1::new(
                    block.identity(),
                    block.source(),
                    statements,
                    rewrite.terminator(block.terminator(), function.locals()),
                )
                .unwrap()
            })
            .collect();
        let mut replacement = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            element_abi_v29(function.abi(), &types, &changed),
            function.locals().to_vec(),
            function.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = function.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        functions.push(replacement);
    }
    assert_eq!(
        rewrite.rewritten_indices == 0,
        matches!(case, SourceCase::Discard)
    );
    let mut callables = semantic.callables().to_vec();
    for callable in &mut callables {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation,
            operation_identity,
        } = callable
        else {
            continue;
        };
        let tag = match operation {
            SemanticCompilerIntrinsicOperationV1::Execution(
                SemanticExecutionOperationV29::MaskedTileLoadU32 { .. },
            ) => 160_u8,
            SemanticCompilerIntrinsicOperationV1::Execution(
                SemanticExecutionOperationV29::MaskedTileIntoFragmentU32 { .. },
            ) => 161_u8,
            SemanticCompilerIntrinsicOperationV1::Execution(
                SemanticExecutionOperationV29::LaneFragmentIntoPartsU32 { .. },
            ) => 162_u8,
            _ => continue,
        };
        assert_eq!(
            binding.identity(),
            SemanticFunctionIdentityV1::from_sha256([tag; 32])
        );
        *callable = SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                binding.identity(),
                SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                source(),
                element_abi_v29(binding.abi(), &types, &changed),
            ),
            operation: operation.clone(),
            operation_identity: *operation_identity,
        };
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn configured_scalar_candidate_elements(
    case: SourceCase,
    order: ScopedTileOrderV29,
    lanes: u16,
    elements: u16,
    base: u64,
    budget: &mut ArgumentBudgetV1<'_>,
) -> ScopedTileScalarCandidateV29 {
    scalar_candidate_from_source_v29(
        || elements_source_owner(case, lanes, elements, base),
        order,
        lanes,
        budget,
    )
}

#[test]
fn genuine_e1_e3_sources_match_every_value_mask_and_ordered_memory_event() {
    for case in [
        SourceCase::Repeated,
        SourceCase::Slots,
        SourceCase::Shifted,
        SourceCase::Discard,
        SourceCase::BranchParts,
    ] {
        for order in [ScopedTileOrderV29::Blocked, ScopedTileOrderV29::Striped] {
            for elements in [1_u16, 3] {
                let lanes = 3_u16;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
                budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
                let candidate = configured_scalar_candidate_elements(
                    case,
                    order,
                    lanes,
                    elements,
                    0,
                    &mut budget,
                );
                candidate.replay_with_budget(&mut budget).unwrap();
                let extent = usize::from(lanes) * usize::from(elements);
                for length in [0, 1, extent - 1, extent, extent + 3] {
                    assert_source_simulation(
                        &candidate,
                        case,
                        order,
                        &mut budget,
                        ScalarCase {
                            lanes,
                            elements,
                            base: 0,
                            length,
                            prefix: 3,
                            groups: 2,
                            initial_seed: 0,
                        },
                    );
                }
                drop_scalar_candidate(candidate, &mut budget);
                assert_eq!(budget.storage(), SCHEDULE_FLOOR);
                budget.release_storage(SCHEDULE_FLOOR).unwrap();
            }
        }
    }
}

#[test]
fn genuine_e1_e3_source_bases_preserve_finite_width_masks_and_snapshots() {
    for order in [ScopedTileOrderV29::Blocked, ScopedTileOrderV29::Striped] {
        for elements in [1_u16, 3] {
            for base in [1, u64::MAX - 1, u64::MAX] {
                let lanes = 3_u16;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
                budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
                let case = SourceCase::Shifted;
                let candidate = configured_scalar_candidate_elements(
                    case,
                    order,
                    lanes,
                    elements,
                    base,
                    &mut budget,
                );
                candidate.replay_with_budget(&mut budget).unwrap();
                assert_source_simulation(
                    &candidate,
                    case,
                    order,
                    &mut budget,
                    ScalarCase {
                        lanes,
                        elements,
                        base,
                        length: 11,
                        prefix: 3,
                        groups: 2,
                        initial_seed: 2,
                    },
                );
                drop_scalar_candidate(candidate, &mut budget);
                assert_eq!(budget.storage(), SCHEDULE_FLOOR);
                budget.release_storage(SCHEDULE_FLOOR).unwrap();
            }
        }
    }
}

const METADATA_HELPER_V29: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(4);

fn return_conversion_source_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let template = source_owner(SourceCase::Slots);
    let semantic = template.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    assert_eq!(functions.len(), 4);
    callables.insert(4, SemanticCallableDeclV1::defined(METADATA_HELPER_V29));
    // Defined callables remain the prefix. Remap old intrinsics before adding
    // the new call4 so its identity cannot be accidentally shifted.
    for prior in &mut functions {
        let blocks = prior
            .blocks()
            .iter()
            .map(|source_block| {
                let SemanticTerminatorKindV1::Call(call) = source_block.terminator().kind() else {
                    return source_block.clone();
                };
                if call.callee().index() < 4 {
                    return source_block.clone();
                }
                SemanticBasicBlockV1::new(
                    source_block.identity(),
                    source_block.source(),
                    source_block.statements().to_vec(),
                    SemanticTerminatorV1::new(
                        source_block.terminator().source(),
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable(
                                SemanticCallableIdV1::from_index(call.callee().index() + 1),
                                call.arguments().to_vec(),
                                call.destination().cloned(),
                                call.unwind(),
                            )
                            .unwrap(),
                        ),
                    ),
                )
                .unwrap()
            })
            .collect();
        *prior =
            replace_extra_function_v29(prior, prior.abi().clone(), prior.locals().to_vec(), blocks);
    }
    let workgroup = functions[2].abi().source_input_types()[0];
    let SemanticTypeShapeV1::Aggregate(fields) =
        semantic.types()[workgroup.index() as usize].shape()
    else {
        panic!("Workgroup source fields");
    };
    let index_ty = fields.fields()[0];
    let helper = &functions[3];
    let slice_ty = helper.locals()[3].ty();
    let slice_argument = helper.abi().arguments()[2].clone();
    let mut locals = helper.locals().to_vec();
    let destination = locals.len() as u32;
    locals.push(local(230, index_ty, SemanticLocalRoleV1::Temporary));
    let mut blocks = helper.blocks().to_vec();
    let continuation = blocks.len() as u32;
    let original = blocks[0].clone();
    assert!(matches!(
        original.terminator().kind(),
        SemanticTerminatorKindV1::Call(_)
    ));
    blocks.push(block(230, vec![], original.terminator().kind().clone()));
    blocks[0] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        original.statements().to_vec(),
        SemanticTerminatorV1::new(
            original.terminator().source(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(4),
                    vec![SemanticOperandV1::Copy(place(3, slice_ty))],
                    Some(SemanticCallDestinationV1::new(
                        place(destination, index_ty),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(continuation),
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap();
    functions[3] = replace_extra_function_v29(helper, helper.abi().clone(), locals, blocks);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([231; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![slice_argument],
        direct(index_ty),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap();
    functions.push(function(
        231,
        SemanticFunctionRoleV1::InternalHelper,
        abi,
        vec![
            local(231, index_ty, SemanticLocalRoleV1::Return),
            local(232, slice_ty, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(
            231,
            vec![assign(
                place(0, index_ty),
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: SemanticOperandV1::Copy(place(1, slice_ty)),
                },
            )],
            SemanticTerminatorKindV1::Return,
        )],
    ));
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}
