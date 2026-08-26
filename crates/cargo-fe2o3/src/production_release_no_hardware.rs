//! Policy-only row-softmax boundary for the default no-hardware command image.

use core::fmt;
use std::error::Error;

use fe2o3_hsaco_finalize::{
    InertFirstBuildWorkerV2EvidenceV1, RowSoftmaxV1DirectWorkerExpectationV1,
};

use crate::numerical_contract::{SoftmaxContractErrorV1, row_softmax_oracle_v1};

pub const ROW_SOFTMAX_V1_PRODUCTION_POLICY: &str = "gfx942-ocml-unmasked-64-v1";

const ROW_ELEMENTS_V1: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactRowSoftmaxV1CaseV1 {
    Normal,
    Equal,
    Dominant,
    Exceptional,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowSoftmaxV1MaskProfileV1 {
    Unmasked,
    Alternating,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1ReleaseWorkloadV1<'policy> {
    pub case: ExactRowSoftmaxV1CaseV1,
    pub row_elements: u32,
    pub mask: RowSoftmaxV1MaskProfileV1,
    pub comparison_policy: &'policy str,
}

#[derive(Debug)]
pub struct AdmittedRowSoftmaxV1WorkloadV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1ProductionReceiptV1;

impl RowSoftmaxV1ProductionReceiptV1 {
    pub const fn case(&self) -> ExactRowSoftmaxV1CaseV1 {
        ExactRowSoftmaxV1CaseV1::Normal
    }

    pub const fn unload_identity(&self) -> &[u8; 32] {
        &[0; 32]
    }

    pub const fn proves_masked_execution(&self) -> bool {
        false
    }

    pub const fn proves_verus_refinement(&self) -> bool {
        false
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum RowSoftmaxV1ProductionErrorV1 {
    Shape,
    Mask,
    Policy,
    Oracle(SoftmaxContractErrorV1),
    Runtime(String),
}

impl fmt::Display for RowSoftmaxV1ProductionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape => formatter.write_str("stage=workload-shape: fixed row width must be 64"),
            Self::Mask => formatter.write_str("stage=workload-mask: fixed profile is unmasked"),
            Self::Policy => formatter.write_str(
                "stage=workload-policy: comparison policy differs from the fixed gfx942 OCML profile",
            ),
            Self::Oracle(error) => {
                write!(formatter, "stage=cpu-oracle: fixed workload was rejected: {error:?}")
            }
            Self::Runtime(error) => write!(formatter, "stage=typed-runtime: {error}"),
        }
    }
}

impl Error for RowSoftmaxV1ProductionErrorV1 {}

pub fn preflight_row_softmax_v1_workload_v1(
    request: RowSoftmaxV1ReleaseWorkloadV1<'_>,
) -> Result<AdmittedRowSoftmaxV1WorkloadV1, RowSoftmaxV1ProductionErrorV1> {
    if request.row_elements != ROW_ELEMENTS_V1 as u32 {
        return Err(RowSoftmaxV1ProductionErrorV1::Shape);
    }
    if request.mask != RowSoftmaxV1MaskProfileV1::Unmasked {
        return Err(RowSoftmaxV1ProductionErrorV1::Mask);
    }
    if request.comparison_policy != ROW_SOFTMAX_V1_PRODUCTION_POLICY {
        return Err(RowSoftmaxV1ProductionErrorV1::Policy);
    }
    let input = case_input(request.case);
    let mut expected = [0.0; ROW_ELEMENTS_V1];
    row_softmax_oracle_v1(&input, None, &mut expected)
        .map_err(RowSoftmaxV1ProductionErrorV1::Oracle)?;
    Ok(AdmittedRowSoftmaxV1WorkloadV1)
}

pub struct NoHardwareRowSoftmaxTokenV1(());

pub fn admit_row_softmax_v1_source_tested_artifact_v1(
    _evidence: InertFirstBuildWorkerV2EvidenceV1,
    _expectation: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<NoHardwareRowSoftmaxTokenV1, RowSoftmaxV1ProductionErrorV1> {
    Err(RowSoftmaxV1ProductionErrorV1::Runtime(
        "the workload-specific row-softmax runtime was retired; use Worker V3".to_owned(),
    ))
}

pub fn execute_row_softmax_v1_production_workload_v1(
    _token: NoHardwareRowSoftmaxTokenV1,
    _workload: AdmittedRowSoftmaxV1WorkloadV1,
) -> Result<RowSoftmaxV1ProductionReceiptV1, RowSoftmaxV1ProductionErrorV1> {
    Err(RowSoftmaxV1ProductionErrorV1::Runtime(
        "the workload-specific row-softmax runtime was retired; use Worker V3".to_owned(),
    ))
}

fn case_input(case: ExactRowSoftmaxV1CaseV1) -> [f32; ROW_ELEMENTS_V1] {
    match case {
        ExactRowSoftmaxV1CaseV1::Normal => {
            core::array::from_fn(|index| ((index * 17 + 3) % 29) as f32 * 0.25 - 3.5)
        }
        ExactRowSoftmaxV1CaseV1::Equal => [0.5; ROW_ELEMENTS_V1],
        ExactRowSoftmaxV1CaseV1::Dominant => {
            let mut input = [-32.0; ROW_ELEMENTS_V1];
            input[37] = 32.0;
            input
        }
        ExactRowSoftmaxV1CaseV1::Exceptional => {
            let mut input = [0.0; ROW_ELEMENTS_V1];
            input[11] = f32::NAN;
            input
        }
    }
}
