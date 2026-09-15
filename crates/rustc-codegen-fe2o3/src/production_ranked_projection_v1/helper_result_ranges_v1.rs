//! Range facts from the exact replayed execution body, including its retained
//! parameter/return moves. No callable name, launch assumption or assertion is
//! authority here. Only the acyclic prefix is evaluated; a backedge and every
//! block depending on it receive no facts. Every predecessor participates.

use super::*;

#[path = "helper_result_ranges_v1/primitive_arrays_v1.rs"]
mod primitive_arrays_v1;

#[path = "helper_result_ranges_v1/retained_facts_v1.rs"]
mod retained_facts_v1;

#[path = "helper_result_ranges_v1/working_scope_v1.rs"]
mod working_scope_v1;

const MAX_CELLS: usize = 262_144;
const MAX_FIELDS: usize = 8;
const MAX_DEPTH: usize = 4;
const MAX_VALUE_NODES: usize = 64;
// Operand snapshots, one aggregate/result and one joining value can coexist.
// State reservations below remain live while this bounded scratch is in use.
const SCRATCH_CELLS: usize = MAX_VALUE_NODES * (2 * MAX_FIELDS + 8);
const STORAGE_LIMIT: &str = "helper-result range working storage limit";
type State = BTreeMap<usize, Value>;
type Stamp = (usize, usize);

struct WorkingCells {
    live: usize,
    limit: usize,
    #[cfg(test)]
    peak: usize,
}

impl WorkingCells {
    fn new(limit: usize) -> Self {
        Self {
            live: 0,
            limit,
            #[cfg(test)]
            peak: 0,
        }
    }

