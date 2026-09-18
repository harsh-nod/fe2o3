//! Private mechanical template algebra. This does not establish source/callee
//! identity, totality, ABI, effects, or correspondence; both adapters do so.
use super::*;

#[cfg(test)]
#[path = "native_helper_value_template_v1_tests.rs"]
mod tests;

type Scalar = ProductionSemanticScalarTypeV2;
type Expression = NormalizedScalarExpressionV1;
type Error = &'static str;
const NODES: usize = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
const DEPTH: usize = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2;

struct Stats {
    nodes: usize,
    depth: usize,
}

impl NormalizedScalarExpressionV1 {
    fn template_scalar(&self) -> Scalar {
        match self {
            Self::Symbol { scalar, .. }
            | Self::Constant { scalar, .. }
            | Self::Load { scalar, .. }
            | Self::Unary { scalar, .. }
            | Self::Binary { scalar, .. }
            | Self::Select { scalar, .. } => *scalar,
            Self::Compare { .. } => Scalar::Bool,
            Self::Cast { target, .. } => *target,
        }
    }

    fn template_validate(&self, meter: &mut dyn Meter) -> Result<Stats, Error> {
        fn walk(
            expression: &NormalizedScalarExpressionV1,
            depth: usize,
            stats: &mut Stats,
            meter: &mut dyn Meter,
        ) -> Result<(), Error> {
            use NormalizedScalarExpressionV1 as E;
            stats.nodes = stats
                .nodes
                .checked_add(1)
                .ok_or("native expression size overflow")?;
            stats.depth = stats.depth.max(depth);
            if stats.nodes > NODES || depth > DEPTH {
                return Err("native expression expansion exceeds limits");
            }
            let scalar = expression.template_scalar();
            if !matches!(
                scalar,
                Scalar::Bool
                    | Scalar::Integer {
                        bits: 8 | 16 | 32 | 64,
                        ..
                    }
                    | Scalar::Float { bits: 32 | 64 }
            ) {
                return Err("native expression scalar outside domain");
            }
            let next = depth
                .checked_add(1)
                .ok_or("native expression depth overflow")?;
            match expression {
                E::Symbol { symbol, scalar } => {
                    ProductionSemanticExpressionV2::Symbol {
                        symbol: *symbol,
                        scalar: *scalar,
                    }
                    .validate()
                    .map_err(|_| "native expression symbol invalid")?;
                }
                E::Constant { scalar, bits } => {
                    ProductionSemanticExpressionV2::Constant {
                        scalar: *scalar,
                        bits: *bits,
                    }
                    .validate()
                    .map_err(|_| "native expression constant invalid")?;
                }
                E::Load { .. } => {}
                E::Unary {
                    scalar, operand, ..
                } => {
                    if operand.template_scalar() != *scalar {
                        return Err("native unary template type mismatch");
                    }
                    walk(operand, next, stats, meter)?;
                }
                E::Binary {
                    operation,
                    scalar,
                    overflow,
                    lhs,
                    rhs,
                } => {
                    if matches!(
                        operation,
                        ProductionSemanticBinaryOpV2::ShiftLeft
                            | ProductionSemanticBinaryOpV2::ShiftRight
                    ) {
                        meter.work(8)?;
                        if *overflow != ProductionOverflowContractV2::Wrapping
                            || fixed_native_shift_count_v1(rhs, *scalar).is_none()
                        {
                            return Err("native shift template is not an exact in-range constant");
                        }
                    }
                    if lhs.template_scalar() != *scalar || rhs.template_scalar() != *scalar {
                        return Err("native binary template type mismatch");
                    }
                    walk(lhs, next, stats, meter)?;
                    walk(rhs, next, stats, meter)?;
                }
                E::Compare {
                    operand_scalar,
                    lhs,
                    rhs,
                    ..
                } => {
                    if lhs.template_scalar() != *operand_scalar
                        || rhs.template_scalar() != *operand_scalar
                    {
                        return Err("native comparison template type mismatch");
                    }
                    walk(lhs, next, stats, meter)?;
                    walk(rhs, next, stats, meter)?;
                }
                E::Select {
                    scalar,
                    condition,
                    when_true,
                    when_false,
                } => {
                    if condition.template_scalar() != Scalar::Bool
                        || when_true.template_scalar() != *scalar
                        || when_false.template_scalar() != *scalar
                    {
                        return Err("native select template type mismatch");
                    }
                    walk(condition, next, stats, meter)?;
                    walk(when_true, next, stats, meter)?;
                    walk(when_false, next, stats, meter)?;
                }
                E::Cast {
                    source, operand, ..
                } => {
                    if operand.template_scalar() != *source {
                        return Err("native cast template type mismatch");
                    }
                    walk(operand, next, stats, meter)?;
                }
            }
            Ok(())
        }
        let mut stats = Stats { nodes: 0, depth: 0 };
        walk(self, 0, &mut stats, meter)?;
        Ok(stats)
    }
}

