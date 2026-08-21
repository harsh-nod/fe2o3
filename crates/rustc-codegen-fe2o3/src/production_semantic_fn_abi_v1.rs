//! Pure conversion from an already-observed rustc `FnAbi` into semantic MIR.
//!
//! This module does not query rustc or any qualification authority. Its closed
//! producer API requires callers to supply every source-signature, identity,
//! role, and adjusted-layout binding that is not present in `FnAbi` itself.

#![allow(dead_code)] // The importer wiring intentionally lands in a later change.

use std::collections::BTreeMap;
use std::fmt;

use fe2o3_mir_model::semantic_mir_v1::{
    HARD_MAX_CALL_ARGUMENTS_V1, HARD_MAX_FUNCTIONS_V1, SemanticAbiAdjustedTypeV1,
    SemanticAbiArgumentV1, SemanticAbiCastV1, SemanticAbiExtensionV1,
    SemanticAbiHiddenArgumentRoleV1, SemanticAbiIdentityV1, SemanticAbiPassModeV1,
    SemanticAbiPointeeInfoV1, SemanticAbiRegisterKindV1, SemanticAbiRegisterV1,
    SemanticAbiRegularAttributesV1, SemanticAbiUniformV1, SemanticAbiValueAttributesV1,
    SemanticAbiValueV1, SemanticArmCallV1, SemanticCanonAbiV1, SemanticExternAbiV1,
    SemanticFunctionAbiV1, SemanticInterruptKindV1, SemanticLayoutIdentityV1, SemanticMirErrorV1,
    SemanticTypeIdV1, SemanticTypeLayoutV1, SemanticX86CallV1,
};
use rustc_abi::{ArmCall, CanonAbi, ExternAbi, InterruptKind, Reg, RegKind, X86Call};
use rustc_middle::ty::layout::TyAndLayout;
use rustc_middle::ty::{Ty, TyKind};
use rustc_target::callconv::{
    ArgAbi, ArgAttributes, ArgExtension, CastTarget, FnAbi, PassMode, Uniform,
};

use crate::rustc_semantic_adapter_v1::rustc_type_identity_v1;
use crate::rustc_semantic_plan_v1::{
    RetainedSemanticFunctionAbiProducerV1, RetainedSemanticTypeProducerV1,
};

#[derive(Debug)]
pub(crate) enum ProductionSemanticFnAbiErrorV1 {
    LimitExceeded {
        component: &'static str,
        actual: u64,
        maximum: u64,
    },
    UnsupportedExternAbi(ExternAbi),
    ProducerMismatch {
        component: &'static str,
        argument: Option<u32>,
    },
    MissingTypeProducer,
    UnsupportedAdjustedLayout,
    Allocation,
    Schema(SemanticMirErrorV1),
}

impl fmt::Display for ProductionSemanticFnAbiErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                component,
                actual,
                maximum,
            } => write!(
                formatter,
                "semantic FnAbi producer rejected {component} count {actual}; maximum is {maximum}",
            ),
            Self::UnsupportedExternAbi(abi) => write!(
                formatter,
                "semantic FnAbi producer cannot represent source ABI {abi}",
            ),
            Self::ProducerMismatch {
                component,
                argument: Some(argument),
            } => write!(
                formatter,
                "semantic FnAbi producer mismatch for {component} at adjusted argument {argument}",
            ),
            Self::ProducerMismatch {
                component,
                argument: None,
            } => write!(
                formatter,
                "semantic FnAbi producer mismatch for {component}"
            ),
            Self::MissingTypeProducer => {
                formatter.write_str("semantic FnAbi references an absent canonical type producer")
            }
            Self::UnsupportedAdjustedLayout => formatter.write_str(
                "semantic FnAbi uses an adjusted rustc layout outside the first production subset",
            ),
            Self::Allocation => formatter
                .write_str("semantic FnAbi construction could not allocate bounded records"),
            Self::Schema(error) => write!(formatter, "semantic FnAbi schema rejection: {error}"),
        }
    }
}

impl std::error::Error for ProductionSemanticFnAbiErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Schema(error) => Some(error),
            Self::LimitExceeded { .. }
            | Self::UnsupportedExternAbi(_)
            | Self::ProducerMismatch { .. }
            | Self::MissingTypeProducer
            | Self::UnsupportedAdjustedLayout
            | Self::Allocation => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ConstructedSemanticFunctionAbisV1 {
    records: Box<[SemanticFunctionAbiV1]>,
}

impl ConstructedSemanticFunctionAbisV1 {
    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }
}