    fn reserve(&mut self, amount: usize) -> Result<(), ProductionRankedProjectionErrorV1> {
        let next = self
            .live
            .checked_add(amount)
            .filter(|&next| next <= self.limit)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                STORAGE_LIMIT,
            ))?;
        self.live = next;
        #[cfg(test)]
        {
            self.peak = self.peak.max(next);
        }
        Ok(())
    }

    fn release(&mut self, amount: usize) {
        self.live = self
            .live
            .checked_sub(amount)
            .expect("reserved helper-range cells");
    }

    fn replace(
        &mut self,
        previous: usize,
        next: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if next > previous {
            self.reserve(next - previous)?;
        } else {
            self.release(previous - next);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Scalar {
    lo: u128,
    hi: u128,
    stamp: Option<Stamp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Test {
    Compare(SemanticBinaryOpV1, Scalar, Scalar),
    Discriminant(Stamp, Vec<u128>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Unknown,
    Scalar(Scalar, Option<Test>),
    Fields(Vec<Value>),
    Enum {
        stamp: Option<Stamp>,
        variants: BTreeMap<u32, Vec<Value>>,
    },
}

impl Value {
    fn scalar(&self) -> Option<&Scalar> {
        match self {
            Self::Scalar(value, _) => Some(value),
            _ => None,
        }
    }

    fn exact(value: u128) -> Self {
        Self::Scalar(
            Scalar {
                lo: value,
                hi: value,
                stamp: None,
            },
            None,
        )
    }

    fn join(&mut self, other: &Self) {
        match (&mut *self, other) {
            (Self::Scalar(a, test), Self::Scalar(b, other_test)) => {
                a.lo = a.lo.min(b.lo);
                a.hi = a.hi.max(b.hi);
                if a.stamp != b.stamp {
                    a.stamp = None;
                }
                if test != other_test {
                    *test = None;
                }
            }
            (Self::Fields(a), Self::Fields(b)) if a.len() == b.len() => {
                for (a, b) in a.iter_mut().zip(b) {
                    a.join(b);
                }
            }
            (
                Self::Enum { stamp, variants },
                Self::Enum {
                    stamp: other_stamp,
                    variants: other,
                },
            ) => {
                if stamp != other_stamp {
                    *stamp = None;
                }
                for (variant, fields) in other {
                    match variants.get_mut(variant) {
                        Some(current) if current.len() == fields.len() => {
                            for (a, b) in current.iter_mut().zip(fields) {
                                a.join(b);
                            }
                        }
                        Some(_) => {
                            *self = Self::Unknown;
                            return;
                        }
                        None => {
                            variants.insert(*variant, fields.clone());
                        }
                    }
                }
                if variants.len() > MAX_FIELDS {
                    *self = Self::Unknown;
                }
            }
            _ => *self = Self::Unknown,
        }
        if nodes(self) > MAX_VALUE_NODES {
            *self = Self::Unknown;
        }
    }
}

pub(super) struct HelperResultRangesV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    operands: retained_facts_v1::FactBuilderV1,
    retained: Option<retained_facts_v1::RetainedFactsV1>,
}

impl<'a> HelperResultRangesV1<'a> {
    fn matches_owner_and_type(
        &self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        operand: &SemanticOperandV1,
    ) -> bool {
        if !std::ptr::eq(types, self.types) || !std::ptr::eq(function, self.function) {
            return false;
        }
        // Do not short-circuit the exact overflow-condition/message matcher.
        matches!(
            types
                .get(operand.ty().index() as usize)
                .map(|ty| ty.shape()),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                ..
            }))
        )
    }

    pub(super) fn at_with_work(
        &self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        operand: &SemanticOperandV1,
        block: usize,
        statement: usize,
        work: &mut usize,
    ) -> Result<Option<UnsignedRangeProofV1>, ProductionRankedProjectionErrorV1> {
        project_loop_graph_charge_v1(work, 1)?;
        if !self.matches_owner_and_type(types, function, operand) {
            return Ok(None);
        }
        let retained =
            self.retained
                .as_ref()
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "helper range facts are not retained",
                ))?;
        retained.at(operand, block, statement, work)
    }

    #[cfg(test)]
    pub(super) fn at(
        &self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        operand: &SemanticOperandV1,
        block: usize,
        statement: usize,
    ) -> Option<UnsignedRangeProofV1> {
        if self.retained.is_some() {
            return self
                .at_with_work(types, function, operand, block, statement, &mut 0)
                .unwrap();
        }
        // Existing engine tests inspect the still-live builder and its ledger.
        if !self.matches_owner_and_type(types, function, operand) {
            return None;
        }
        let (b, s, range) = self.operands.get(&(operand as *const _))?;
        (b == block && s == statement).then_some(range)
    }

    pub(super) fn analyze(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
        work: &mut usize,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let mut cells = WorkingCells::new(MAX_CELLS);
        let mut facts = Self::analyze_with_cells(types, function, work, &mut cells)?;
        let retained = std::mem::take(&mut facts.operands).finish(cells, work)?;
        facts.retained = Some(retained);
        Ok(facts)
    }

    fn analyze_with_cells(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
        work: &mut usize,
        cells: &mut WorkingCells,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let mut scope = working_scope_v1::WorkingScopeV1::new(cells, work)?;
        let facts = Self::analyze_in_cells(types, function, work, scope.cells())?;
        let retained = facts.operands.retained_cells()?;
        scope.finish(facts, retained)
    }

    fn analyze_in_cells(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
        work: &mut usize,
        cells: &mut WorkingCells,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let mut facts = Self {
            types,
            function,
            operands: retained_facts_v1::FactBuilderV1::new(cells, work)?,
            retained: None,
        };
        let n = function.blocks().len();
        let locals = function.locals().len();
        // Optional precision must not require dense unbounded state storage.
        if n > MAX_CELLS || locals > MAX_CELLS {
            return Ok(facts);
        }
        project_loop_graph_charge_v1(work, locals.saturating_add(n))?;
        // Includes graph tables, reachability/worklist capacity, and escaped
        // local bits. Source declarations themselves are borrowed, not cloned.
        let mut graph_cells = n.saturating_mul(8).saturating_add(locals);
        cells.reserve(SCRATCH_CELLS.saturating_add(graph_cells))?;
        // Keep the original conservative cell/work gates. Only successor counts
        // are retained; the immutable body already owns every ordered edge.
        let mut successor_counts = vec![0usize; n];
        let mut edge_count = 0usize;
        for (index, block) in function.blocks().iter().enumerate() {
            block.terminator().kind().try_for_each_edge(|edge| {
                project_loop_graph_charge_v1(work, 1)?;
                edge_count += 1;
                if edge_count > MAX_CELLS {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        STORAGE_LIMIT,
                    ));
                }
                cells.reserve(4)?;
                graph_cells += 4;
                let target = edge.target().index() as usize;
                if target >= n {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "helper-result range edge outside retained function",
                    ));
                }
                successor_counts[index] += 1;
                Ok(())
            })?;
        }
        let entry = function.entry().index() as usize;
        if entry >= n {
            return Ok(facts);
        }
        let mut reachable = vec![false; n];
        let mut pending = vec![entry];
        while let Some(block) = pending.pop() {
            if std::mem::replace(&mut reachable[block], true) {
                continue;
            }
            pending.reserve(successor_counts[block]);
            function.blocks()[block].terminator().kind().try_for_each_edge::<std::convert::Infallible>(|edge| {
                pending.push(edge.target().index() as usize);
                Ok(())
            }).expect("infallible retained edge traversal");
        }
        let mut incoming = vec![0usize; n];
        for (_, block) in function.blocks().iter().enumerate().filter(|(b, _)| reachable[*b]) {
            block.terminator().kind().try_for_each_edge::<std::convert::Infallible>(|edge| {
                incoming[edge.target().index() as usize] += 1;
                Ok(())
            }).expect("infallible retained edge traversal");
        }
        let mut escaped = vec![false; locals];
        for block in function.blocks() {
            for statement in block.statements() {
                project_loop_graph_charge_v1(work, 1)?;
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                    && let SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::AddressOf { place, .. } = assignment.value().kind()
                    && let Some(slot) = escaped.get_mut(place.local().index() as usize)
                {
                    *slot = true;
                }
            }
        }
        let mut engine = Engine {
            types,
            function,
            escaped,
            work,
            cells,
        };
        let mut states: Vec<Option<State>> = vec![None; n];
        engine.cells.reserve(1)?;
        let mut initial = State::new();
        for (i, local) in function.locals().iter().enumerate() {
            if matches!(local.role(), SemanticLocalRoleV1::Argument(_)) && !engine.escaped[i] {
                engine.replace_local(&mut initial, i, engine.unknown_scalar(local.ty(), (n, i)))?;
            }
        }
        states[entry] = Some(initial);
        let mut ready: VecDeque<usize> = (0..n)
            .filter(|&b| reachable[b] && incoming[b] == 0)
            .collect();
        while let Some(index) = ready.pop_front() {
            let block = &function.blocks()[index];
            let mut state = states[index].take();
            if let Some(state) = state.as_mut() {
                for (&local, value) in state.iter_mut() {
                    if let Value::Enum { stamp, .. } = value
                        && stamp.is_none()
                    {
                        *stamp = Some((n + 1 + index, local));
                    }
                }
                for (statement_index, statement) in block.statements().iter().enumerate() {
                    engine.statement(
                        state,
                        statement.kind(),
                        (index, statement_index),
                        &mut facts,
                    )?;
                }
            }
            let outputs = engine.terminator(
                state,
                block.terminator().kind(),
                (index, block.statements().len()),
                &mut facts,
            )?;
            if outputs.len() != successor_counts[index] {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "helper-result range successor count changed",
                ));
            }
            let output_slots = outputs.len();
            let mut outputs = outputs.into_iter();
            block.terminator().kind().try_for_each_edge(|edge| {
                let target = edge.target().index() as usize;
                let output = outputs.next().ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "helper-result range successor count changed",
                ))?;
                project_loop_graph_charge_v1(engine.work, output.as_ref().map_or(0, state_nodes))?;
                if let Some(output) = output {
                    match &mut states[target] {
                        Some(state) => {
                            project_loop_graph_charge_v1(engine.work, state_nodes(state))?;
                            // Missing is top, not an absent predecessor fact.
                            // Retain only facts present on every incoming path.
                            let previous = state_nodes(state);
                            state.retain(|local, _| output.contains_key(local));
                            engine.cells.release(previous - state_nodes(state));
                            for (&local, b) in &output {
                                if let Some(a) = state.get(&local) {
                                    let mut joined = a.clone();
                                    joined.join(b);
                                    engine.replace_local(state, local, joined)?;
                                }
                            }
                            engine.cells.release(state_nodes(&output));
                        }
                        slot @ None => {
                            *slot = Some(output);
                        }
                    }
                }
                incoming[target] -= 1;
                if incoming[target] == 0 {
                    ready.push_back(target);
                }
                Ok(())
            })?;
            if outputs.next().is_some() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "helper-result range successor count changed",
                ));
            }
            drop(outputs);
            // IntoIter retains its backing allocation until the entire batch
            // is consumed, even after individual states were transferred.
            engine.cells.release(output_slots);
        }
        let retained_states = states.iter().flatten().map(state_nodes).sum::<usize>();
        drop(states);
        engine.cells.release(retained_states);
        drop(ready);
        drop(incoming);
        drop(reachable);
        drop(pending);
        drop(successor_counts);
        drop(engine.escaped);
        engine.cells.release(graph_cells + SCRATCH_CELLS);
        Ok(facts)
    }
}

