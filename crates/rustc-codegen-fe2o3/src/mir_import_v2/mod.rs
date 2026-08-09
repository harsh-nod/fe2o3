mod accounting;
mod budget;
mod normalized;
mod preflight;
mod rustc_adapter;
mod type_preflight;

pub(crate) fn capture_observation_sha256_v2<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
) -> Result<[u8; 32], String> {
    use sha2::{Digest, Sha256};

    let limits = normalized::CaptureLimitsV2::default();
    let capture = rustc_adapter::capture_instance_body_v2(tcx, instance, limits)
        .map_err(|error| error.to_string())?;
    let data = rustc_adapter::rustc_authentic_capture_data_v2(&capture);
    let canonical =
        normalized::canonical_semantic_bytes_v2(data, limits).map_err(|error| error.to_string())?;
    Ok(Sha256::digest(&canonical).into())
}

#[cfg(test)]
mod tests;