pub(crate) fn construct_production_semantic_fn_abis_v1<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    producers: &[RetainedSemanticFunctionAbiProducerV1<'tcx>],
    types: &[RetainedSemanticTypeProducerV1<'tcx>],
) -> Result<ConstructedSemanticFunctionAbisV1, ProductionSemanticFnAbiErrorV1> {
    require_limit_v1("functions", producers.len(), HARD_MAX_FUNCTIONS_V1)?;
    let mut type_bindings = BTreeMap::new();
    for (index, producer) in types.iter().enumerate() {
        let index = u32::try_from(index)
            .map_err(|_| ProductionSemanticFnAbiErrorV1::MissingTypeProducer)?;
        if type_bindings
            .insert(
                producer.identity,
                (producer, SemanticTypeIdV1::from_index(index)),
            )
            .is_some()
        {
            return Err(ProductionSemanticFnAbiErrorV1::MissingTypeProducer);
        }
    }

    let mut records = Vec::new();
    records
        .try_reserve_exact(producers.len())
        .map_err(|_| ProductionSemanticFnAbiErrorV1::Allocation)?;
    for (index, producer) in producers.iter().enumerate() {
        if usize::try_from(producer.function.index()).ok() != Some(index) {
            return Err(mismatch_v1("function ordering", None));
        }
        let source_inputs = producer
            .source_inputs
            .iter()
            .map(|ty| type_binding_v1(tcx, *ty, &type_bindings))
            .collect::<Result<Vec<_>, _>>()?;
        let source_output = type_binding_v1(tcx, producer.source_output, &type_bindings)?;
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(producer.fn_abi.args.len())
            .map_err(|_| ProductionSemanticFnAbiErrorV1::Allocation)?;

        let rust_call = producer.extern_abi == ExternAbi::RustCall;
        let fixed_source_count = source_inputs
            .len()
            .checked_sub(usize::from(rust_call))
            .ok_or_else(|| mismatch_v1("RustCall source tuple", None))?;
        for (source, rustc_argument) in source_inputs
            .iter()
            .take(fixed_source_count)
            .zip(producer.fn_abi.args.iter())
        {
            arguments.push(ProductionSemanticFnAbiArgumentProducerV1::source(
                exact_value_binding_v1(*source, rustc_argument)?,
            ));
        }
        let mut adjusted_index = fixed_source_count;
        if rust_call {
            let tuple = *producer
                .source_inputs
                .last()
                .ok_or_else(|| mismatch_v1("RustCall source tuple", None))?;
            let TyKind::Tuple(fields) = tuple.kind() else {
                return Err(mismatch_v1("RustCall source tuple", None));
            };
            for (field_index, field) in fields.iter().enumerate() {
                let rustc_argument = producer
                    .fn_abi
                    .args
                    .get(adjusted_index)
                    .ok_or_else(|| mismatch_v1("RustCall adjusted arguments", None))?;
                let binding = type_binding_v1(tcx, field, &type_bindings)?;
                arguments.push(
                    ProductionSemanticFnAbiArgumentProducerV1::rust_call_tuple_field(
                        u32::try_from(field_index)
                            .map_err(|_| mismatch_v1("RustCall tuple field", None))?,
                        exact_value_binding_v1(binding, rustc_argument)?,
                    ),
                );
                adjusted_index += 1;
            }
        }
        if adjusted_index < producer.fn_abi.args.len() {
            if producer.extern_abi != ExternAbi::Rust
                || adjusted_index + 1 != producer.fn_abi.args.len()
            {
                return Err(mismatch_v1("hidden adjusted arguments", None));
            }
            let rustc_argument = &producer.fn_abi.args[adjusted_index];
            let binding = type_binding_v1(tcx, rustc_argument.layout.ty, &type_bindings)?;
            arguments.push(
                ProductionSemanticFnAbiArgumentProducerV1::hidden_caller_location(
                    exact_value_binding_v1(binding, rustc_argument)?,
                ),
            );
        }
        if arguments.len() != producer.fn_abi.args.len() {
            return Err(mismatch_v1("adjusted argument cardinality", None));
        }

        let return_value = exact_value_binding_v1(source_output, &producer.fn_abi.ret)?;
        records.push(construct_production_semantic_fn_abi_v1(
            ProductionSemanticFnAbiV1Producer::new(
                producer.identity,
                producer.layout_identity,
                producer.extern_abi,
                producer.fn_abi,
                source_inputs,
                source_output,
                arguments,
                return_value,
            )?,
        )?);
    }
    Ok(ConstructedSemanticFunctionAbisV1 {
        records: records.into_boxed_slice(),
    })
}

fn type_binding_v1<'a, 'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    bindings: &'a BTreeMap<
        fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1,
        (&'a RetainedSemanticTypeProducerV1<'tcx>, SemanticTypeIdV1),
    >,
) -> Result<ProductionSemanticFnAbiTypeProducerV1<'tcx>, ProductionSemanticFnAbiErrorV1> {
    let identity = rustc_type_identity_v1(tcx, ty);
    let (producer, semantic_type) = bindings
        .get(&identity)
        .copied()
        .ok_or(ProductionSemanticFnAbiErrorV1::MissingTypeProducer)?;
    if producer.ty != ty {
        return Err(ProductionSemanticFnAbiErrorV1::MissingTypeProducer);
    }
    Ok(ProductionSemanticFnAbiTypeProducerV1::new(
        producer.layout,
        semantic_type,
    ))
}

fn exact_value_binding_v1<'tcx>(
    source: ProductionSemanticFnAbiTypeProducerV1<'tcx>,
    rustc_value: &ArgAbi<'tcx, Ty<'tcx>>,
) -> Result<ProductionSemanticFnAbiValueProducerV1<'tcx>, ProductionSemanticFnAbiErrorV1> {
    if source.rustc_layout != rustc_value.layout {
        return Err(ProductionSemanticFnAbiErrorV1::UnsupportedAdjustedLayout);
    }
    Ok(ProductionSemanticFnAbiValueProducerV1::new(source))
}

impl From<SemanticMirErrorV1> for ProductionSemanticFnAbiErrorV1 {
    fn from(error: SemanticMirErrorV1) -> Self {
        Self::Schema(error)
    }
}

/// Exact correspondence between one canonical rustc type layout and its
/// request-local semantic type ID.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionSemanticFnAbiTypeProducerV1<'tcx> {
    rustc_layout: TyAndLayout<'tcx>,
    semantic_type: SemanticTypeIdV1,
}

impl<'tcx> ProductionSemanticFnAbiTypeProducerV1<'tcx> {
    pub(crate) const fn new(
        rustc_layout: TyAndLayout<'tcx>,
        semantic_type: SemanticTypeIdV1,
    ) -> Self {
        Self {
            rustc_layout,
            semantic_type,
        }
    }
}

