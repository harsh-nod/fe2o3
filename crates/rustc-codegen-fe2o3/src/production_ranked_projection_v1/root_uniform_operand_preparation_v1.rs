//! Shared uniform operand decisions; not a complete argument producer or bounds loan.
//! Paid operations and all accepted credits belong to the caller's retained owner.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::mem::size_of;
type Error = ProductionRankedProjectionErrorV1;

enum OperandMeterV1<'a, 'b, 'w> {
    Legacy,
    Paid(&'a mut PreparationResourcesV1<'b, 'w>),
}
impl OperandMeterV1<'_, '_, '_> {
    fn work(&mut self, amount: usize) -> Result<(), Error> {
        match self {
            Self::Legacy => Ok(()),
            Self::Paid(resources) => resources.work(amount),
        }
    }
    fn operand_scan(&mut self, operand: &SemanticOperandV1) -> Result<(), Error> {
        match self {
            Self::Legacy => Ok(()),
            Self::Paid(resources) => {
                let n = match operand {
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                        place.projections().len()
                    }
                    SemanticOperandV1::Constant(_) => 0,
                };
                resources.work(n)
            }
        }
    }
    fn reserve_operation(
        &mut self,
        operations: &mut Vec<ProductionRankedOperationV1>,
    ) -> Result<(), Error> {
        match self {
            Self::Legacy => super::reserve_operation(operations),
            Self::Paid(resources) => {
                // Preserve the existing semantic operation-limit check before
                // any new resource admission and before the SSA allocator.
                if operations.len() == MAX_PROJECTED_OPERATIONS_V1 {
                    return Err(Error::Unsupported(
                        "a semantic intrinsic projection exceeding the ranked operation limit",
                    ));
                }
                resources.work(1)?;
                resources.reserve(operations, 1)
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn uniform_operand_legacy_v1(
    operand: &SemanticOperandV1,
    constants: &[Option<u64>],
    origins: &[Option<u32>],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<Option<ProductionRankedValueV1>, Error> {
    project_shared(
        operand,
        constants,
        origins,
        arguments,
        next_argument,
        operations,
        next_value,
        &mut OperandMeterV1::Legacy,
    )
}

/// This is a paid component primitive, not an authentic producer constructor.
/// All input rows are DATA here. Its eventual actual caller must couple them to
/// the same real source assembly and its one operations/value-allocator owner.
/// It must retain that physical owner and this original credit counter through
/// every enclosing postflight, including errors/unwinds; this function refunds
/// nothing and never detaches a vector or creates a completed-stage token.
#[allow(clippy::too_many_arguments)]
pub(super) fn uniform_operand_paid_v1(
    operand: &SemanticOperandV1,
    constants: &[Option<u64>],
    origins: &[Option<u32>],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<Option<ProductionRankedValueV1>, Error> {
    if !resources.is_metered() || resources.has_denial() {
        return Err(Error::Unsupported(
            "uniform operand preparation requires the retained original ledger",
        ));
    }
    let frame = operand_frame_v1()?;
    resources.work(frame)?;
    resources.reserve_storage(frame)?;
    project_shared(
        operand,
        constants,
        origins,
        arguments,
        next_argument,
        operations,
        next_value,
        &mut OperandMeterV1::Paid(resources),
    )
}

#[allow(clippy::too_many_arguments)]
fn project_shared(
    operand: &SemanticOperandV1,
    constants: &[Option<u64>],
    stable_argument_origins: &[Option<u32>],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    meter: &mut OperandMeterV1<'_, '_, '_>,
) -> Result<Option<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
    meter.work(1)?;
    if let Some(value) = constant_operand_value(operand, constants) {
        meter.reserve_operation(operations)?;
        meter.work(1)?;
        let result = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::IndexConstant { result, value });
        return Ok(Some(ProductionRankedValueV1::Local(result)));
    }
    meter.operand_scan(operand)?;
    let Some(local) = simple_operand_local(operand) else {
        return Ok(None);
    };
    meter.work(1)?;
    let local_index = local.index() as usize;
    let origin = stable_argument_origins.get(local_index).copied().flatten();
    let Some(origin) = origin.map(|origin| origin as usize) else {
        return Ok(None);
    };
    meter.work(1)?;
    let slot = arguments
        .get_mut(origin)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "a uniform switch argument origin outside the semantic local table",
        ))?;
    let argument = match *slot {
        Some(argument) => argument,
        None => {
            // Prepay conversion, checked count advance and slot installation
            // together: no new denial can split the counter/slot mutation.
            meter.work(3)?;
            let argument = u32::try_from(*next_argument).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "too many uniform switch ranked arguments",
                )
            })?;
            *next_argument = next_argument.checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "uniform switch ranked argument count overflow",
                ),
            )?;
            *slot = Some(argument);
            argument
        }
    };
    Ok(Some(ProductionRankedValueV1::Argument(argument)))
}