struct Engine<'a, 'w> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    escaped: Vec<bool>,
    work: &'w mut usize,
    cells: &'w mut WorkingCells,
}

impl Engine<'_, '_> {
    fn discriminant_maximum(&self, ty: SemanticTypeIdV1) -> Option<u128> {
        match self.types.get(ty.index() as usize)?.shape() {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: true,
                bits: bits @ 1..=128,
            }) => Some((1u128 << (bits - 1)) - 1),
            _ => self.maximum(ty),
        }
    }

    fn maximum(&self, ty: SemanticTypeIdV1) -> Option<u128> {
        match self.types.get(ty.index() as usize)?.shape() {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => Some(1),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 1..=127,
            }) => {
                let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { bits, .. }) =
                    self.types[ty.index() as usize].shape()
                else {
                    unreachable!()
                };
                Some((1u128 << bits) - 1)
            }
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 128,
            }) => Some(u128::MAX),
            _ => None,
        }
    }

    fn unknown_scalar(&self, ty: SemanticTypeIdV1, stamp: Stamp) -> Value {
        self.maximum(ty).map_or(Value::Unknown, |hi| {
            Value::Scalar(
                Scalar {
                    lo: 0,
                    hi,
                    stamp: Some(stamp),
                },
                None,
            )
        })
    }

    fn place(&self, state: &State, place: &SemanticPlaceV1) -> Value {
        let local = place.local().index() as usize;
        if self.escaped.get(local) != Some(&false) {
            return Value::Unknown;
        }
        if !self.place_types_match(place) {
            return Value::Unknown;
        }
        let Some(mut value) = state.get(&local) else {
            return Value::Unknown;
        };
        let mut projections = place.projections().iter();
        while let Some(projection) = projections.next() {
            value = match (projection.kind(), value) {
                (
                    kind @ (SemanticProjectionKindV1::Index(_)
                    | SemanticProjectionKindV1::ConstantIndex { .. }),
                    Value::Fields(fields),
                ) => {
                    let Some(index) = self.exact_array_index(state, kind, fields.len()) else {
                        return Value::Unknown;
                    };
                    &fields[index]
                }
                (SemanticProjectionKindV1::Field(field), Value::Fields(fields)) => {
                    match fields.get(field as usize) {
                        Some(value) => value,
                        None => return Value::Unknown,
                    }
                }
                (SemanticProjectionKindV1::Downcast(variant), Value::Enum { variants, .. })
                    if variants.len() == 1 =>
                {
                    let Some(fields) = variants.get(&variant) else {
                        return Value::Unknown;
                    };
                    let Some(SemanticProjectionKindV1::Field(field)) =
                        projections.next().map(|p| p.kind())
                    else {
                        return Value::Unknown;
                    };
                    match fields.get(field as usize) {
                        Some(value) => value,
                        None => return Value::Unknown,
                    }
                }
                _ => return Value::Unknown,
            };
        }
        if let Some(scalar) = value.scalar() {
            let maximum = if matches!(value, Value::Scalar(_, Some(Test::Discriminant(..)))) {
                self.discriminant_maximum(place.ty())
            } else {
                self.maximum(place.ty())
            };
            if maximum.is_none_or(|max| scalar.hi > max) {
                return Value::Unknown;
            }
        }
        value.clone()
    }

    fn place_types_match(&self, place: &SemanticPlaceV1) -> bool {
        if place.projections().len() > MAX_DEPTH * 2 {
            return false;
        }
        let Some(local) = self.function.locals().get(place.local().index() as usize) else {
            return false;
        };
        let mut ty = local.ty();
        let mut variant = None;
        for projection in place.projections() {
            let Some(shape) = self.types.get(ty.index() as usize).map(|ty| ty.shape()) else {
                return false;
            };
            let next = match (projection.kind(), shape) {
                (
                    SemanticProjectionKindV1::Index(_)
                    | SemanticProjectionKindV1::ConstantIndex { .. },
                    SemanticTypeShapeV1::Array { element, .. },
                ) if variant.is_none() && self.primitive_array_layout(ty).is_some() => *element,
                (
                    SemanticProjectionKindV1::Downcast(index),
                    SemanticTypeShapeV1::Enum { variants, .. },
                ) if variant.is_none() && variants.get(index as usize).is_some() => {
                    variant = Some(index);
                    ty
                }
                (SemanticProjectionKindV1::Field(index), SemanticTypeShapeV1::Tuple(fields))
                    if variant.is_none() =>
                {
                    match fields.fields().get(index as usize) {
                        Some(ty) => *ty,
                        None => return false,
                    }
                }
                (
                    SemanticProjectionKindV1::Field(index),
                    SemanticTypeShapeV1::Enum { variants, .. },
                ) => {
                    let Some(selected) = variant.take().and_then(|v| variants.get(v as usize))
                    else {
                        return false;
                    };
                    match selected.fields().fields().get(index as usize) {
                        Some(ty) => *ty,
                        None => return false,
                    }
                }
                _ => return false,
            };
            if next != projection.result_type() {
                return false;
            }
            ty = next;
        }
        variant.is_none() && ty == place.ty()
    }

    fn operand(&self, state: &State, operand: &SemanticOperandV1) -> Value {
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.place(state, place)
            }
            SemanticOperandV1::Constant(constant) => {
                if let SemanticConstantValueV1::Bytes(bytes) = constant.value() {
                    return self.primitive_array_constant(constant.ty(), bytes.as_bytes());
                }
                let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                    return Value::Unknown;
                };
                let bytes = match self
                    .types
                    .get(constant.ty().index() as usize)
                    .map(|ty| ty.shape())
                {
                    Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)) => Some(1),
                    Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: bits @ 1..=128,
                    })) => Some(bits.div_ceil(8) as u8),
                    _ => None,
                };
                if bytes == Some(value.size_bytes())
                    && self
                        .maximum(constant.ty())
                        .is_some_and(|max| value.bits() <= max)
                {
                    Value::exact(value.bits())
                } else {
                    Value::Unknown
                }
            }
        }
    }

    fn record(
        &mut self,
        state: &State,
        operand: &SemanticOperandV1,
        site: Stamp,
        facts: &mut HelperResultRangesV1<'_>,
    ) -> Result<Value, ProductionRankedProjectionErrorV1> {
        project_loop_graph_charge_v1(self.work, 1)?;
        if let SemanticOperandV1::Constant(constant) = operand
            && let SemanticConstantValueV1::Bytes(bytes) = constant.value()
        {
            // Decoding is bounded before examining the byte contents.
            project_loop_graph_charge_v1(self.work, bytes.as_bytes().len().min(MAX_FIELDS * 16))?;
        }
        let snapshot = self.operand(state, operand);
        let projections = match operand {
            SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) => {
                p.projections().len().min(MAX_DEPTH * 2 + 1)
            }
            _ => 0,
        };
        project_loop_graph_charge_v1(
            self.work,
            nodes(&snapshot)
                .saturating_mul(2)
                .saturating_add(projections),
        )?;
        if let Some(value) = snapshot.scalar()
            && self.maximum(operand.ty()).is_some()
            && !is_bool(self.types, operand.ty())
            && !matches!(operand, SemanticOperandV1::Constant(_))
        {
            facts.operands.insert(
                operand,
                site,
                UnsignedRangeProofV1 {
                    minimum: value.lo,
                    maximum: value.hi,
                },
                self.cells,
                self.work,
            )?;
        }
        Ok(snapshot)
    }

    fn kill(&mut self, state: &mut State, place: &SemanticPlaceV1) {
        let local = place.local().index() as usize;
        let Some(value) = state.get_mut(&local) else {
            return;
        };
        let previous = nodes(value);
        // Preserve a disjoint checked tuple field after moving its overflow bit.
        if let ([field], Value::Fields(fields)) = (place.projections(), &mut *value)
            && let SemanticProjectionKindV1::Field(field) = field.kind()
            && let Some(value) = fields.get_mut(field as usize)
        {
            self.cells.release(nodes(value) - 1);
            *value = Value::Unknown;
            return;
        }
        state.remove(&local);
        self.cells.release(previous + 1);
    }

    fn consume(&mut self, state: &mut State, operand: &SemanticOperandV1) {
        if let SemanticOperandV1::Move(place) = operand {
            self.kill(state, place);
        }
    }

    fn replace_local(
        &mut self,
        state: &mut State,
        local: usize,
        value: Value,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let previous = state.get(&local).map_or(0, |v| 1 + nodes(v));
        let next = if value == Value::Unknown {
            0
        } else {
            1 + nodes(&value)
        };
        self.cells.replace(previous, next)?;
        if next == 0 {
            state.remove(&local);
        } else {
            state.insert(local, value);
        }
        Ok(())
    }

    fn clear(&mut self, state: &mut State) -> Result<(), ProductionRankedProjectionErrorV1> {
        let previous = state_nodes(state);
        project_loop_graph_charge_v1(self.work, previous)?;
        state.clear();
        self.cells.release(previous - state_nodes(state));
        Ok(())
    }

    fn statement(
        &mut self,
        state: &mut State,
        kind: &SemanticStatementKindV1,
        site: Stamp,
        facts: &mut HelperResultRangesV1<'_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        project_loop_graph_charge_v1(self.work, 1)?;
        match kind {
            SemanticStatementKindV1::Assign(assignment) => {
                project_loop_graph_charge_v1(
                    self.work,
                    state
                        .get(&(assignment.destination().local().index() as usize))
                        .map_or(0, nodes)
                        .saturating_mul(2),
                )?;
                let mut inputs = Vec::new();
                assignment.value().kind().try_visit_operands(|operand| {
                    let snapshot = self.record(state, operand, site, facts)?;
                    self.consume(state, operand);
                    if inputs.len() < MAX_FIELDS {
                        inputs.push(snapshot);
                    }
                    Ok::<(), ProductionRankedProjectionErrorV1>(())
                })?;
                project_loop_graph_charge_v1(
                    self.work,
                    4 + inputs.iter().map(nodes).sum::<usize>() * 2,
                )?;
                let value = self.rvalue(state, assignment.value(), site, &inputs);
                self.kill(state, assignment.destination());
                let local = assignment.destination().local().index() as usize;
                if assignment.destination().projections().is_empty()
                    && assignment.destination().ty() == assignment.value().result_type()
                    && self
                        .function
                        .locals()
                        .get(local)
                        .is_some_and(|l| l.ty() == assignment.destination().ty())
                    && self.escaped.get(local) == Some(&false)
                {
                    self.replace_local(state, local, value)?;
                }
            }
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) => {
                if let Some(value) = state.remove(&(local.index() as usize)) {
                    project_loop_graph_charge_v1(self.work, nodes(&value))?;
                    self.cells.release(nodes(&value) + 1);
                }
            }
            SemanticStatementKindV1::Deinitialize(place)
            | SemanticStatementKindV1::SetDiscriminant { place, .. } => self.kill(state, place),
            SemanticStatementKindV1::Nop => {}
            // No facts are derived from Assume, memory operations or unknown writes.
            _ => self.clear(state)?,
        }
        Ok(())
    }

    fn rvalue(
        &self,
        state: &State,
        value: &SemanticRvalueV1,
        stamp: Stamp,
        inputs: &[Value],
    ) -> Value {
        let fallback = || self.unknown_scalar(value.result_type(), stamp);
        let input = |index| inputs.get(index).unwrap_or(&Value::Unknown);
        match value.kind() {
            SemanticRvalueKindV1::Use(operand) if operand.ty() == value.result_type() => {
                match input(0).clone() {
                    Value::Unknown => fallback(),
                    result => result,
                }
            }
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand,
            } => {
                let source = input(0);
                match (
                    source.scalar(),
                    self.maximum(value.result_type()),
                    self.maximum(operand.ty()),
                ) {
                    (Some(range), Some(max), Some(_)) if range.hi <= max => Value::Scalar(
                        Scalar {
                            lo: range.lo,
                            hi: range.hi,
                            stamp: Some(stamp),
                        },
                        None,
                    ),
                    _ => fallback(),
                }
            }
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } => self.binary(inputs, *operation, left, right, value.result_type(), stamp),
            SemanticRvalueKindV1::CheckedBinary(checked) => {
                let SemanticTypeShapeV1::Tuple(fields) = self
                    .types
                    .get(value.result_type().index() as usize)
                    .map(|ty| ty.shape())
                    .unwrap_or(&SemanticTypeShapeV1::Opaque)
                else {
                    return Value::Unknown;
                };
                if fields.fields().len() != 2
                    || fields.fields()[0] != checked.left().ty()
                    || !is_bool(self.types, fields.fields()[1])
                    || checked.left().ty() != checked.right().ty()
                {
                    return Value::Unknown;
                }
                let operation = match checked.operation() {
                    SemanticCheckedBinaryOpV1::Add => SemanticBinaryOpV1::Add,
                    SemanticCheckedBinaryOpV1::Subtract => SemanticBinaryOpV1::Subtract,
                    SemanticCheckedBinaryOpV1::Multiply => SemanticBinaryOpV1::Multiply,
                };
                if is_bool(self.types, checked.left().ty()) {
                    return Value::Unknown;
                }
                let a = input(0);
                let b = input(1);
                match (a.scalar(), b.scalar(), self.maximum(checked.left().ty())) {
                    (Some(a), Some(b), Some(max)) => {
                        if let Some(result) = arithmetic(operation, a, b, max) {
                            Value::Fields(vec![
                                Value::Scalar(
                                    Scalar {
                                        stamp: Some(stamp),
                                        ..result
                                    },
                                    None,
                                ),
                                Value::exact(0),
                            ])
                        } else {
                            Value::Unknown
                        }
                    }
                    _ => Value::Unknown,
                }
            }
            SemanticRvalueKindV1::UncheckedBinary(unchecked) => {
                let operation = match unchecked.operation() {
                    SemanticUncheckedBinaryOpV1::Add => SemanticBinaryOpV1::Add,
                    SemanticUncheckedBinaryOpV1::Subtract => SemanticBinaryOpV1::Subtract,
                    SemanticUncheckedBinaryOpV1::Multiply => SemanticBinaryOpV1::Multiply,
                };
                // Precision requires independently total numeric bounds. This
                // neither admits source unsafe code nor normalizes this MIR node.
                self.binary(
                    inputs,
                    operation,
                    unchecked.left(),
                    unchecked.right(),
                    value.result_type(),
                    stamp,
                )
            }
            SemanticRvalueKindV1::Aggregate(aggregate)
                if aggregate.operands().len() <= MAX_FIELDS =>
            {
                if inputs.len() != aggregate.operands().len() {
                    return Value::Unknown;
                }
                let fields = inputs.to_vec();
                if fields.iter().any(|value| depth(value) >= MAX_DEPTH)
                    || fields.iter().map(nodes).sum::<usize>() + 1 > MAX_VALUE_NODES
                {
                    return Value::Unknown;
                }
                match (
                    aggregate.kind(),
                    self.types
                        .get(value.result_type().index() as usize)
                        .map(|ty| ty.shape())
                        .unwrap_or(&SemanticTypeShapeV1::Opaque),
                ) {
                    (SemanticAggregateKindV1::Tuple, SemanticTypeShapeV1::Tuple(types))
                        if types
                            .fields()
                            .iter()
                            .copied()
                            .eq(aggregate.operands().iter().map(|op| op.ty())) =>
                    {
                        Value::Fields(fields)
                    }
                    (
                        SemanticAggregateKindV1::EnumVariant(variant),
                        SemanticTypeShapeV1::Enum { variants, .. },
                    ) if variants.len() <= MAX_FIELDS
                        && variants.get(*variant as usize).is_some_and(|variant| {
                            variant
                                .fields()
                                .fields()
                                .iter()
                                .copied()
                                .eq(aggregate.operands().iter().map(|op| op.ty()))
                        }) =>
                    {
                        Value::Enum {
                            stamp: Some(stamp),
                            variants: BTreeMap::from([(*variant, fields)]),
                        }
                    }
                    _ => Value::Unknown,
                }
            }
            SemanticRvalueKindV1::Discriminant(place) => {
                let Value::Enum {
                    stamp: Some(origin),
                    variants: available,
                } = self.place(state, place)
                else {
                    return fallback();
                };
                let SemanticTypeShapeV1::Enum {
                    discriminant,
                    variants,
                } = self
                    .types
                    .get(place.ty().index() as usize)
                    .map(|ty| ty.shape())
                    .unwrap_or(&SemanticTypeShapeV1::Opaque)
                else {
                    return fallback();
                };
                if *discriminant != value.result_type() {
                    return fallback();
                }
                if variants.len() > MAX_FIELDS {
                    return fallback();
                }
                let values: Vec<_> = variants
                    .iter()
                    .map(|variant| variant.discriminant())
                    .collect();
                let mut live = available
                    .keys()
                    .filter_map(|v| values.get(*v as usize))
                    .copied();
                let Some(first) = live.next() else {
                    return fallback();
                };
                let (lo, hi) = live.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v)));
                if self
                    .discriminant_maximum(value.result_type())
                    .is_none_or(|max| hi > max)
                {
                    return fallback();
                }
                Value::Scalar(
                    Scalar {
                        lo,
                        hi,
                        stamp: Some(stamp),
                    },
                    Some(Test::Discriminant(origin, values)),
                )
            }
            _ => fallback(),
        }
    }

    fn binary(
        &self,
        inputs: &[Value],
        op: SemanticBinaryOpV1,
        left: &SemanticOperandV1,
        right: &SemanticOperandV1,
        output: SemanticTypeIdV1,
        stamp: Stamp,
    ) -> Value {
        let fallback = || self.unknown_scalar(output, stamp);
        if left.ty() != right.ty() || self.maximum(left.ty()).is_none() {
            return fallback();
        }
        let a = inputs.first().unwrap_or(&Value::Unknown);
        let b = inputs.get(1).unwrap_or(&Value::Unknown);
        let (Some(a), Some(b)) = (a.scalar(), b.scalar()) else {
            return fallback();
        };
        if matches!(
            op,
            SemanticBinaryOpV1::Equal
                | SemanticBinaryOpV1::NotEqual
                | SemanticBinaryOpV1::LessThan
                | SemanticBinaryOpV1::LessOrEqual
                | SemanticBinaryOpV1::GreaterThan
                | SemanticBinaryOpV1::GreaterOrEqual
        ) {
            if !is_bool(self.types, output) {
                return Value::Unknown;
            }
            let result = compare(op, a, b);
            return Value::Scalar(
                Scalar {
                    lo: result.map_or(0, u128::from),
                    hi: result.map_or(1, u128::from),
                    stamp: Some(stamp),
                },
                Some(Test::Compare(op, a.clone(), b.clone())),
            );
        }
        if output != left.ty() || is_bool(self.types, left.ty()) {
            return fallback();
        }
        match self
            .maximum(output)
            .and_then(|max| arithmetic(op, a, b, max))
        {
            Some(range) => Value::Scalar(
                Scalar {
                    stamp: Some(stamp),
                    ..range
                },
                None,
            ),
            None => fallback(),
        }
    }

    fn copies(
        &mut self,
        state: State,
        count: usize,
    ) -> Result<Vec<Option<State>>, ProductionRankedProjectionErrorV1> {
        let size = state_nodes(&state);
        if count == 0 {
            self.cells.release(size);
            return Ok(Vec::new());
        }
        self.cells.reserve(count)?;
        let additional =
            size.checked_mul(count - 1)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    STORAGE_LIMIT,
                ))?;
        self.cells.reserve(additional)?;
        project_loop_graph_charge_v1(self.work, additional)?;
        let mut outputs = Vec::with_capacity(count);
        for _ in 1..count {
            outputs.push(Some(state.clone()));
        }
        outputs.push(Some(state));
        Ok(outputs)
    }

    fn filtered(
        &mut self,
        state: State,
        value: &Value,
        selected: Option<u128>,
        excluded: &[u128],
    ) -> Option<State> {
        let previous = state_nodes(&state);
        let result = refine(state, value, selected, excluded);
        self.cells
            .release(previous - result.as_ref().map_or(0, state_nodes));
        result
    }

    fn terminator(
        &mut self,
        state: Option<State>,
        kind: &SemanticTerminatorKindV1,
        site: Stamp,
        facts: &mut HelperResultRangesV1<'_>,
    ) -> Result<Vec<Option<State>>, ProductionRankedProjectionErrorV1> {
        let mut count = 0;
        kind.try_for_each_edge::<()>(|_| {
            count += 1;
            Ok(())
        })
        .unwrap();
        let Some(mut state) = state else {
            self.cells.reserve(count)?;
            return Ok(vec![None; count]);
        };
        project_loop_graph_charge_v1(self.work, state_nodes(&state))?;
        match kind {
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } => {
                let value = self.record(&state, discriminant, site, facts)?;
                self.consume(&mut state, discriminant);
                let excluded_slots = targets.values().len();
                self.cells.reserve(count.saturating_add(excluded_slots))?;
                // The source state remains live while every outgoing clone is
                // retained. Queued states are reserved by the same owner.
                let mut outputs = Vec::with_capacity(count);
                for target in targets.values() {
                    let size = state_nodes(&state);
                    self.cells.reserve(size)?;
                    project_loop_graph_charge_v1(self.work, size.saturating_mul(5))?;
                    outputs.push(self.filtered(state.clone(), &value, Some(target.value()), &[]));
                }
                let excluded: Vec<_> = targets.values().iter().map(|v| v.value()).collect();
                project_loop_graph_charge_v1(
                    self.work,
                    state_nodes(&state)
                        .saturating_mul(5)
                        .saturating_add(excluded.len().saturating_mul(MAX_FIELDS + 1)),
                )?;
                outputs.push(self.filtered(state, &value, None, &excluded));
                drop(excluded);
                self.cells.release(excluded_slots);
                Ok(outputs)
            }
            SemanticTerminatorKindV1::Goto(_) => self.copies(state, count),
            SemanticTerminatorKindV1::Assert {
                condition, unwind, ..
            } => {
                self.record(&state, condition, site, facts)?;
                self.consume(&mut state, condition);
                // No assertion assumption or success-edge narrowing.
                if *unwind != SemanticUnwindActionV1::Unreachable {
                    self.clear(&mut state)?;
                }
                self.copies(state, count)
            }
            SemanticTerminatorKindV1::Call(call) => {
                for operand in call.arguments() {
                    self.record(&state, operand, site, facts)?;
                    self.consume(&mut state, operand);
                }
                if call.unwind() != SemanticUnwindActionV1::Unreachable {
                    self.clear(&mut state)?;
                    return self.copies(state, count);
                }
                if let Some(destination) = call.destination() {
                    self.kill(&mut state, destination.place());
                    let local = destination.place().local().index() as usize;
                    if destination.place().projections().is_empty()
                        && self.escaped.get(local) == Some(&false)
                        && self
                            .function
                            .locals()
                            .get(local)
                            .is_some_and(|l| l.ty() == destination.place().ty())
                    {
                        self.replace_local(
                            &mut state,
                            local,
                            self.unknown_scalar(destination.place().ty(), site),
                        )?;
                    }
                }
                self.copies(state, count)
            }
            _ => {
                self.clear(&mut state)?;
                self.copies(state, count)
            }
        }
    }
}