/// Semantic layout record for a rustc ABI-only layout adjustment, such as a
/// virtual receiver made thin by `fn_abi_of_instance`.
#[derive(Clone, Debug)]
pub(crate) struct ProductionSemanticFnAbiAdjustedLayoutProducerV1<'tcx> {
    rustc_layout: TyAndLayout<'tcx>,
    layout_identity: SemanticLayoutIdentityV1,
    layout: SemanticTypeLayoutV1,
}

impl<'tcx> ProductionSemanticFnAbiAdjustedLayoutProducerV1<'tcx> {
    pub(crate) const fn new(
        rustc_layout: TyAndLayout<'tcx>,
        layout_identity: SemanticLayoutIdentityV1,
        layout: SemanticTypeLayoutV1,
    ) -> Self {
        Self {
            rustc_layout,
            layout_identity,
            layout,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProductionSemanticFnAbiValueProducerV1<'tcx> {
    source: ProductionSemanticFnAbiTypeProducerV1<'tcx>,
    adjusted: Option<ProductionSemanticFnAbiAdjustedLayoutProducerV1<'tcx>>,
    pointee_override: Option<SemanticAbiPointeeInfoV1>,
}

impl<'tcx> ProductionSemanticFnAbiValueProducerV1<'tcx> {
    pub(crate) const fn new(source: ProductionSemanticFnAbiTypeProducerV1<'tcx>) -> Self {
        Self {
            source,
            adjusted: None,
            pointee_override: None,
        }
    }

    pub(crate) fn with_adjusted_layout(
        mut self,
        adjusted: ProductionSemanticFnAbiAdjustedLayoutProducerV1<'tcx>,
    ) -> Self {
        self.adjusted = Some(adjusted);
        self
    }

    pub(crate) const fn with_pointee_override(
        mut self,
        pointee_override: SemanticAbiPointeeInfoV1,
    ) -> Self {
        self.pointee_override = Some(pointee_override);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProductionSemanticFnAbiArgumentRoleV1 {
    Source,
    RustCallTupleField(u32),
    HiddenCallerLocation,
}

#[derive(Clone, Debug)]
pub(crate) struct ProductionSemanticFnAbiArgumentProducerV1<'tcx> {
    role: ProductionSemanticFnAbiArgumentRoleV1,
    value: ProductionSemanticFnAbiValueProducerV1<'tcx>,
}

impl<'tcx> ProductionSemanticFnAbiArgumentProducerV1<'tcx> {
    pub(crate) const fn source(value: ProductionSemanticFnAbiValueProducerV1<'tcx>) -> Self {
        Self {
            role: ProductionSemanticFnAbiArgumentRoleV1::Source,
            value,
        }
    }

    pub(crate) const fn rust_call_tuple_field(
        field: u32,
        value: ProductionSemanticFnAbiValueProducerV1<'tcx>,
    ) -> Self {
        Self {
            role: ProductionSemanticFnAbiArgumentRoleV1::RustCallTupleField(field),
            value,
        }
    }

    pub(crate) const fn hidden_caller_location(
        value: ProductionSemanticFnAbiValueProducerV1<'tcx>,
    ) -> Self {
        Self {
            role: ProductionSemanticFnAbiArgumentRoleV1::HiddenCallerLocation,
            value,
        }
    }
}

/// Closed input to the pure converter. Construction validates every axis that
/// must be supplied alongside rustc's adjusted `FnAbi`.
#[derive(Debug)]
pub(crate) struct ProductionSemanticFnAbiV1Producer<'abi, 'tcx> {
    identity: SemanticAbiIdentityV1,
    layout_identity: SemanticLayoutIdentityV1,
    extern_abi: ExternAbi,
    fn_abi: &'abi FnAbi<'tcx, Ty<'tcx>>,
    source_inputs: Box<[ProductionSemanticFnAbiTypeProducerV1<'tcx>]>,
    source_output: ProductionSemanticFnAbiTypeProducerV1<'tcx>,
    arguments: Box<[ProductionSemanticFnAbiArgumentProducerV1<'tcx>]>,
    return_value: ProductionSemanticFnAbiValueProducerV1<'tcx>,
}

impl<'abi, 'tcx> ProductionSemanticFnAbiV1Producer<'abi, 'tcx> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        identity: SemanticAbiIdentityV1,
        layout_identity: SemanticLayoutIdentityV1,
        extern_abi: ExternAbi,
        fn_abi: &'abi FnAbi<'tcx, Ty<'tcx>>,
        source_inputs: Vec<ProductionSemanticFnAbiTypeProducerV1<'tcx>>,
        source_output: ProductionSemanticFnAbiTypeProducerV1<'tcx>,
        arguments: Vec<ProductionSemanticFnAbiArgumentProducerV1<'tcx>>,
        return_value: ProductionSemanticFnAbiValueProducerV1<'tcx>,
    ) -> Result<Self, ProductionSemanticFnAbiErrorV1> {
        require_count_v1("source inputs", source_inputs.len())?;
        require_count_v1("adjusted arguments", arguments.len())?;
        require_count_v1("rustc adjusted arguments", fn_abi.args.len())?;
        require_count_v1(
            "rustc fixed arguments",
            usize::try_from(fn_abi.fixed_count).map_err(|_| mismatch_v1("fixed count", None))?,
        )?;
        let producer = Self {
            identity,
            layout_identity,
            extern_abi,
            fn_abi,
            source_inputs: source_inputs.into_boxed_slice(),
            source_output,
            arguments: arguments.into_boxed_slice(),
            return_value,
        };
        validate_producer_v1(&producer)?;
        Ok(producer)
    }
}

pub(crate) fn construct_production_semantic_fn_abi_v1(
    producer: ProductionSemanticFnAbiV1Producer<'_, '_>,
) -> Result<SemanticFunctionAbiV1, ProductionSemanticFnAbiErrorV1> {
    validate_producer_v1(&producer)?;

    let extern_abi = convert_extern_abi_v1(producer.extern_abi)?;
    let canon_abi = convert_canon_abi_v1(producer.fn_abi.conv);
    let source_input_types = producer
        .source_inputs
        .iter()
        .map(|input| input.semantic_type)
        .collect();
    let mut arguments = Vec::with_capacity(producer.arguments.len());
    for (rustc_argument, argument) in producer.fn_abi.args.iter().zip(producer.arguments) {
        let value = convert_value_v1(rustc_argument, argument.value)?;
        arguments.push(match argument.role {
            ProductionSemanticFnAbiArgumentRoleV1::Source => SemanticAbiArgumentV1::source(value),
            ProductionSemanticFnAbiArgumentRoleV1::RustCallTupleField(field) => {
                SemanticAbiArgumentV1::rust_call_tuple_field(field, value)
            }
            ProductionSemanticFnAbiArgumentRoleV1::HiddenCallerLocation => {
                SemanticAbiArgumentV1::hidden(
                    SemanticAbiHiddenArgumentRoleV1::CallerLocation,
                    value,
                )
            }
        });
    }
    let return_value = convert_value_v1(&producer.fn_abi.ret, producer.return_value)?;

    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        producer.identity,
        producer.layout_identity,
        canon_abi,
        extern_abi,
        producer.fn_abi.can_unwind,
        producer.fn_abi.c_variadic,
        producer.fn_abi.fixed_count,
        source_input_types,
        producer.source_output.semantic_type,
        arguments,
        return_value,
    )
    .map_err(Into::into)
}

fn validate_producer_v1(
    producer: &ProductionSemanticFnAbiV1Producer<'_, '_>,
) -> Result<(), ProductionSemanticFnAbiErrorV1> {
    require_count_v1("source inputs", producer.source_inputs.len())?;
    require_count_v1("adjusted arguments", producer.arguments.len())?;
    require_count_v1("rustc adjusted arguments", producer.fn_abi.args.len())?;
    require_count_v1(
        "rustc fixed arguments",
        usize::try_from(producer.fn_abi.fixed_count)
            .map_err(|_| mismatch_v1("fixed count", None))?,
    )?;
    let extern_abi = convert_extern_abi_v1(producer.extern_abi)?;
    if expected_canon_abi_v1(extern_abi) != convert_canon_abi_v1(producer.fn_abi.conv) {
        return Err(mismatch_v1("source and canonical ABI", None));
    }
    if producer.fn_abi.c_variadic
        && !matches!(
            extern_abi,
            SemanticExternAbiV1::C { .. }
                | SemanticExternAbiV1::Cdecl { .. }
                | SemanticExternAbiV1::System { .. }
        )
    {
        return Err(mismatch_v1("variadic source ABI", None));
    }

    if producer.fn_abi.args.len() != producer.arguments.len() {
        return Err(mismatch_v1("adjusted argument cardinality", None));
    }

    let rust_call = producer.extern_abi == ExternAbi::RustCall;
    let fixed_source_count = if rust_call {
        producer
            .source_inputs
            .len()
            .checked_sub(1)
            .ok_or_else(|| mismatch_v1("RustCall source tuple", None))?
    } else {
        producer.source_inputs.len()
    };
    if usize::try_from(producer.fn_abi.fixed_count).ok() != Some(fixed_source_count) {
        return Err(mismatch_v1("fixed source argument count", None));
    }

    let rust_call_fields = if rust_call {
        match producer
            .source_inputs
            .last()
            .expect("checked nonempty RustCall signature")
            .rustc_layout
            .ty
            .kind()
        {
            TyKind::Tuple(fields) => Some(*fields),
            _ => return Err(mismatch_v1("RustCall source tuple", None)),
        }
    } else {
        None
    };

    let mut source_index = 0_usize;
    let mut tuple_field_index = 0_usize;
    let mut saw_hidden = false;
    for (index, (rustc_argument, argument)) in producer
        .fn_abi
        .args
        .iter()
        .zip(producer.arguments.iter())
        .enumerate()
    {
        let argument_index = u32::try_from(index).expect("bounded adjusted argument index");
        validate_value_producer_v1(rustc_argument, &argument.value, Some(argument_index))?;
        match argument.role {
            ProductionSemanticFnAbiArgumentRoleV1::Source => {
                if source_index >= fixed_source_count
                    || tuple_field_index != 0
                    || saw_hidden
                    || !same_type_binding_v1(
                        &producer.source_inputs[source_index],
                        &argument.value.source,
                    )
                {
                    return Err(mismatch_v1("source argument role", Some(argument_index)));
                }
                source_index += 1;
            }
            ProductionSemanticFnAbiArgumentRoleV1::RustCallTupleField(field) => {
                let Some(fields) = rust_call_fields else {
                    return Err(mismatch_v1(
                        "RustCall tuple field role",
                        Some(argument_index),
                    ));
                };
                if source_index != fixed_source_count
                    || saw_hidden
                    || usize::try_from(field).ok() != Some(tuple_field_index)
                    || fields.get(tuple_field_index).copied()
                        != Some(argument.value.source.rustc_layout.ty)
                {
                    return Err(mismatch_v1(
                        "RustCall tuple field role",
                        Some(argument_index),
                    ));
                }
                tuple_field_index += 1;
            }
            ProductionSemanticFnAbiArgumentRoleV1::HiddenCallerLocation => {
                if producer.extern_abi != ExternAbi::Rust
                    || source_index != fixed_source_count
                    || tuple_field_index != 0
                    || saw_hidden
                {
                    return Err(mismatch_v1(
                        "hidden caller location role",
                        Some(argument_index),
                    ));
                }
                saw_hidden = true;
            }
        }
    }

    if source_index != fixed_source_count
        || rust_call_fields.is_some_and(|fields| tuple_field_index != fields.len())
    {
        return Err(mismatch_v1("source signature role closure", None));
    }
    if !same_type_binding_v1(&producer.source_output, &producer.return_value.source) {
        return Err(mismatch_v1("source return type", None));
    }
    validate_value_producer_v1(&producer.fn_abi.ret, &producer.return_value, None)
}

fn validate_value_producer_v1<'tcx>(
    rustc_value: &ArgAbi<'tcx, Ty<'tcx>>,
    producer: &ProductionSemanticFnAbiValueProducerV1<'tcx>,
    argument: Option<u32>,
) -> Result<(), ProductionSemanticFnAbiErrorV1> {
    if rustc_value.layout.ty != producer.source.rustc_layout.ty {
        return Err(mismatch_v1("rustc source type", argument));
    }
    match &producer.adjusted {
        None if rustc_value.layout == producer.source.rustc_layout => Ok(()),
        Some(adjusted)
            if rustc_value.layout != producer.source.rustc_layout
                && rustc_value.layout == adjusted.rustc_layout =>
        {
            Ok(())
        }
        None => Err(mismatch_v1("missing adjusted layout", argument)),
        Some(_) => Err(mismatch_v1("substituted adjusted layout", argument)),
    }
}

fn same_type_binding_v1<'tcx>(
    left: &ProductionSemanticFnAbiTypeProducerV1<'tcx>,
    right: &ProductionSemanticFnAbiTypeProducerV1<'tcx>,
) -> bool {
    left.semantic_type == right.semantic_type && left.rustc_layout == right.rustc_layout
}

