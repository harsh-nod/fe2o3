use fe2o3_host::ValidatedPublishedDirectLinkSelectionV1;

fn promote_inert(admission: ValidatedPublishedDirectLinkSelectionV1) {
    let _ = admission.into_pending_load_admission();
}

fn main() {}