fn is_bool(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    matches!(
        types.get(ty.index() as usize).map(|ty| ty.shape()),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
    )
}

fn nodes(value: &Value) -> usize {
    1 + match value {
        Value::Scalar(_, Some(Test::Discriminant(_, values))) => values.len(),
        Value::Fields(fields) => fields.iter().map(nodes).sum::<usize>(),
        Value::Enum { variants, .. } => {
            variants.len() + variants.values().flatten().map(nodes).sum::<usize>()
        }
        _ => 0,
    }
}

fn state_nodes(state: &State) -> usize {
    1 + state.values().map(|v| 1 + nodes(v)).sum::<usize>()
}

fn depth(value: &Value) -> usize {
    match value {
        Value::Fields(fields) => 1 + fields.iter().map(depth).max().unwrap_or(0),
        Value::Enum { variants, .. } => {
            1 + variants.values().flatten().map(depth).max().unwrap_or(0)
        }
        _ => 0,
    }
}

fn arithmetic(op: SemanticBinaryOpV1, a: &Scalar, b: &Scalar, max: u128) -> Option<Scalar> {
    let (lo, hi) = match op {
        SemanticBinaryOpV1::Add => (a.lo.checked_add(b.lo)?, a.hi.checked_add(b.hi)?),
        SemanticBinaryOpV1::Subtract => (a.lo.checked_sub(b.hi)?, a.hi.checked_sub(b.lo)?),
        SemanticBinaryOpV1::Multiply => (a.lo.checked_mul(b.lo)?, a.hi.checked_mul(b.hi)?),
        SemanticBinaryOpV1::Divide if b.lo > 0 => (a.lo / b.hi, a.hi / b.lo),
        SemanticBinaryOpV1::Remainder if b.lo > 0 => {
            if a.lo == a.hi && b.lo == b.hi {
                (a.lo % b.lo, a.lo % b.lo)
            } else {
                (0, a.hi.min(b.hi - 1))
            }
        }
        _ => return None,
    };
    (lo <= hi && hi <= max).then_some(Scalar {
        lo,
        hi,
        stamp: None,
    })
}

