//! Entry argument correspondence borrowed from admitted Semantic MIR.
//! This describes ABI identity, not current SSA provenance or borrow authority.

use crate::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticAbiArgumentRoleV1, SemanticAbiArgumentV1,
    SemanticExternAbiV1, SemanticFunctionDeclV1, SemanticFunctionIdV1, SemanticLocalIdV1,
    SemanticLocalRoleV1, SemanticSourceArgumentOwnershipV1, SemanticTypeIdV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticLogicalArgumentErrorV1 {
    UnknownFunction,
    AllocationFailure,
}

impl std::fmt::Display for SemanticLogicalArgumentErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownFunction => "logical argument function is not in the admitted module",
            Self::AllocationFailure => "logical argument storage cannot be reserved",
        })
    }
}
impl std::error::Error for SemanticLogicalArgumentErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticSourceArgumentBindingV1<'a> {
    Whole(SemanticLocalIdV1),
    /// An expanded zero-field tuple is still a source argument.
    ExpandedTuple(&'a [SemanticLocalIdV1]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticSourceArgumentV1<'a> {
    ordinal: u32,
    ty: SemanticTypeIdV1,
    source_ownership: SemanticSourceArgumentOwnershipV1,
    binding: SemanticSourceArgumentBindingV1<'a>,
}

impl<'a> SemanticSourceArgumentV1<'a> {
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
    pub const fn ty(self) -> SemanticTypeIdV1 {
        self.ty
    }
    /// Ownership of the whole source argument, not of a tuple field's pointee.
    pub const fn source_ownership(self) -> SemanticSourceArgumentOwnershipV1 {
        self.source_ownership
    }
    pub const fn binding(self) -> SemanticSourceArgumentBindingV1<'a> {
        self.binding
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SemanticAdjustedArgumentV1<'a> {
    ordinal: u32,
    source_argument: u32,
    tuple_field: Option<u32>,
    source_ownership: SemanticSourceArgumentOwnershipV1,
    abi: &'a SemanticAbiArgumentV1,
    local: SemanticLocalIdV1,
    local_field: Option<u32>,
}

impl<'a> SemanticAdjustedArgumentV1<'a> {
    /// ABI ordinal, including ignored arguments; not an emitted GPU parameter.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
    pub const fn source_argument(self) -> u32 {
        self.source_argument
    }
    pub const fn tuple_field(self) -> Option<u32> {
        self.tuple_field
    }
    pub const fn source_ownership(self) -> SemanticSourceArgumentOwnershipV1 {
        self.source_ownership
    }
    pub const fn abi(self) -> &'a SemanticAbiArgumentV1 {
        self.abi
    }
    pub const fn local(self) -> SemanticLocalIdV1 {
        self.local
    }
    /// The outer tuple field of a packed entry local, with result type
    /// `abi().ty()`. Expanded tuple-field locals have no projection.
    pub const fn local_field(self) -> Option<u32> {
        self.local_field
    }
}

#[derive(Debug)]
pub struct SemanticLogicalArgumentMapV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    source_locals: Vec<Option<SemanticLocalIdV1>>,
    expanded_fields: Vec<SemanticLocalIdV1>,
}

impl AdmittedInertSemanticMirV1 {
    /// Indexes canonical entry bindings in O(locals + arguments) time and
    /// O(arguments) storage. Canonical admission already checked role/type
    /// uniqueness and RustCall expansion. No wire data or authority is added.
    pub fn logical_arguments_v1(
        &self,
        function: SemanticFunctionIdV1,
    ) -> Result<SemanticLogicalArgumentMapV1<'_>, SemanticLogicalArgumentErrorV1> {
        let function = self
            .functions()
            .get(function.index() as usize)
            .ok_or(SemanticLogicalArgumentErrorV1::UnknownFunction)?;
        let abi = function.abi();
        let mut source_locals = Vec::new();
        source_locals
            .try_reserve_exact(abi.source_input_types().len())
            .map_err(|_| SemanticLogicalArgumentErrorV1::AllocationFailure)?;
        source_locals.resize(abi.source_input_types().len(), None);
        for (index, local) in function.locals().iter().enumerate() {
            if let SemanticLocalRoleV1::Argument(ordinal) = local.role() {
                source_locals[ordinal as usize] = Some(SemanticLocalIdV1::from_index(index as u32));
            }
        }
        let mut expanded_fields = Vec::new();
        if abi.extern_abi() == SemanticExternAbiV1::RustCall
            && source_locals[abi.fixed_count() as usize].is_none()
        {
            let count = abi.adjusted_arguments().len() - abi.fixed_count() as usize;
            expanded_fields
                .try_reserve_exact(count)
                .map_err(|_| SemanticLogicalArgumentErrorV1::AllocationFailure)?;
            // Admission guarantees every expanded field is assigned below.
            expanded_fields.resize(count, SemanticLocalIdV1::from_index(0));
            for (index, local) in function.locals().iter().enumerate() {
                if let SemanticLocalRoleV1::RustCallTupleField { field, .. } = local.role() {
                    expanded_fields[field as usize] = SemanticLocalIdV1::from_index(index as u32);
                }
            }
        }
        Ok(SemanticLogicalArgumentMapV1 {
            function,
            source_locals,
            expanded_fields,
        })
    }
}

impl SemanticLogicalArgumentMapV1<'_> {
    pub fn source_arguments(&self) -> impl ExactSizeIterator<Item = SemanticSourceArgumentV1<'_>> {
        let abi = self.function.abi();
        abi.source_input_types()
            .iter()
            .enumerate()
            .map(|(ordinal, &ty)| SemanticSourceArgumentV1 {
                ordinal: ordinal as u32,
                ty,
                source_ownership: abi.source_argument_ownership()[ordinal],
                binding: match self.source_locals[ordinal] {
                    Some(local) => SemanticSourceArgumentBindingV1::Whole(local),
                    None => SemanticSourceArgumentBindingV1::ExpandedTuple(&self.expanded_fields),
                },
            })
    }

    /// Only the outer RustCall tuple is expanded. Hidden arguments stay in the
    /// original FnAbi; pass modes, adjustments and attributes remain unchanged.
    pub fn adjusted_arguments(
        &self,
    ) -> impl ExactSizeIterator<Item = SemanticAdjustedArgumentV1<'_>> {
        let abi = self.function.abi();
        abi.adjusted_arguments()
            .iter()
            .enumerate()
            .map(|(ordinal, argument)| {
                let (source_argument, tuple_field) = match argument.role() {
                    SemanticAbiArgumentRoleV1::Source => (ordinal as u32, None),
                    SemanticAbiArgumentRoleV1::RustCallTupleField(field) => {
                        (abi.fixed_count(), Some(field))
                    }
                    SemanticAbiArgumentRoleV1::Hidden(_) => {
                        unreachable!("adjusted_arguments excludes hidden arguments")
                    }
                };
                let (local, local_field) = match self.source_locals[source_argument as usize] {
                    Some(local) => (local, tuple_field),
                    None => (
                        self.expanded_fields
                            [tuple_field.expect("admitted expanded tuple") as usize],
                        None,
                    ),
                };
                SemanticAdjustedArgumentV1 {
                    ordinal: ordinal as u32,
                    source_argument,
                    tuple_field,
                    source_ownership: abi.source_argument_ownership()[source_argument as usize],
                    abi: argument,
                    local,
                    local_field,
                }
            })
    }
}
