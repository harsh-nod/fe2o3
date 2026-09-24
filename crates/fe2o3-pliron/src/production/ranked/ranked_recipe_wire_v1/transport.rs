use super::*;

pub(super) trait Wire: Sized {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError>;
    fn read<R: Resolver>(input: &mut Decoder<'_, '_, '_, R>)
    -> Result<Self, DecodeError<R::Error>>;
}

pub(super) enum Mode<'a> {
    Count,
    Fill(&'a mut [u8]),
    Compare(&'a [u8]),
}
pub(super) struct Encoder<'out, 'borrow, 'work> {
    mode: Mode<'out>,
    pub offset: usize,
    pub heap: usize,
    pub matches: bool,
    pub budget: &'borrow mut Budget<'work>,
    pub depth: usize,
    pub nodes: usize,
    pub coverage: Option<usize>,
}
impl<'out, 'borrow, 'work> Encoder<'out, 'borrow, 'work> {
    pub fn new(mode: Mode<'out>, budget: &'borrow mut Budget<'work>) -> Self {
        Self {
            mode,
            offset: 0,
            heap: 0,
            matches: true,
            budget,
            depth: 0,
            nodes: 0,
            coverage: None,
        }
    }
    pub fn bytes(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        let end = self
            .offset
            .checked_add(bytes.len())
            .ok_or(Resource::Arithmetic)?;
        if end > MAX_PRODUCTION_RANKED_RECIPE_BYTES_V1 {
            return Err(WireError::Invalid("wire extent"));
        }
        self.budget
            .charge_work(bytes.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
        match &mut self.mode {
            Mode::Count => {}
            Mode::Fill(out) => out
                .get_mut(self.offset..end)
                .ok_or(WireError::Invalid("output extent"))?
                .copy_from_slice(bytes),
            Mode::Compare(expected) => {
                self.matches &= expected.get(self.offset..end) == Some(bytes)
            }
        }
        self.offset = end;
        Ok(())
    }
    pub fn count(&mut self, count: usize, max: usize) -> Result<(), WireError> {
        if count > max {
            return Err(WireError::Invalid("sequence count"));
        }
        u32::try_from(count)
            .map_err(|_| Resource::Arithmetic)?
            .emit(self)
    }
    pub fn retain(&mut self, amount: usize) -> Result<(), WireError> {
        self.budget.charge_work(1)?;
        self.heap = self.heap.checked_add(amount).ok_or(Resource::Arithmetic)?;
        if self.heap > MAX_PRODUCTION_RANKED_RECIPE_STORAGE_V1 {
            return Err(WireError::Invalid("retained payload"));
        }
        if let Some(coverage) = &mut self.coverage {
            if self.heap > *coverage {
                self.budget.reserve_storage(self.heap - *coverage)?;
                *coverage = self.heap;
            }
        }
        Ok(())
    }
    pub fn retain_array<T>(&mut self, count: usize) -> Result<(), WireError> {
        self.retain(
            count
                .checked_mul(size_of::<T>())
                .ok_or(Resource::Arithmetic)?,
        )
    }
    pub fn sequence<T: Wire>(&mut self, values: &Vec<T>, max: usize) -> Result<(), WireError> {
        self.count(values.len(), max)?;
        self.retain_array::<T>(values.capacity())?;
        for value in values {
            value.emit(self)?;
        }
        Ok(())
    }
}

pub(super) struct Decoder<'input, 'borrow, 'work, R> {
    bytes: &'input [u8],
    pub offset: usize,
    pub budget: &'borrow mut Budget<'work>,
    resolver: &'borrow mut R,
    reserved: usize,
    pub block: u32,
    pub operation: u32,
    ordinal: u32,
    pub depth: usize,
    pub nodes: usize,
}
impl<'input, 'borrow, 'work, R: Resolver> Decoder<'input, 'borrow, 'work, R> {
    pub fn new(
        bytes: &'input [u8],
        resolver: &'borrow mut R,
        budget: &'borrow mut Budget<'work>,
    ) -> Self {
        Self {
            bytes,
            offset: 0,
            budget,
            resolver,
            reserved: 0,
            block: 0,
            operation: 0,
            ordinal: 0,
            depth: 0,
            nodes: 0,
        }
    }
    pub fn take(&mut self, length: usize) -> Result<&'input [u8], DecodeError<R::Error>> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(Resource::Arithmetic)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(WireError::Invalid("truncated bytes"))?;
        self.budget
            .charge_work(length.checked_add(1).ok_or(Resource::Arithmetic)?)?;
        self.offset = end;
        Ok(bytes)
    }
    pub fn count(&mut self, max: usize) -> Result<usize, DecodeError<R::Error>> {
        let count = usize::try_from(u32::read(self)?).map_err(|_| Resource::Arithmetic)?;
        if count > max {
            return Err(WireError::Invalid("sequence count").into());
        }
        Ok(count)
    }
    pub fn reserve(&mut self, amount: usize) -> Result<(), DecodeError<R::Error>> {
        let next = self
            .reserved
            .checked_add(amount)
            .ok_or(Resource::Arithmetic)?;
        if next > MAX_PRODUCTION_RANKED_RECIPE_STORAGE_V1 {
            return Err(WireError::Invalid("decoder payload").into());
        }
        self.budget.reserve_storage(amount)?;
        self.reserved = next;
        Ok(())
    }
    pub fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, DecodeError<R::Error>> {
        self.reserve(
            count
                .checked_mul(size_of::<T>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        if values.capacity() != count {
            return Err(Resource::Allocation.into());
        }
        Ok(values)
    }
    pub fn sequence<T: Wire>(&mut self, max: usize) -> Result<Vec<T>, DecodeError<R::Error>> {
        let count = self.count(max)?;
        let mut values = self.vector(count)?;
        for _ in 0..count {
            values.push(T::read(self)?);
        }
        Ok(values)
    }
    pub fn resolve(
        &mut self,
        receipt_digest: DigestV1,
        binding: FunctionalRefinementBindingV2,
    ) -> Result<ProductionReferenceProofV2, DecodeError<R::Error>> {
        self.budget.charge_work(1)?;
        let claim = ProductionRankedRecipeProofClaimV1 {
            ordinal: self.ordinal,
            block: self.block,
            operation: self.operation,
            receipt_digest,
            binding,
        };
        let mut work = ProductionRankedRecipeResolverWorkV1 {
            budget: self.budget,
            denied: None,
        };
        let proof = self
            .resolver
            .resolve(claim, &mut work)
            .map_err(DecodeError::Resolver)?;
        if let Some(error) = work.denied {
            return Err(error.into());
        }
        self.budget.charge_work(1)?;
        if proof.receipt_identity().digest() != receipt_digest || proof.binding() != binding {
            return Err(WireError::Invalid("resolved proof claim").into());
        }
        self.ordinal = self.ordinal.checked_add(1).ok_or(Resource::Arithmetic)?;
        Ok(proof)
    }
    pub fn finish(&mut self) -> Result<(), DecodeError<R::Error>> {
        self.budget.charge_work(1)?;
        let mut work = ProductionRankedRecipeResolverWorkV1 {
            budget: self.budget,
            denied: None,
        };
        self.resolver
            .finish(self.ordinal, &mut work)
            .map_err(DecodeError::Resolver)?;
        if let Some(error) = work.denied {
            return Err(error.into());
        }
        Ok(())
    }
}

macro_rules! integer_wire {
    ($($ty:ty),*) => { $(impl Wire for $ty {
        fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> { out.bytes(&self.to_le_bytes()) }
        fn read<R: Resolver>(input: &mut Decoder<'_, '_, '_, R>) -> Result<Self, DecodeError<R::Error>> {
            Ok(Self::from_le_bytes(input.take(size_of::<Self>())?.try_into().map_err(|_| WireError::Invalid("integer width"))?))
        }
    })* };
}
integer_wire!(u8, u16, u32, u64);

impl Wire for bool {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        u8::from(*self).emit(out)
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        match u8::read(input)? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(WireError::UnknownTag {
                field: "boolean",
                tag,
            }
            .into()),
        }
    }
}