fn compare(op: SemanticBinaryOpV1, a: &Scalar, b: &Scalar) -> Option<bool> {
    use SemanticBinaryOpV1::*;
    match op {
        Equal if a.hi < b.lo || b.hi < a.lo => Some(false),
        Equal if a.lo == a.hi && b.lo == b.hi => Some(a.lo == b.lo),
        NotEqual => compare(Equal, a, b).map(|v| !v),
        LessThan if a.hi < b.lo => Some(true),
        LessThan if a.lo >= b.hi => Some(false),
        LessOrEqual => compare(LessThan, b, a).map(|v| !v),
        GreaterThan => compare(LessThan, b, a),
        GreaterOrEqual => compare(LessThan, a, b).map(|v| !v),
        _ => None,
    }
}

fn refine(
    mut state: State,
    value: &Value,
    selected: Option<u128>,
    excluded: &[u128],
) -> Option<State> {
    let Value::Scalar(range, test) = value else {
        return Some(state);
    };
    let permits = |v| selected.map_or(!excluded.contains(&v), |selected| selected == v);
    if selected.is_some_and(|v| v < range.lo || v > range.hi)
        || (range.lo == range.hi && !permits(range.lo))
    {
        return None;
    }
    if let Some(Test::Discriminant(stamp, values)) = test {
        if values.len() > MAX_FIELDS {
            return Some(state);
        }
        let mut allowed = [false; MAX_FIELDS];
        for (index, value) in values.iter().enumerate() {
            allowed[index] = permits(*value);
        }
        for value in state.values_mut() {
            if let Value::Enum {
                stamp: Some(origin),
                variants,
            } = value
                && origin == stamp
            {
                variants.retain(|variant, _| allowed.get(*variant as usize) == Some(&true));
                if variants.is_empty() {
                    return None;
                }
            }
        }
    }
    let truth = match selected {
        Some(0) => Some(false),
        Some(1) => Some(true),
        None if excluded == [0] => Some(true),
        None if excluded == [1] => Some(false),
        _ => None,
    };
    if let (Some(Test::Compare(op, a, b)), Some(truth)) = (test, truth) {
        narrow(&mut state, *op, a, b, truth)?;
    }
    Some(state)
}