fn require_count_v1(
    component: &'static str,
    actual: usize,
) -> Result<(), ProductionSemanticFnAbiErrorV1> {
    require_limit_v1(component, actual, HARD_MAX_CALL_ARGUMENTS_V1)
}

fn require_limit_v1(
    component: &'static str,
    actual: usize,
    maximum: u64,
) -> Result<(), ProductionSemanticFnAbiErrorV1> {
    let actual = u64::try_from(actual).unwrap_or(u64::MAX);
    if actual > maximum {
        Err(ProductionSemanticFnAbiErrorV1::LimitExceeded {
            component,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

const fn mismatch_v1(
    component: &'static str,
    argument: Option<u32>,
) -> ProductionSemanticFnAbiErrorV1 {
    ProductionSemanticFnAbiErrorV1::ProducerMismatch {
        component,
        argument,
    }
}

fn convert_value_v1<'tcx>(
    rustc_value: &ArgAbi<'tcx, Ty<'tcx>>,
    producer: ProductionSemanticFnAbiValueProducerV1<'tcx>,
) -> Result<SemanticAbiValueV1, ProductionSemanticFnAbiErrorV1> {
    let mode = convert_pass_mode_v1(&rustc_value.mode)?;
    let mut value = if let Some(adjusted) = producer.adjusted {
        SemanticAbiValueV1::new_with_adjusted_type(
            producer.source.semantic_type,
            SemanticAbiAdjustedTypeV1::new(
                producer.source.semantic_type,
                adjusted.layout_identity,
                adjusted.layout,
            ),
            mode,
        )
    } else {
        SemanticAbiValueV1::new(producer.source.semantic_type, mode)
    };
    if let Some(pointee_override) = producer.pointee_override {
        value = value.with_pointee_override(pointee_override);
    }
    Ok(value)
}

fn convert_extern_abi_v1(
    abi: ExternAbi,
) -> Result<SemanticExternAbiV1, ProductionSemanticFnAbiErrorV1> {
    match abi {
        ExternAbi::C { unwind } => Ok(SemanticExternAbiV1::C { unwind }),
        ExternAbi::Cdecl { unwind } => Ok(SemanticExternAbiV1::Cdecl { unwind }),
        ExternAbi::System { unwind } => Ok(SemanticExternAbiV1::System { unwind }),
        ExternAbi::Rust => Ok(SemanticExternAbiV1::Rust),
        ExternAbi::RustCall => Ok(SemanticExternAbiV1::RustCall),
        ExternAbi::RustCold => Ok(SemanticExternAbiV1::RustCold),
        ExternAbi::RustPreserveNone => Ok(SemanticExternAbiV1::RustPreserveNone),
        ExternAbi::Unadjusted => Ok(SemanticExternAbiV1::Unadjusted),
        ExternAbi::Custom => Ok(SemanticExternAbiV1::Custom),
        ExternAbi::GpuKernel => Ok(SemanticExternAbiV1::GpuKernel),
        unsupported @ (ExternAbi::RustInvalid
        | ExternAbi::EfiApi
        | ExternAbi::Aapcs { .. }
        | ExternAbi::CmseNonSecureCall
        | ExternAbi::CmseNonSecureEntry
        | ExternAbi::PtxKernel
        | ExternAbi::AvrInterrupt
        | ExternAbi::AvrNonBlockingInterrupt
        | ExternAbi::Msp430Interrupt
        | ExternAbi::RiscvInterruptM
        | ExternAbi::RiscvInterruptS
        | ExternAbi::X86Interrupt
        | ExternAbi::Stdcall { .. }
        | ExternAbi::Fastcall { .. }
        | ExternAbi::Thiscall { .. }
        | ExternAbi::Vectorcall { .. }
        | ExternAbi::SysV64 { .. }
        | ExternAbi::Win64 { .. }) => Err(ProductionSemanticFnAbiErrorV1::UnsupportedExternAbi(
            unsupported,
        )),
    }
}

const fn expected_canon_abi_v1(abi: SemanticExternAbiV1) -> SemanticCanonAbiV1 {
    match abi {
        SemanticExternAbiV1::C { .. }
        | SemanticExternAbiV1::Cdecl { .. }
        | SemanticExternAbiV1::System { .. }
        | SemanticExternAbiV1::Unadjusted => SemanticCanonAbiV1::C,
        SemanticExternAbiV1::Rust | SemanticExternAbiV1::RustCall => SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCold => SemanticCanonAbiV1::RustCold,
        SemanticExternAbiV1::RustPreserveNone => SemanticCanonAbiV1::RustPreserveNone,
        SemanticExternAbiV1::Custom => SemanticCanonAbiV1::Custom,
        SemanticExternAbiV1::GpuKernel => SemanticCanonAbiV1::GpuKernel,
    }
}

const fn convert_canon_abi_v1(abi: CanonAbi) -> SemanticCanonAbiV1 {
    match abi {
        CanonAbi::C => SemanticCanonAbiV1::C,
        CanonAbi::Rust => SemanticCanonAbiV1::Rust,
        CanonAbi::RustCold => SemanticCanonAbiV1::RustCold,
        CanonAbi::RustPreserveNone => SemanticCanonAbiV1::RustPreserveNone,
        CanonAbi::Custom => SemanticCanonAbiV1::Custom,
        CanonAbi::Arm(arm) => SemanticCanonAbiV1::Arm(match arm {
            ArmCall::Aapcs => SemanticArmCallV1::Aapcs,
            ArmCall::CCmseNonSecureCall => SemanticArmCallV1::CCmseNonSecureCall,
            ArmCall::CCmseNonSecureEntry => SemanticArmCallV1::CCmseNonSecureEntry,
        }),
        CanonAbi::GpuKernel => SemanticCanonAbiV1::GpuKernel,
        CanonAbi::Interrupt(interrupt) => SemanticCanonAbiV1::Interrupt(match interrupt {
            InterruptKind::Avr => SemanticInterruptKindV1::Avr,
            InterruptKind::AvrNonBlocking => SemanticInterruptKindV1::AvrNonBlocking,
            InterruptKind::Msp430 => SemanticInterruptKindV1::Msp430,
            InterruptKind::RiscvMachine => SemanticInterruptKindV1::RiscvMachine,
            InterruptKind::RiscvSupervisor => SemanticInterruptKindV1::RiscvSupervisor,
            InterruptKind::X86 => SemanticInterruptKindV1::X86,
        }),
        CanonAbi::X86(x86) => SemanticCanonAbiV1::X86(match x86 {
            X86Call::Fastcall => SemanticX86CallV1::Fastcall,
            X86Call::Stdcall => SemanticX86CallV1::Stdcall,
            X86Call::SysV64 => SemanticX86CallV1::SysV64,
            X86Call::Thiscall => SemanticX86CallV1::Thiscall,
            X86Call::Vectorcall => SemanticX86CallV1::Vectorcall,
            X86Call::Win64 => SemanticX86CallV1::Win64,
        }),
    }
}

fn convert_pass_mode_v1(
    mode: &PassMode,
) -> Result<SemanticAbiPassModeV1, ProductionSemanticFnAbiErrorV1> {
    match mode {
        PassMode::Ignore => Ok(SemanticAbiPassModeV1::Ignore),
        PassMode::Direct(attributes) => Ok(SemanticAbiPassModeV1::Direct(convert_attributes_v1(
            *attributes,
        )?)),
        PassMode::Pair(first, second) => Ok(SemanticAbiPassModeV1::Pair {
            first: convert_attributes_v1(*first)?,
            second: convert_attributes_v1(*second)?,
        }),
        PassMode::Cast { pad_i32, cast } => Ok(SemanticAbiPassModeV1::cast(
            *pad_i32,
            convert_cast_target_v1(cast)?,
        )),
        PassMode::Indirect {
            attrs,
            meta_attrs,
            on_stack,
        } => Ok(SemanticAbiPassModeV1::Indirect {
            attributes: convert_attributes_v1(*attrs)?,
            metadata_attributes: meta_attrs.map(convert_attributes_v1).transpose()?,
            on_stack: *on_stack,
        }),
    }
}

fn convert_attributes_v1(
    attributes: ArgAttributes,
) -> Result<SemanticAbiValueAttributesV1, ProductionSemanticFnAbiErrorV1> {
    let regular = SemanticAbiRegularAttributesV1::from_rustc_bits(attributes.regular.bits())?;
    let extension = match attributes.arg_ext {
        ArgExtension::None => SemanticAbiExtensionV1::None,
        ArgExtension::Zext => SemanticAbiExtensionV1::ZeroExtend,
        ArgExtension::Sext => SemanticAbiExtensionV1::SignExtend,
    };
    SemanticAbiValueAttributesV1::new(
        regular,
        extension,
        attributes.pointee_size.bytes(),
        attributes.pointee_align.map(|alignment| alignment.bytes()),
    )
    .map_err(Into::into)
}

fn convert_cast_target_v1(
    cast: &CastTarget,
) -> Result<SemanticAbiCastV1, ProductionSemanticFnAbiErrorV1> {
    let mut prefix = [None; 8];
    for (semantic, rustc) in prefix.iter_mut().zip(cast.prefix) {
        *semantic = rustc.map(convert_register_v1).transpose()?;
    }
    Ok(SemanticAbiCastV1::new(
        prefix,
        cast.rest_offset.map(|offset| offset.bytes()),
        convert_uniform_v1(cast.rest)?,
        convert_attributes_v1(cast.attrs)?,
    ))
}

fn convert_uniform_v1(
    uniform: Uniform,
) -> Result<SemanticAbiUniformV1, ProductionSemanticFnAbiErrorV1> {
    SemanticAbiUniformV1::from_rustc(
        convert_register_v1(uniform.unit)?,
        uniform.total.bytes(),
        uniform.is_consecutive,
    )
    .map_err(Into::into)
}

fn convert_register_v1(
    register: Reg,
) -> Result<SemanticAbiRegisterV1, ProductionSemanticFnAbiErrorV1> {
    let kind = match register.kind {
        RegKind::Integer => SemanticAbiRegisterKindV1::Integer,
        RegKind::Float => SemanticAbiRegisterKindV1::Float,
        RegKind::Vector => SemanticAbiRegisterKindV1::Vector,
    };
    SemanticAbiRegisterV1::new(kind, register.size.bytes()).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::SemanticAbiPointerCaptureV1;
    use rustc_abi::{Align, Size};
    use rustc_target::callconv::ArgAttribute;

    fn attributes(
        regular: ArgAttribute,
        arg_ext: ArgExtension,
        pointee_size: u64,
        pointee_align: Option<u64>,
    ) -> ArgAttributes {
        ArgAttributes {
            regular,
            arg_ext,
            pointee_size: Size::from_bytes(pointee_size),
            pointee_align: pointee_align.map(|alignment| Align::from_bytes(alignment).unwrap()),
        }
    }

    #[test]
    fn pinned_extern_abi_mapping_is_explicit_and_fail_closed() {
        for abi in ExternAbi::ALL_VARIANTS.iter().copied() {
            let supported = matches!(
                abi,
                ExternAbi::C { .. }
                    | ExternAbi::Cdecl { .. }
                    | ExternAbi::System { .. }
                    | ExternAbi::Rust
                    | ExternAbi::RustCall
                    | ExternAbi::RustCold
                    | ExternAbi::RustPreserveNone
                    | ExternAbi::Unadjusted
                    | ExternAbi::Custom
                    | ExternAbi::GpuKernel
            );
            assert_eq!(convert_extern_abi_v1(abi).is_ok(), supported, "{abi}");
        }

        assert_eq!(
            convert_extern_abi_v1(ExternAbi::C { unwind: true }).unwrap(),
            SemanticExternAbiV1::C { unwind: true }
        );
        assert_eq!(
            convert_extern_abi_v1(ExternAbi::Cdecl { unwind: false }).unwrap(),
            SemanticExternAbiV1::Cdecl { unwind: false }
        );
        assert_eq!(
            convert_extern_abi_v1(ExternAbi::System { unwind: true }).unwrap(),
            SemanticExternAbiV1::System { unwind: true }
        );
    }

    #[test]
    fn pinned_canonical_abi_mapping_is_total() {
        let cases = [
            (CanonAbi::C, SemanticCanonAbiV1::C),
            (CanonAbi::Rust, SemanticCanonAbiV1::Rust),
            (CanonAbi::RustCold, SemanticCanonAbiV1::RustCold),
            (
                CanonAbi::RustPreserveNone,
                SemanticCanonAbiV1::RustPreserveNone,
            ),
            (CanonAbi::Custom, SemanticCanonAbiV1::Custom),
            (
                CanonAbi::Arm(ArmCall::Aapcs),
                SemanticCanonAbiV1::Arm(SemanticArmCallV1::Aapcs),
            ),
            (
                CanonAbi::Arm(ArmCall::CCmseNonSecureCall),
                SemanticCanonAbiV1::Arm(SemanticArmCallV1::CCmseNonSecureCall),
            ),
            (
                CanonAbi::Arm(ArmCall::CCmseNonSecureEntry),
                SemanticCanonAbiV1::Arm(SemanticArmCallV1::CCmseNonSecureEntry),
            ),
            (CanonAbi::GpuKernel, SemanticCanonAbiV1::GpuKernel),
            (
                CanonAbi::Interrupt(InterruptKind::Avr),
                SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::Avr),
            ),
            (
                CanonAbi::Interrupt(InterruptKind::AvrNonBlocking),
                SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::AvrNonBlocking),
            ),
            (
                CanonAbi::Interrupt(InterruptKind::Msp430),
                SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::Msp430),
            ),
            (
                CanonAbi::Interrupt(InterruptKind::RiscvMachine),
                SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::RiscvMachine),
            ),
            (
                CanonAbi::Interrupt(InterruptKind::RiscvSupervisor),
                SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::RiscvSupervisor),
            ),
            (
                CanonAbi::Interrupt(InterruptKind::X86),
                SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::X86),
            ),
            (
                CanonAbi::X86(X86Call::Fastcall),
                SemanticCanonAbiV1::X86(SemanticX86CallV1::Fastcall),
            ),
            (
                CanonAbi::X86(X86Call::Stdcall),
                SemanticCanonAbiV1::X86(SemanticX86CallV1::Stdcall),
            ),
            (
                CanonAbi::X86(X86Call::SysV64),
                SemanticCanonAbiV1::X86(SemanticX86CallV1::SysV64),
            ),
            (
                CanonAbi::X86(X86Call::Thiscall),
                SemanticCanonAbiV1::X86(SemanticX86CallV1::Thiscall),
            ),
            (
                CanonAbi::X86(X86Call::Vectorcall),
                SemanticCanonAbiV1::X86(SemanticX86CallV1::Vectorcall),
            ),
            (
                CanonAbi::X86(X86Call::Win64),
                SemanticCanonAbiV1::X86(SemanticX86CallV1::Win64),
            ),
        ];
        for (rustc, semantic) in cases {
            assert_eq!(convert_canon_abi_v1(rustc), semantic);
        }
    }

    #[test]
    fn attributes_preserve_every_rustc_axis() {
        let rustc = attributes(
            ArgAttribute::CapturesReadOnly
                | ArgAttribute::NoAlias
                | ArgAttribute::NonNull
                | ArgAttribute::ReadOnly
                | ArgAttribute::InReg
                | ArgAttribute::NoUndef,
            ArgExtension::Sext,
            37,
            Some(16),
        );
        let semantic = convert_attributes_v1(rustc).unwrap();
        assert!(semantic.regular().no_alias());
        assert_eq!(
            semantic.regular().pointer_capture(),
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly)
        );
        assert!(semantic.regular().non_null());
        assert!(semantic.regular().read_only());
        assert!(semantic.regular().in_register());
        assert!(semantic.regular().no_undef());
        assert_eq!(semantic.extension(), SemanticAbiExtensionV1::SignExtend);
        assert_eq!(semantic.pointee_size_bytes(), 37);
        assert_eq!(semantic.pointee_alignment_bytes(), Some(16));
    }

    #[test]
    fn every_pointer_capture_encoding_is_preserved() {
        let cases = [
            (ArgAttribute::empty(), None),
            (
                ArgAttribute::CapturesNone,
                Some(SemanticAbiPointerCaptureV1::CapturesNone),
            ),
            (
                ArgAttribute::CapturesAddress,
                Some(SemanticAbiPointerCaptureV1::CapturesAddress),
            ),
            (
                ArgAttribute::CapturesReadOnly,
                Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            ),
        ];
        for (rustc, expected) in cases {
            let converted =
                convert_attributes_v1(attributes(rustc, ArgExtension::None, 0, None)).unwrap();
            assert_eq!(converted.regular().pointer_capture(), expected);
        }
    }

    #[test]
    fn malformed_pointer_capture_bits_fail_closed() {
        let malformed = attributes(
            ArgAttribute::from_bits_retain(0b001),
            ArgExtension::None,
            0,
            None,
        );
        assert!(matches!(
            convert_attributes_v1(malformed),
            Err(ProductionSemanticFnAbiErrorV1::Schema(
                SemanticMirErrorV1::InvalidFunctionAbi
            ))
        ));
    }

    #[test]
    fn every_pass_mode_and_cast_axis_is_preserved() {
        assert_eq!(
            convert_pass_mode_v1(&PassMode::Ignore).unwrap(),
            SemanticAbiPassModeV1::Ignore
        );

        let direct = attributes(ArgAttribute::NoUndef, ArgExtension::Zext, 0, None);
        let pair_second = attributes(ArgAttribute::InReg, ArgExtension::Sext, 0, None);
        assert!(matches!(
            convert_pass_mode_v1(&PassMode::Direct(direct)).unwrap(),
            SemanticAbiPassModeV1::Direct(value)
                if value.extension() == SemanticAbiExtensionV1::ZeroExtend
        ));
        assert!(matches!(
            convert_pass_mode_v1(&PassMode::Pair(direct, pair_second)).unwrap(),
            SemanticAbiPassModeV1::Pair { first, second }
                if first.regular().no_undef()
                    && second.regular().in_register()
                    && second.extension() == SemanticAbiExtensionV1::SignExtend
        ));

        let cast = CastTarget {
            prefix: [
                Some(Reg::i32()),
                Some(Reg::f64()),
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            rest_offset: Some(Size::from_bytes(16)),
            rest: Uniform::consecutive(
                Reg {
                    kind: RegKind::Vector,
                    size: Size::from_bytes(16),
                },
                Size::from_bytes(32),
            ),
            attrs: direct,
        };
        let converted = convert_pass_mode_v1(&PassMode::Cast {
            pad_i32: true,
            cast: Box::new(cast),
        })
        .unwrap();
        let SemanticAbiPassModeV1::Cast { pad_i32, cast } = converted else {
            panic!("cast mode was not retained");
        };
        assert!(pad_i32);
        assert_eq!(
            cast.prefix()[0].unwrap().kind(),
            SemanticAbiRegisterKindV1::Integer
        );
        assert_eq!(
            cast.prefix()[1].unwrap().kind(),
            SemanticAbiRegisterKindV1::Float
        );
        assert_eq!(cast.rest_offset_bytes(), Some(16));
        assert_eq!(cast.rest().unit().kind(), SemanticAbiRegisterKindV1::Vector);
        assert_eq!(cast.rest_total_bytes(), 32);
        assert!(cast.rest_consecutive());

        let metadata = attributes(
            ArgAttribute::CapturesAddress,
            ArgExtension::None,
            0,
            Some(8),
        );
        let converted = convert_pass_mode_v1(&PassMode::Indirect {
            attrs: direct,
            meta_attrs: Some(metadata),
            on_stack: false,
        })
        .unwrap();
        assert!(matches!(
            converted,
            SemanticAbiPassModeV1::Indirect {
                metadata_attributes: Some(value),
                on_stack: false,
                ..
            } if value.regular().pointer_capture()
                == Some(SemanticAbiPointerCaptureV1::CapturesAddress)
        ));
    }

    #[test]
    fn hard_argument_bound_is_checked_without_allocation() {
        assert!(require_count_v1("arguments", HARD_MAX_CALL_ARGUMENTS_V1 as usize).is_ok());
        assert!(matches!(
            require_count_v1(
                "arguments",
                usize::try_from(HARD_MAX_CALL_ARGUMENTS_V1 + 1).unwrap()
            ),
            Err(ProductionSemanticFnAbiErrorV1::LimitExceeded {
                component: "arguments",
                actual,
                maximum: HARD_MAX_CALL_ARGUMENTS_V1,
            }) if actual == HARD_MAX_CALL_ARGUMENTS_V1 + 1
        ));
    }
}
