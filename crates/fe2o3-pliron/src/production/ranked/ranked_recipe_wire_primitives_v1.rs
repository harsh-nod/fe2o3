use super::*;
use sha2::Sha256;
use std::panic::AssertUnwindSafe;

pub(super) fn limit(field: &'static str, actual: usize, maximum: usize) -> R<()> {
    if actual > maximum {
        Err(E::Limit {
            field,
            actual,
            limit: maximum,
        })
    } else {
        Ok(())
    }
}
pub(super) fn add(a: usize, b: usize) -> R<usize> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}
pub(super) fn product(a: usize, b: usize) -> R<usize> {
    a.checked_mul(b).ok_or_else(|| Resource::Arithmetic.into())
}
pub(super) fn need<T>(value: Option<T>) -> R<T> {
    value.ok_or_else(|| Resource::Accounting.into())
}

pub(super) fn scope<T>(
    budget: &mut Budget<'_>,
    body: impl FnOnce(&mut Budget<'_>) -> R<(T, usize)>,
) -> R<(T, RankedRecipeStorageV1)> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const _;
    budget.charge_work(2)?;
    let result =
        std::panic::catch_unwind(AssertUnwindSafe(|| body(budget))).unwrap_or(Err(E::Panicked));
    if budget.work_ledger_identity_v1() != ledger
        || !std::ptr::eq(slot, budget)
        || budget.storage() < floor
    {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    match result {
        Ok((value, retained)) => {
            let live = budget.storage() - floor;
            if retained > live {
                drop(value);
                budget.release_storage(live)?;
                return Err(Resource::Accounting.into());
            }
            budget.release_storage(live)?;
            Ok((value, RankedRecipeStorageV1(retained)))
        }
        Err(error) => {
            budget.release_storage(budget.storage() - floor)?;
            Err(error)
        }
    }
}

pub(super) fn vector<T>(count: usize, budget: &mut Budget<'_>) -> R<(Vec<T>, usize)> {
    let requested = product(count, size_of::<T>())?;
    budget.reserve_storage(requested)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = product(values.capacity(), size_of::<T>())?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok((values, actual))
}

pub(super) fn identity(bytes: &[u8], budget: &mut Budget<'_>) -> R<[u8; 32]> {
    let scratch = size_of::<Sha256>() + size_of::<[u8; 8]>();
    budget.reserve_storage(scratch)?;
    budget.charge_work(add(add(RANKED_RECIPE_DOMAIN_V1.len(), 9)?, bytes.len())?)?;
    let mut hash = Sha256::new();
    hash.update(RANKED_RECIPE_DOMAIN_V1);
    hash.update(
        u64::try_from(bytes.len())
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    hash.update(bytes);
    let identity = hash.finalize().into();
    budget.release_storage(scratch)?;
    Ok(identity)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Shape {
    pub blocks: usize,
    pub operations: usize,
    pub expressions: usize,
    pub weighted: usize,
    pub arguments: usize,
    pub tensor_sites: usize,
    pub tensor_claims: usize,
    pub components: usize,
}
impl Shape {
    pub fn block(&mut self, arguments: usize) -> R<()> {
        limit(
            "block arguments",
            arguments,
            HARD_MAX_PRODUCTION_RANKED_ARGUMENTS,
        )?;
        self.blocks = add(self.blocks, 1)?;
        self.arguments = add(self.arguments, arguments)?;
        limit("blocks", self.blocks, MAX_RANKED_BOUNDS_BLOCKS)?;
        limit(
            "total block arguments",
            self.arguments,
            MAX_RANKED_BOUNDS_OPERATIONS,
        )
    }
    pub fn operation(&mut self, tag: u16, expression_nodes: usize) -> R<()> {
        self.operations = add(self.operations, 1)?;
        let weight = match tag {
            31 => add(expression_nodes, 1)?,
            34..=41 => 3,
            _ => 1,
        };
        self.weighted = add(self.weighted, weight)?;
        self.tensor_sites = add(self.tensor_sites, usize::from(tag == 26))?;
        self.tensor_claims = add(self.tensor_claims, usize::from(matches!(tag, 40 | 41)))?;
        self.finish_limits()
    }
    pub fn terminator(&mut self, tag: u16) -> R<()> {
        self.weighted = add(self.weighted, 1 + usize::from(matches!(tag, 9 | 10)))?;
        self.finish_limits()
    }
    pub fn expression(&mut self) -> R<()> {
        self.expressions = add(self.expressions, 1)?;
        limit(
            "total expression nodes",
            self.expressions,
            MAX_RANKED_BOUNDS_OPERATIONS,
        )
    }
    fn finish_limits(&self) -> R<()> {
        limit(
            "materialized operations",
            self.weighted,
            MAX_RANKED_BOUNDS_OPERATIONS,
        )?;
        limit(
            "tensor sites",
            self.tensor_sites,
            MAX_PRODUCTION_TENSOR_REFINEMENT_SITES_V1,
        )?;
        limit(
            "tensor claims",
            self.tensor_claims,
            MAX_PRODUCTION_TENSOR_REFINEMENT_SITES_V1,
        )?;
        let tree = ranked_tree_work(self.blocks, self.weighted).ok_or(Resource::Arithmetic)?;
        limit(
            "operation tree",
            tree,
            HARD_MAX_SESSION_OPERATION_TREE_ITEMS,
        )
    }
    pub fn finish(&self) -> R<()> {
        if self.blocks == 0 {
            return Err(E::Header);
        }
        self.finish_limits()
    }
    pub fn census_visits(&self) -> R<usize> {
        add(
            add(add(self.blocks, self.operations)?, self.expressions)?,
            add(self.components, 1)?,
        )
    }
}

enum Mode<'a> {
    Count,
    Bytes(Vec<u8>),
    Compare { expected: &'a [u8], equal: bool },
}
pub(super) struct Writer<'budget, 'work> {
    mode: Mode<'budget>,
    pub budget: &'budget mut Budget<'work>,
    pub length: usize,
    pub shape: Shape,
}
impl<'budget, 'work> Writer<'budget, 'work> {
    pub fn counter(budget: &'budget mut Budget<'work>) -> Self {
        Self {
            mode: Mode::Count,
            budget,
            length: 0,
            shape: Shape::default(),
        }
    }
    pub fn owned(length: usize, budget: &'budget mut Budget<'work>) -> R<Self> {
        let (bytes, _) = vector(length, budget)?;
        Ok(Self {
            mode: Mode::Bytes(bytes),
            budget,
            length: 0,
            shape: Shape::default(),
        })
    }
    pub fn comparing(expected: &'budget [u8], budget: &'budget mut Budget<'work>) -> Self {
        Self {
            mode: Mode::Compare {
                expected,
                equal: true,
            },
            budget,
            length: 0,
            shape: Shape::default(),
        }
    }
    pub fn bytes(&mut self, bytes: &[u8]) -> R<()> {
        let end = add(self.length, bytes.len())?;
        limit("recipe bytes", end, MAX_RANKED_RECIPE_BYTES_V1)?;
        match &mut self.mode {
            Mode::Count => self.budget.charge_work(1)?,
            Mode::Bytes(output) => {
                self.budget.charge_work(add(bytes.len(), 1)?)?;
                if end > output.capacity() {
                    return Err(Resource::Accounting.into());
                }
                output.extend_from_slice(bytes);
            }
            Mode::Compare { expected, equal } => {
                self.budget.charge_work(add(bytes.len(), 1)?)?;
                *equal &= expected
                    .get(self.length..end)
                    .is_some_and(|actual| actual == bytes);
            }
        }
        self.length = end;
        Ok(())
    }
    pub fn u8(&mut self, value: u8) -> R<()> {
        self.bytes(&[value])
    }
    pub fn u16(&mut self, value: u16) -> R<()> {
        self.bytes(&value.to_le_bytes())
    }
    pub fn u32(&mut self, value: u32) -> R<()> {
        self.bytes(&value.to_le_bytes())
    }
    pub fn u64(&mut self, value: u64) -> R<()> {
        self.bytes(&value.to_le_bytes())
    }
    pub fn boolean(&mut self, value: bool) -> R<()> {
        self.u8(u8::from(value))
    }
    pub fn count(&mut self, value: usize, maximum: usize, field: &'static str) -> R<()> {
        limit(field, value, maximum)?;
        self.u32(u32::try_from(value).map_err(|_| Resource::Arithmetic)?)
    }
    pub fn header(&mut self, kernel: &Kernel, shape: Shape, length: usize) -> R<()> {
        self.bytes(&RANKED_RECIPE_MAGIC_V1)?;
        self.u16(1)?;
        self.u16(0)?;
        self.u32(HEADER as u32)?;
        self.u64(u64::try_from(length).map_err(|_| Resource::Arithmetic)?)?;
        self.count(shape.blocks, MAX_RANKED_BOUNDS_BLOCKS, "blocks")?;
        self.count(shape.operations, MAX_RANKED_BOUNDS_OPERATIONS, "operations")?;
        self.count(
            shape.expressions,
            MAX_RANKED_BOUNDS_OPERATIONS,
            "expressions",
        )?;
        self.count(
            kernel.argument_count,
            HARD_MAX_PRODUCTION_RANKED_ARGUMENTS,
            "arguments",
        )?;
        self.count(
            kernel.function_name.len(),
            crate::HARD_MAX_NAME_BYTES,
            "name",
        )?;
        self.u32(0)
    }
    pub fn into_bytes(self) -> R<Vec<u8>> {
        match self.mode {
            Mode::Bytes(bytes) => Ok(bytes),
            _ => Err(Resource::Accounting.into()),
        }
    }
    pub fn is_equal(&self) -> bool {
        matches!(&self.mode, Mode::Compare { expected, equal: true } if expected.len() == self.length)
    }
    pub fn value(&mut self, value: Value) -> R<()> {
        match value {
            Value::Argument(id) => {
                self.u16(1)?;
                self.u32(id)
            }
            Value::BlockArgument { block, argument } => {
                self.u16(2)?;
                self.u32(block)?;
                self.u32(argument)
            }
            Value::Local(id) => {
                self.u16(3)?;
                self.u32(id.get())
            }
        }
    }
    pub fn values(&mut self, values: &[Value], maximum: usize) -> R<()> {
        self.count(values.len(), maximum, "values")?;
        for value in values {
            self.value(*value)?;
        }
        Ok(())
    }
    pub fn widths(&mut self, values: &[u64], maximum: usize) -> R<()> {
        self.count(values.len(), maximum, "widths")?;
        for value in values {
            self.u64(*value)?;
        }
        Ok(())
    }
    pub fn digest(&mut self, value: DigestV1) -> R<()> {
        self.bytes(value.as_bytes())
    }
}

pub(super) struct Reader<'wire, 'budget, 'work> {
    pub bytes: &'wire [u8],
    pub position: usize,
    pub budget: &'budget mut Budget<'work>,
    pub materialize: bool,
    pub shape: Shape,
    pub heap: usize,
}
impl<'wire, 'budget, 'work> Reader<'wire, 'budget, 'work> {
    pub fn new(bytes: &'wire [u8], materialize: bool, budget: &'budget mut Budget<'work>) -> Self {
        Self {
            bytes,
            position: 0,
            budget,
            materialize,
            shape: Shape::default(),
            heap: 0,
        }
    }
    pub fn take(&mut self, length: usize) -> R<&'wire [u8]> {
        let end = add(self.position, length)?;
        self.budget.charge_work(add(length, 1)?)?;
        let bytes = self.bytes.get(self.position..end).ok_or(E::Length)?;
        self.position = end;
        Ok(bytes)
    }
    pub fn u8(&mut self) -> R<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> R<u16> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().map_err(|_| E::Length)?,
        ))
    }
    pub fn u32(&mut self) -> R<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| E::Length)?,
        ))
    }
    pub fn u64(&mut self) -> R<u64> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| E::Length)?,
        ))
    }
    pub fn boolean(&mut self) -> R<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(E::Tag {
                field: "bool",
                tag: u16::from(tag),
            }),
        }
    }
    pub fn zero(&mut self) -> R<()> {
        if self.u16()? != 0 {
            Err(E::Reserved)
        } else {
            Ok(())
        }
    }
    pub fn count(&mut self, maximum: usize, minimum_bytes: usize, field: &'static str) -> R<usize> {
        let count = usize::try_from(self.u32()?).map_err(|_| Resource::Arithmetic)?;
        limit(field, count, maximum)?;
        if product(count, minimum_bytes)? > self.bytes.len() - self.position {
            return Err(E::Length);
        }
        Ok(count)
    }
    pub fn vector<T>(&mut self, count: usize) -> R<Vec<T>> {
        if !self.materialize {
            return Ok(Vec::new());
        }
        let (values, storage) = vector(count, self.budget)?;
        self.heap = add(self.heap, storage)?;
        Ok(values)
    }
    pub fn value(&mut self) -> R<Value> {
        match self.u16()? {
            1 => Ok(Value::Argument(self.u32()?)),
            2 => Ok(Value::BlockArgument {
                block: self.u32()?,
                argument: self.u32()?,
            }),
            3 => Ok(Value::Local(Id::new(self.u32()?))),
            tag => Err(E::Tag {
                field: "value",
                tag,
            }),
        }
    }
    pub fn values(&mut self, maximum: usize) -> R<Vec<Value>> {
        let count = self.count(maximum, 6, "values")?;
        let mut values = self.vector(count)?;
        for _ in 0..count {
            let value = self.value()?;
            if self.materialize {
                values.push(value);
            }
        }
        Ok(values)
    }
    pub fn widths(&mut self, maximum: usize) -> R<Vec<u64>> {
        let count = self.count(maximum, 8, "widths")?;
        let mut values = self.vector(count)?;
        for _ in 0..count {
            let value = self.u64()?;
            if self.materialize {
                values.push(value);
            }
        }
        Ok(values)
    }
    pub fn digest(&mut self) -> R<DigestV1> {
        Ok(DigestV1::from_untrusted_bytes(
            self.take(32)?.try_into().map_err(|_| E::Length)?,
        ))
    }
    pub fn build<T>(&self, build: impl FnOnce() -> R<T>) -> R<Option<T>> {
        if self.materialize {
            build().map(Some)
        } else {
            Ok(None)
        }
    }
}
