//! Inert coordinate checks inside original locked publication custody.
use super::*;
use fe2o3_compiler_lineage::{
    RUSTC_ENROLLMENT_INVENTORY_MAX_READ_WORK_V1 as READ_WORK,
    RUSTC_ENROLLMENT_INVENTORY_WORKING_STORAGE_V1 as READ_SCRATCH,
    RustcEnrollmentInventoryErrorV1 as InventoryError,
    read_rustc_enrollment_inventory_v1 as read_inventory,
};

pub(super) const WORK: usize = ENTRY + READ_WORK + Enrollment::HEADER_MATCH_WORK;
pub(super) const SCRATCH: usize = READ_SCRATCH + size_of::<Option<Enrollment>>();

// Only the concrete original Owners builder supplies these bytes and full quote.
// No alternate inventory view or received header can select the expectation.
pub(super) fn require(
    inventory: &[u8],
    owners: usize,
    expected: &Option<Enrollment>,
    b: &mut Budget<'_>,
) -> Result<()> {
    b.check_prior_denials_v1()?;
    let floor = owners
        .checked_add(size_of::<Option<Enrollment>>())
        .ok_or(Resource::Arithmetic)?;
    b.with_prepaid_scope(
        floor,
        ENTRY,
        ENTRY + Enrollment::HEADER_MATCH_WORK,
        SCRATCH,
        |b| {
            let inventory = read_inventory(inventory, READ_SCRATCH, |work| b.charge_work(work))
                .map_err(|error| match error {
                    InventoryError::Charge(error) => RootPublicationCustodyErrorV3::from(error),
                    _ => RootPublicationCustodyErrorV3::state(
                        "publication enrollment inventory is missing or invalid",
                    ),
                })?;
            let header = inventory.header();
            let matches = match expected {
                Some(expected) => {
                    expected.enrollment_binding_count != 0 && expected.matches_header(&header)
                }
                None => header.enrollment_binding_count == 0,
            };
            if !matches {
                return Err(RootPublicationCustodyErrorV3::state(
                    "publication enrollment differs from original request",
                ));
            }
            b.check_prior_denials_v1()?;
            Ok(())
        },
    )
}

#[cfg(test)]
#[path = "compiler_execution_root_publication_enrollment_tests.rs"]
mod tests;