impl<const N: usize> Wire for [u64; N] {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        for value in self {
            value.emit(out)?;
        }
        Ok(())
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        let mut values = [0; N];
        for value in &mut values {
            *value = u64::read(input)?;
        }
        Ok(values)
    }
}

impl<T: Wire> Wire for Vec<T> {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        out.sequence(self, MAX_RANKED_BOUNDS_OPERATIONS)
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        input.sequence(MAX_RANKED_BOUNDS_OPERATIONS)
    }
}

impl<T: Wire> Wire for Option<T> {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        self.is_some().emit(out)?;
        if let Some(value) = self {
            value.emit(out)?;
        }
        Ok(())
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        if bool::read(input)? {
            Ok(Some(T::read(input)?))
        } else {
            Ok(None)
        }
    }
}

impl Wire for String {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        out.count(self.len(), crate::HARD_MAX_NAME_BYTES)?;
        out.retain(self.capacity())?;
        out.bytes(self.as_bytes())
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        let count = input.count(crate::HARD_MAX_NAME_BYTES)?;
        let bytes = input.take(count)?;
        input
            .budget
            .charge_work(count.checked_mul(2).ok_or(Resource::Arithmetic)?)?;
        let text = std::str::from_utf8(bytes).map_err(|_| WireError::Invalid("UTF-8 name"))?;
        input.reserve(count)?;
        let mut value = String::new();
        value
            .try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        if value.capacity() != count {
            return Err(Resource::Allocation.into());
        }
        value.push_str(text);
        Ok(value)
    }
}

