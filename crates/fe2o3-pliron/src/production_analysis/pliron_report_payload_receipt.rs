//! Private accounting for the payload copied by sealed report validation.
//!
//! Producer reservations also cover caches and potential findings. A receipt
//! accounts only for the actual report; it never releases that reservation.

use super::pliron_resource_envelope::{
    ProductionAnalysisResourceLimitV1, ProductionAnalysisResourceLimitsV1,
    ProductionAnalysisResourcePhaseV1, ProductionAnalysisResourceUpperBoundV1,
};
use crate::KernelCheckPassKindV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionAnalysisReportPayloadReceiptV1 {
    pass: KernelCheckPassKindV1,
    census_work: usize,
    exact: Option<ExactReportPayloadV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactReportPayloadV1 {
    owner_storage: usize,
    clone_storage: usize,
    clone_work: usize,
    comparison_work: usize,
    status_work: usize,
}

pub(super) fn payload_limit_v1(resource: &'static str) -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::ReportValidation,
        resource,
    }
}

pub(super) fn payload_sum_v1(values: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0usize, |sum, value| {
        sum.checked_add(*value)
            .ok_or_else(|| payload_limit_v1("report payload accounting overflow"))
    })
}

pub(super) fn payload_product_v1(
    value: usize,
    factor: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    value
        .checked_mul(factor)
        .ok_or_else(|| payload_limit_v1("report payload accounting overflow"))
}

pub(super) fn require_payload_census_v1(
    work: usize,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<(), ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::ReportValidation;
    limits.require(
        phase,
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, 0, 0)?,
    )?;
    Ok(())
}

impl ProductionAnalysisReportPayloadReceiptV1 {
    pub(super) const fn fallback(pass: KernelCheckPassKindV1, census_work: usize) -> Self {
        Self {
            pass,
            census_work,
            exact: None,
        }
    }

    pub(super) const fn exact(
        pass: KernelCheckPassKindV1,
        census_work: usize,
        owner_storage: usize,
        clone_storage: usize,
        clone_work: usize,
        comparison_work: usize,
        status_work: usize,
    ) -> Self {
        Self {
            pass,
            census_work,
            exact: Some(ExactReportPayloadV1 {
                owner_storage,
                clone_storage,
                clone_work,
                comparison_work,
                status_work,
            }),
        }
    }

    pub(super) fn require_pass(
        self,
        pass: KernelCheckPassKindV1,
    ) -> Result<(), ProductionAnalysisResourceLimitV1> {
        if self.pass != pass {
            return Err(payload_limit_v1("report payload receipt pass mismatch"));
        }
        Ok(())
    }

    pub(super) const fn is_exact(self) -> bool {
        self.exact.is_some()
    }

    #[cfg(test)]
    pub(super) fn owner_storage_for_test_v1(self) -> usize {
        self.exact
            .map_or(1, |exact| exact.owner_storage.max(exact.clone_storage))
    }

    pub(super) fn payload_bounds(
        self,
        pass: KernelCheckPassKindV1,
        producer_retained: usize,
    ) -> Result<(usize, usize), ProductionAnalysisResourceLimitV1> {
        self.require_pass(pass)?;
        let Some(exact) = self.exact else {
            // Three clones, two comparisons, and four status evaluations.
            return Ok((
                payload_sum_v1(&[self.census_work, payload_product_v1(producer_retained, 9)?])?,
                producer_retained,
            ));
        };
        if exact.owner_storage > producer_retained || exact.clone_storage > producer_retained {
            return Err(payload_limit_v1(
                "report payload exceeds producer reservation",
            ));
        }
        // Exact receipts require all findings vectors to be allocation-free
        // and empty. Their four status evaluations are fixed-size operations.
        Ok((
            payload_sum_v1(&[
                self.census_work,
                payload_product_v1(exact.clone_work, 3)?,
                payload_product_v1(exact.comparison_work, 2)?,
                exact.status_work,
            ])?,
            exact.clone_storage,
        ))
    }
}

pub(super) fn try_reserve_payload_v1<T>(
    values: &mut Vec<T>,
    count: usize,
) -> Result<(), ProductionAnalysisResourceLimitV1> {
    allocation_checkpoint_v1()?;
    values
        .try_reserve_exact(count)
        .map_err(|_| payload_limit_v1("report payload allocation"))
}

pub(super) fn try_clone_payload_string_v1(
    value: &str,
) -> Result<String, ProductionAnalysisResourceLimitV1> {
    allocation_checkpoint_v1()?;
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| payload_limit_v1("report payload allocation"))?;
    result.push_str(value);
    Ok(result)
}

#[cfg(test)]
thread_local! {
    static FAIL_PAYLOAD_ALLOCATION_AFTER_V1: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn fail_next_payload_allocation_v1() {
    fail_payload_allocation_after_v1(0);
}

#[cfg(test)]
pub(super) fn fail_payload_allocation_after_v1(successful_allocations: usize) {
    FAIL_PAYLOAD_ALLOCATION_AFTER_V1.with(|remaining| remaining.set(Some(successful_allocations)));
}

fn allocation_checkpoint_v1() -> Result<(), ProductionAnalysisResourceLimitV1> {
    #[cfg(test)]
    if FAIL_PAYLOAD_ALLOCATION_AFTER_V1.with(|remaining| match remaining.get() {
        None => false,
        Some(0) => {
            remaining.set(None);
            true
        }
        Some(count) => {
            remaining.set(Some(count - 1));
            false
        }
    }) {
        return Err(payload_limit_v1("report payload allocation"));
    }
    Ok(())
}

macro_rules! impl_empty_findings_payload_v1 {
    ($report:ty) => {
        impl $report {
            pub(super) fn validation_payload_receipt_v1(
                &self,
                limits: super::pliron_resource_envelope::ProductionAnalysisResourceLimitsV1,
            ) -> Result<
                super::pliron_report_payload_receipt::ProductionAnalysisReportPayloadReceiptV1,
                super::pliron_resource_envelope::ProductionAnalysisResourceLimitV1,
            > {
                use super::pliron_report_payload_receipt::{
                    ProductionAnalysisReportPayloadReceiptV1 as Receipt, require_payload_census_v1,
                };
                require_payload_census_v1(2, limits)?;
                Ok(
                    if self.findings.is_empty() && self.findings.capacity() == 0 {
                        // Each clone checks length/capacity and initializes
                        // one empty vector; equality compares vector lengths.
                        Receipt::exact(self.pass(), 2, 0, 0, 3, 1, 4)
                    } else {
                        Receipt::fallback(self.pass(), 2)
                    },
                )
            }

            pub(super) fn try_clone_validation_payload_v1(
                &self,
            ) -> Result<Self, super::pliron_resource_envelope::ProductionAnalysisResourceLimitV1>
            {
                if !self.findings.is_empty() || self.findings.capacity() != 0 {
                    return Err(super::pliron_report_payload_receipt::payload_limit_v1(
                        "report payload shape changed",
                    ));
                }
                Ok(Self {
                    findings: Vec::new(),
                })
            }
        }
    };
}

pub(super) use impl_empty_findings_payload_v1;