pub(super) trait Meter {
    fn work(&mut self, amount: usize) -> Result<(), Error>;
    fn reserve(&mut self, bytes: usize) -> Result<(), Error>;
    fn release(&mut self, bytes: usize) -> Result<(), Error>;
    fn exhausted(&self) -> bool;
    fn storage(&self) -> Result<usize, Error>;
    fn identity(&mut self) -> Result<Ledger, Error>;
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct Ledger {
    pub(super) slot: usize,
    pub(super) work: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

/// In-place bounded heapsort. Every comparison and swap is paid before use;
/// unlike a fallible comparator adapter, exhaustion stops the traversal.
pub(super) fn sort_metered<T>(
    rows: &mut [T],
    width: usize,
    meter: &mut dyn Meter,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> Result<(), Error> {
    fn sift<T>(
        rows: &mut [T],
        mut parent: usize,
        width: usize,
        meter: &mut dyn Meter,
        compare: &impl Fn(&T, &T) -> std::cmp::Ordering,
    ) -> Result<(), Error> {
        loop {
            meter.work(1)?;
            let left = parent
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or("native helper sort arithmetic")?;
            if left >= rows.len() {
                return Ok(());
            }
            let mut largest = left;
            if left + 1 < rows.len() {
                meter.work(width)?;
                if compare(&rows[left], &rows[left + 1]).is_lt() {
                    largest = left + 1;
                }
            }
            meter.work(width)?;
            if !compare(&rows[parent], &rows[largest]).is_lt() {
                return Ok(());
            }
            meter.work(1)?;
            rows.swap(parent, largest);
            parent = largest;
        }
    }
    for parent in (0..rows.len() / 2).rev() {
        sift(rows, parent, width, meter, &compare)?;
    }
    for end in (1..rows.len()).rev() {
        meter.work(1)?;
        rows.swap(0, end);
        sift(&mut rows[..end], 0, width, meter, &compare)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ArenaBrand(u64);

impl ArenaBrand {
    fn fresh() -> Result<Self, Error> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        // Private equality-only identity. It never contributes to serialized
        // bytes, compiler ordering, receipts or budget sizes. Never wrap/reuse.
        NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map(Self)
            .map_err(|_| "template arena identity exhausted")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Value {
    arena: ArenaBrand,
    index: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    Parameter(usize),
    Constant(u64),
    Unary(ProductionSemanticUnaryOpV2, Value),
    Binary(
        ProductionSemanticBinaryOpV2,
        ProductionOverflowContractV2,
        Value,
        Value,
    ),
    Compare(ProductionSemanticComparisonV2, Scalar, Value, Value),
    Select(Value, Value, Value),
    Cast(ProductionSemanticCastV2, Scalar, Value),
}

impl Kind {
    fn children(self) -> [Option<Value>; 3] {
        match self {
            Self::Parameter(_) | Self::Constant(_) => [None; 3],
            Self::Unary(_, value) | Self::Cast(_, _, value) => [Some(value), None, None],
            Self::Binary(_, _, left, right) | Self::Compare(_, _, left, right) => {
                [Some(left), Some(right), None]
            }
            Self::Select(condition, yes, no) => [Some(condition), Some(yes), Some(no)],
        }
    }

    fn remap(self, arena: ArenaBrand, map: &[Value]) -> Result<Self, Error> {
        let get = |value: Value| {
            if value.arena != arena {
                return Err("foreign template arena");
            }
            map.get(value.index).copied().ok_or("foreign template edge")
        };
        Ok(match self {
            Self::Parameter(_) => return Err("uninstantiated template parameter"),
            Self::Constant(bits) => Self::Constant(bits),
            Self::Unary(operation, value) => Self::Unary(operation, get(value)?),
            Self::Binary(operation, overflow, left, right) => {
                Self::Binary(operation, overflow, get(left)?, get(right)?)
            }
            Self::Compare(operation, scalar, left, right) => {
                Self::Compare(operation, scalar, get(left)?, get(right)?)
            }
            Self::Select(condition, yes, no) => Self::Select(get(condition)?, get(yes)?, get(no)?),
            Self::Cast(kind, source, value) => Self::Cast(kind, source, get(value)?),
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct Node {
    scalar: Scalar,
    kind: Kind,
    tree_nodes: usize,
    depth: usize,
}

/// A local arena has no public constructors for edges and cannot be serialized.
/// The adapter owns its exact function association and its complete body census.
pub(super) struct Template {
    brand: ArenaBrand,
    nodes: Vec<Node>,
    parameters: Vec<Scalar>,
    result: Option<Value>,
    storage: usize,
}

fn bytes<T>(count: usize) -> Result<usize, Error> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or("template size overflow")
}

// Reserve the old/new capacity overlap before a potentially moving allocation.
// Every caller owns the returned reservation until that Vec is dropped.
pub(super) fn vector<T>(count: usize, meter: &mut dyn Meter) -> Result<(Vec<T>, usize), Error> {
    let requested = bytes::<T>(count)?;
    meter.work(count.checked_add(1).ok_or("template work overflow")?)?;
    meter.reserve(requested)?;
    let mut result = Vec::new();
    if result.try_reserve_exact(count).is_err() {
        meter.release(requested)?;
        return Err("template vector allocation failed");
    }
    let actual = bytes::<T>(result.capacity())?;
    if actual > requested
        && let Err(error) = meter.reserve(actual - requested)
    {
        drop(result);
        meter.release(requested)?;
        return Err(error);
    }
    Ok((result, actual))
}

impl Template {
    pub(super) fn new(parameters: &[Scalar], meter: &mut dyn Meter) -> Result<Self, Error> {
        meter.work(1)?;
        let brand = ArenaBrand::fresh()?;
        let (mut copied, storage) = vector(parameters.len(), meter)?;
        copied.extend_from_slice(parameters);
        Ok(Self {
            brand,
            nodes: Vec::new(),
            parameters: copied,
            result: None,
            storage,
        })
    }

    pub(super) fn scalar(&self, value: Value) -> Result<Scalar, Error> {
        if value.arena != self.brand {
            return Err("foreign template arena");
        }
        self.nodes
            .get(value.index)
            .map(|node| node.scalar)
            .ok_or("foreign template value")
    }

    pub(super) fn push(
        &mut self,
        scalar: Scalar,
        kind: Kind,
        meter: &mut dyn Meter,
    ) -> Result<Value, Error> {
        meter.work(5)?;
        if self.result.is_some() || self.nodes.len() >= NODES {
            return Err("closed or oversized template");
        }
        let mut tree_nodes = 1usize;
        let mut depth = 0usize;
        for child in kind.children().into_iter().flatten() {
            if child.arena != self.brand {
                return Err("foreign template arena");
            }
            let node = self
                .nodes
                .get(child.index)
                .ok_or("foreign or cyclic template edge")?;
            tree_nodes = tree_nodes
                .checked_add(node.tree_nodes)
                .ok_or("template size overflow")?;
            depth = depth.max(node.depth.checked_add(1).ok_or("template depth overflow")?);
        }
        if tree_nodes > NODES || depth > DEPTH {
            return Err("template expansion exceeds expression limits");
        }
        if let Kind::Parameter(parameter) = kind
            && self.parameters.get(parameter).copied() != Some(scalar)
        {
            return Err("template parameter type or ordinal mismatch");
        }
        if self.nodes.len() == self.nodes.capacity() {
            let capacity = self
                .nodes
                .capacity()
                .checked_mul(2)
                .unwrap_or(NODES)
                .clamp(8, NODES);
            let (mut replacement, actual) = vector(capacity, meter)?;
            let old = bytes::<Node>(self.nodes.capacity())?;
            replacement.extend_from_slice(&self.nodes);
            drop(std::mem::replace(&mut self.nodes, replacement));
            meter.release(old)?;
            self.storage = self
                .storage
                .checked_sub(old)
                .and_then(|n| n.checked_add(actual))
                .ok_or("template storage accounting overflow")?;
        }
        let value = Value {
            arena: self.brand,
            index: self.nodes.len(),
        };
        self.nodes.push(Node {
            scalar,
            kind,
            tree_nodes,
            depth,
        });
        Ok(value)
    }

    pub(super) fn finish(&mut self, result: Value, expected: Scalar) -> Result<(), Error> {
        if self.result.is_some() || self.scalar(result)? != expected {
            return Err("template result type or multiplicity mismatch");
        }
        self.result = Some(result);
        Ok(())
    }

    /// Substitution between independently scoped helper arenas. Parameters are
    /// replaced with caller values, never reinterpreted as source symbols.
    pub(super) fn append_call(
        &mut self,
        callee: &Self,
        arguments: &[Value],
        meter: &mut dyn Meter,
    ) -> Result<Value, Error> {
        meter.work(
            arguments
                .len()
                .checked_add(1)
                .ok_or("template work overflow")?,
        )?;
        if arguments.len() != callee.parameters.len() || callee.result.is_none() {
            return Err("template call arity or return mismatch");
        }
        for (argument, expected) in arguments.iter().zip(&callee.parameters) {
            if self.scalar(*argument)? != *expected {
                return Err("template call argument type mismatch");
            }
        }
        let (mut map, storage) = vector(callee.nodes.len(), meter)?;
        let result = (|| {
            for node in &callee.nodes {
                meter.work(1)?;
                let value = match node.kind {
                    Kind::Parameter(parameter) => *arguments
                        .get(parameter)
                        .ok_or("template parameter ordinal mismatch")?,
                    kind => self.push(node.scalar, kind.remap(callee.brand, &map)?, meter)?,
                };
                map.push(value);
            }
            let result = callee.result.ok_or("missing template result")?;
            if result.arena != callee.brand {
                return Err("foreign template return arena");
            }
            map.get(result.index)
                .copied()
                .ok_or("foreign template return")
        })();
        drop(map);
        meter.release(storage)?;
        result
    }

    /// The returned tree carries `storage` until dropped or transferred into
    /// another already-reserved owner. The private parameter vocabulary cannot
    /// occur in the existing public expression wire.
    #[cfg(test)]
    pub(super) fn instantiate(
        &self,
        arguments: &[Expression],
        meter: &mut dyn Meter,
    ) -> Result<(Expression, usize), Error> {
        self.instantiate_with_node_limit(arguments, NODES, meter)
            .map(|(expression, storage, _)| (expression, storage))
    }

    pub(super) fn instantiate_with_node_limit(
        &self,
        arguments: &[Expression],
        node_limit: usize,
        meter: &mut dyn Meter,
    ) -> Result<(Expression, usize, usize), Error> {
        let result = self.result.ok_or("unfinished helper template")?;
        if arguments.len() != self.parameters.len() {
            return Err("template call arity mismatch");
        }
        let (mut argument_sizes, argument_storage) = vector(arguments.len(), meter)?;
        let prepared = (|| {
            for (argument, scalar) in arguments.iter().zip(&self.parameters) {
                // Existing validation is bounded, but must not be unmetered.
                meter.work(NODES)?;
                let metadata = argument
                    .template_validate(meter)
                    .map_err(|_| "invalid template argument")?;
                if argument.template_scalar() != *scalar {
                    return Err("template argument type mismatch");
                }
                let indices = 0usize;
                argument_sizes.push((metadata.nodes, metadata.depth, indices));
            }
            self.expanded_size(result, &argument_sizes, meter)
        })();
        drop(argument_sizes);
        meter.release(argument_storage)?;
        let (nodes, _, indices) = prepared?;
        if nodes > node_limit {
            return Err("substituted template exceeds enclosing argument allowance");
        }
        meter.work(
            nodes
                .checked_mul(2)
                .and_then(|n| n.checked_add(indices))
                .ok_or("template work overflow")?,
        )?;
        let storage = bytes::<Expression>(nodes)?
            .checked_add(bytes::<u8>(indices)?)
            .ok_or("template load payload overflow")?;
        meter.reserve(storage)?;
        let expression = self.emit(result, arguments);
        match expression {
            Ok(expression) => {
                if expression.template_validate(meter).is_err() {
                    drop(expression);
                    meter.release(storage)?;
                    Err("instantiated template is not a typed expression")
                } else {
                    Ok((expression, storage, nodes))
                }
            }
            Err(error) => {
                meter.release(storage)?;
                Err(error)
            }
        }
    }

    fn expanded_size(
        &self,
        result: Value,
        arguments: &[(usize, usize, usize)],
        meter: &mut dyn Meter,
    ) -> Result<(usize, usize, usize), Error> {
        let (mut sizes, storage) = vector::<(usize, usize, usize)>(self.nodes.len(), meter)?;
        let result = (|| {
            for node in &self.nodes {
                meter.work(4)?;
                let mut size = (1usize, 0usize, 0usize);
                if let Kind::Parameter(parameter) = node.kind {
                    size = *arguments
                        .get(parameter)
                        .ok_or("unbound template parameter")?;
                } else {
                    for child in node.kind.children().into_iter().flatten() {
                        if child.arena != self.brand {
                            return Err("foreign template arena");
                        }
                        let (nodes, depth, indices) =
                            *sizes.get(child.index).ok_or("cyclic template edge")?;
                        size.0 = size
                            .0
                            .checked_add(nodes)
                            .ok_or("template expansion overflow")?;
                        size.1 = size
                            .1
                            .max(depth.checked_add(1).ok_or("template depth overflow")?);
                        size.2 = size
                            .2
                            .checked_add(indices)
                            .ok_or("template load payload overflow")?;
                    }
                }
                if size.0 > NODES || size.1 > DEPTH {
                    return Err("substituted template exceeds expression limits");
                }
                sizes.push(size);
            }
            if result.arena != self.brand {
                return Err("foreign template return arena");
            }
            sizes
                .get(result.index)
                .copied()
                .ok_or("foreign template result")
        })();
        drop(sizes);
        meter.release(storage)?;
        result
    }

    // Edges are backward-only and the preflight proved expanded depth <= 128.
    // No partial intermediate tree exists before the full expansion is paid.
    fn emit(&self, value: Value, arguments: &[Expression]) -> Result<Expression, Error> {
        if value.arena != self.brand {
            return Err("foreign template arena");
        }
        let node = self
            .nodes
            .get(value.index)
            .ok_or("foreign template value")?;
        let scalar = node.scalar;
        let child = |value| self.emit(value, arguments).map(Box::new);
        Ok(match node.kind {
            Kind::Parameter(parameter) => arguments
                .get(parameter)
                .cloned()
                .ok_or("unbound template parameter")?,
            Kind::Constant(bits) => Expression::Constant { scalar, bits },
            Kind::Unary(operation, value) => Expression::Unary {
                operation,
                scalar,
                operand: child(value)?,
            },
            Kind::Binary(operation, overflow, left, right) => Expression::Binary {
                operation,
                scalar,
                overflow,
                lhs: child(left)?,
                rhs: child(right)?,
            },
            Kind::Compare(operation, operand_scalar, left, right) => Expression::Compare {
                operation,
                operand_scalar,
                lhs: child(left)?,
                rhs: child(right)?,
            },
            Kind::Select(condition, yes, no) => Expression::Select {
                scalar,
                condition: child(condition)?,
                when_true: child(yes)?,
                when_false: child(no)?,
            },
            Kind::Cast(kind, source, value) => Expression::Cast {
                kind,
                source,
                target: scalar,
                operand: child(value)?,
            },
        })
    }

    pub(super) fn destroy(self, meter: &mut dyn Meter) -> Result<(), Error> {
        let storage = self.storage;
        drop(self);
        meter.release(storage)
    }

    pub(super) fn retained_storage(&self) -> usize {
        self.storage
    }
}