impl Wire for DigestV1 {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        out.bytes(self.as_bytes())
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        Ok(Self::from_untrusted_bytes(
            input
                .take(32)?
                .try_into()
                .map_err(|_| WireError::Invalid("digest width"))?,
        ))
    }
}

macro_rules! enum_wire {
    ($ty:ty; $($tag:literal => $variant:ident $( { $($field:ident: $field_ty:ty $(=> $limit:expr)?),* $(,)? } )? ),* $(,)?) => {
        impl Wire for $ty {
            fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
                match self { $(Self::$variant $({$($field),*})? => {
                    ($tag as u8).emit(out)?;
                    $($(emit_field!(out, $field $(, $limit)?);)*)?
                }),* }
                Ok(())
            }
            fn read<R: Resolver>(input: &mut Decoder<'_, '_, '_, R>) -> Result<Self, DecodeError<R::Error>> {
                Ok(match u8::read(input)? {
                    $($tag => Self::$variant $({$($field: read_field!(input, $field_ty $(, $limit)?)),*})?),*,
                    tag => return Err(WireError::UnknownTag {field: stringify!($ty), tag}.into()),
                })
            }
        }
    };
}
pub(super) use enum_wire;

macro_rules! emit_field {
    ($out:expr, $value:expr) => {
        Wire::emit($value, $out)?
    };
    ($out:expr, $value:expr, $max:expr) => {
        $out.sequence($value, $max)?
    };
}
macro_rules! read_field {
    ($input:expr, $ty:ty) => {
        <$ty>::read($input)?
    };
    ($input:expr, $ty:ty, $max:expr) => {
        $input.sequence($max)?
    };
}
pub(super) use {emit_field, read_field};

macro_rules! contract_wire {
    ($ty:ty; $($field:ident: $field_ty:ty $(=> $limit:expr)?),* $(,)?) => {
        impl Wire for $ty {
            fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
                $(emit_field!(out, &self.$field $(, $limit)?);)*
                Ok(())
            }
            fn read<R: Resolver>(input: &mut Decoder<'_, '_, '_, R>) -> Result<Self, DecodeError<R::Error>> {
                Self::new($(read_field!(input, $field_ty $(, $limit)?)),*).map_err(|e| WireError::Kernel(e).into())
            }
        }
    };
}
pub(super) use contract_wire;