fn arithmetic() -> Error {
    bf16_nominal_preparation_resources_v1::resource(Resource::Arithmetic)
}
fn sum(parts: &[usize]) -> Result<usize, Error> {
    parts.iter().try_fold(0usize, |n, value| {
        n.checked_add(*value).ok_or_else(arithmetic)
    })
}
fn call_frame<T>(locals: usize) -> Result<usize, Error> {
    sum(&[
        locals,
        size_of::<T>(),
        size_of::<T>(),
        size_of::<Result<T, Error>>(),
        size_of::<Result<T, Error>>(),
    ])
}
const FRAME_ROWS: usize = 12;
fn frame_rows() -> Result<[usize; FRAME_ROWS], Error> {
    use fe2o3_mir_model::semantic_mir_v1 as mir;
    Ok([
        // 0: paid entry arguments, meter and admission temporaries.
        call_frame::<Option<ProductionRankedValueV1>>(size_of::<(
            &SemanticOperandV1,
            &[Option<u64>],
            &[Option<u32>],
            &mut [Option<u32>],
            &mut usize,
            &mut Vec<ProductionRankedOperationV1>,
            &mut u32,
            &mut PreparationResourcesV1<'_, '_>,
            OperandMeterV1<'_, '_, '_>,
            usize,
            bool,
        )>())?,
        // 1: shared decision caller and all nested branch return transfers.
        call_frame::<Option<ProductionRankedValueV1>>(size_of::<(
            &SemanticOperandV1,
            &[Option<u64>],
            &[Option<u32>],
            &mut [Option<u32>],
            &mut usize,
            &mut Vec<ProductionRankedOperationV1>,
            &mut u32,
            &mut OperandMeterV1<'_, '_, '_>,
            Option<u64>,
            u64,
            Option<SemanticLocalIdV1>,
            SemanticLocalIdV1,
            usize,
            Option<u32>,
            usize,
            &mut Option<u32>,
            u32,
            ProductionRankedValueIdV1,
            ProductionRankedOperationV1,
        )>())?,
        // 2: distinct nested constant_operand_value and constant_definition
        // callers/results; no unrelated row supplies their overlapping locals.
        sum(&[
            call_frame::<Option<u64>>(size_of::<(
                &SemanticOperandV1,
                &[Option<u64>],
                ConstantDefinitionV1,
                SemanticLocalIdV1,
                usize,
                Option<&Option<u64>>,
                Option<Option<u64>>,
                Option<u64>,
            )>())?,
            call_frame::<ConstantDefinitionV1>(size_of::<(
                &SemanticOperandV1,
                &mir::SemanticConstantV1,
                &SemanticConstantValueV1,
                &mir::SemanticScalarValueV1,
                &SemanticPlaceV1,
                u128,
                Result<u64, std::num::TryFromIntError>,
                SemanticLocalIdV1,
            )>())?,
        ])?,
        // 3: simple_operand_local/raw_operand_place/transparent_operand_place
        // keep distinct borrowed caller/result vertices.
        sum(&[
            call_frame::<Option<SemanticLocalIdV1>>(size_of::<(
                &SemanticOperandV1,
                Option<&SemanticPlaceV1>,
                &SemanticPlaceV1,
                bool,
            )>())?,
            call_frame::<Option<&SemanticPlaceV1>>(size_of::<(
                &SemanticOperandV1,
                &SemanticPlaceV1,
            )>())?,
            call_frame::<Option<&SemanticPlaceV1>>(size_of::<(
                &SemanticOperandV1,
                Option<&SemanticPlaceV1>,
                &SemanticPlaceV1,
            )>())?,
        ])?,
        // 4: original transparent projection scan, prepaid at its reached site.
        call_frame::<Option<&SemanticPlaceV1>>(size_of::<(
            &SemanticPlaceV1,
            &[mir::SemanticProjectionV1],
            std::slice::Iter<'_, mir::SemanticProjectionV1>,
            &mir::SemanticProjectionV1,
            SemanticProjectionKindV1,
            bool,
        )>())?,
        // 5: work and projection selector callers remain separate.
        sum(&[
            call_frame::<()>(size_of::<(
                &mut OperandMeterV1<'_, '_, '_>,
                usize,
                &mut PreparationResourcesV1<'_, '_>,
            )>())?,
            call_frame::<()>(size_of::<(
                &mut OperandMeterV1<'_, '_, '_>,
                &SemanticOperandV1,
                &SemanticPlaceV1,
                usize,
                &mut PreparationResourcesV1<'_, '_>,
            )>())?,
        ])?,
        // 6: reserve adapter; actual vector storage/growth stays separately paid.
        call_frame::<()>(size_of::<(
            &mut OperandMeterV1<'_, '_, '_>,
            &mut Vec<ProductionRankedOperationV1>,
            &mut PreparationResourcesV1<'_, '_>,
            usize,
            bool,
        )>())?,
        // 7: unchanged next_value_id's check and error/value transfer.
        call_frame::<ProductionRankedValueIdV1>(size_of::<(&mut u32, u32, Option<u32>)>())?,
        // 8: origin/slot conversion and checked count update temporaries.
        call_frame::<u32>(size_of::<(
            &[Option<u32>],
            &mut [Option<u32>],
            usize,
            Option<&Option<u32>>,
            Option<Option<u32>>,
            Option<u32>,
            &mut Option<u32>,
            &mut usize,
            Result<u32, std::num::TryFromIntError>,
            u32,
            Option<usize>,
        )>())?,
        // 9: complete roster array and operand_frame caller/return transfers.
        call_frame::<[usize; FRAME_ROWS]>(size_of::<(
            [usize; FRAME_ROWS],
            &[usize],
            usize,
            Result<usize, Error>,
        )>())?,
        // 10: checked sum/call-frame helper state.
        sum(&[
            call_frame::<usize>(size_of::<(usize, [usize; 5], &[usize], Result<usize, Error>)>())?,
            call_frame::<usize>(size_of::<(
                &[usize],
                std::slice::Iter<'_, usize>,
                usize,
                &usize,
                Option<usize>,
            )>())?,
        ])?,
        // 11: static-message semantic errors and resource arithmetic constructor.
        call_frame::<Error>(size_of::<(&'static str, Error, Resource)>())?,
    ])
}
pub(super) fn operand_frame_v1() -> Result<usize, Error> {
    sum(&frame_rows()?)
}
#[cfg(test)]
pub(super) mod test_access {
    use super::*;
    pub(in crate::production_ranked_projection_v1) fn constant_callers_control() {
        use fe2o3_mir_model::semantic_mir_v1 as mir;
        // Independent typed formulas, not calls to call_frame or the producer.
        let lookup = size_of::<(
            &SemanticOperandV1,
            &[Option<u64>],
            ConstantDefinitionV1,
            SemanticLocalIdV1,
            usize,
            Option<&Option<u64>>,
            Option<Option<u64>>,
            Option<u64>,
        )>() + 2 * size_of::<Option<u64>>()
            + 2 * size_of::<Result<Option<u64>, Error>>();
        let definition = size_of::<(
            &SemanticOperandV1,
            &mir::SemanticConstantV1,
            &SemanticConstantValueV1,
            &mir::SemanticScalarValueV1,
            &SemanticPlaceV1,
            u128,
            Result<u64, std::num::TryFromIntError>,
            SemanticLocalIdV1,
        )>() + 2 * size_of::<ConstantDefinitionV1>()
            + 2 * size_of::<Result<ConstantDefinitionV1, Error>>();
        assert!(lookup > 0 && definition > 0);
        assert_eq!(frame_rows().unwrap()[2], lookup + definition);
        assert!(sum(&[usize::MAX, lookup]).is_err());
        assert!(sum(&[usize::MAX, definition]).is_err());
    }
    pub(in crate::production_ranked_projection_v1) fn frame_control() {
        let rows = frame_rows().unwrap();
        assert_eq!(rows.len(), 12);
        assert!(rows.iter().all(|n| *n > 0));
        assert_eq!(sum(&rows).unwrap(), operand_frame_v1().unwrap());
        for row in rows {
            assert!(sum(&[usize::MAX, row]).is_err());
        }
        assert!(call_frame::<Option<ProductionRankedValueV1>>(usize::MAX).is_err());
    }
}