fn narrow(
    state: &mut State,
    op: SemanticBinaryOpV1,
    a: &Scalar,
    b: &Scalar,
    truth: bool,
) -> Option<()> {
    use SemanticBinaryOpV1::*;
    let (a, b, strict) = match (op, truth) {
        (LessThan, true) | (GreaterOrEqual, false) => (a, b, true),
        (LessOrEqual, true) | (GreaterThan, false) => (a, b, false),
        (GreaterThan, true) | (LessOrEqual, false) => (b, a, true),
        (GreaterOrEqual, true) | (LessThan, false) => (b, a, false),
        (Equal, true) | (NotEqual, false) => {
            let lo = a.lo.max(b.lo);
            let hi = a.hi.min(b.hi);
            if lo > hi {
                return None;
            }
            restrict(state, a.stamp, lo, hi)?;
            return restrict(state, b.stamp, lo, hi);
        }
        (Equal, false) | (NotEqual, true) => {
            if b.lo == 0 && b.hi == 0 {
                restrict(state, a.stamp, 1, u128::MAX)?;
            }
            if a.lo == 0 && a.hi == 0 {
                restrict(state, b.stamp, 1, u128::MAX)?;
            }
            return Some(());
        }
        _ => return Some(()),
    };
    restrict(state, a.stamp, 0, b.hi.checked_sub(u128::from(strict))?)?;
    restrict(
        state,
        b.stamp,
        a.lo.checked_add(u128::from(strict))?,
        u128::MAX,
    )
}

fn restrict(state: &mut State, stamp: Option<Stamp>, lo: u128, hi: u128) -> Option<()> {
    let Some(stamp) = stamp else {
        return Some(());
    };
    for value in state.values_mut() {
        if let Value::Scalar(range, _) = value
            && range.stamp == Some(stamp)
        {
            range.lo = range.lo.max(lo);
            range.hi = range.hi.min(hi);
            if range.lo > range.hi {
                return None;
            }
        }
    }
    Some(())
}

#[cfg(test)]
#[path = "helper_result_ranges_v1/tests.rs"]
mod tests;
