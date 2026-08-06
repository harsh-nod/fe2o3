use fe2o3_core::{
    CooperativeCapabilityError, Error, GpuContext, PeerAccessEnableError,
    PeerAccessObservationError,
};

#[test]
fn opt_in_live_cooperative_and_peer_observation() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("FE2O3_ALLOW_GPU_SMOKE").as_deref() != Ok("1") {
        return Ok(());
    }
    let expected_target = std::env::var("FE2O3_TARGET")
        .expect("FE2O3_TARGET must explicitly name the hardware smoke target");

    let source = GpuContext::new(0)?;
    let observed_target = source.observe_target()?.target_id().to_string();
    assert_eq!(observed_target, expected_target);

    match source.observe_cooperative_launch() {
        Ok(capability) => {
            assert!(capability.is_for_context(&source));
            assert_eq!(capability.device_id(), 0);
        }
        Err(CooperativeCapabilityError::Unsupported { .. }) => {}
        Err(error) => return Err(error.into()),
    }

    let destination = match GpuContext::new(1) {
        Ok(context) => context,
        Err(Error::NoDevice { .. }) => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    match source.observe_peer_access(&destination) {
        Ok(capability) => match capability.enable() {
            Ok(access) => {
                let outcome = access.disable()?;
                eprintln!("peer mapping cleanup: {outcome:?}");
            }
            Err(PeerAccessEnableError::AlreadyEnabled { .. }) => {}
            Err(error) => return Err(error.into()),
        },
        Err(PeerAccessObservationError::Unavailable { .. }) => {}
        Err(error) => return Err(error.into()),
    }

    Ok(())
}
