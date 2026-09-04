#![allow(unsafe_code)]

use std::error::Error;
use std::io;

use fe2o3_runtime::{
    KfdRuntimeAuthorityRequestV1, KfdRuntimeBackendV1, KfdRuntimeLaunchAuthorityV1,
    serve_runtime_backend_worker_v5_in_place,
};

const USAGE: &str = "usage: fe2o3-runtime-kfd-worker-v5-qualification <unique-id>";

#[derive(Debug)]
struct CopyOnlyQualificationAuthorityV1;

// SAFETY: This feature-gated qualification child admits no machine code. It
// serves memory and copy operations only and rejects every launch request.
unsafe impl KfdRuntimeLaunchAuthorityV1 for CopyOnlyQualificationAuthorityV1 {
    fn authorize_launch_v1(&self, _request: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

fn parse_unique_id(text: &str) -> Result<u64, Box<dyn Error>> {
    let unique_id = text
        .strip_prefix("0x")
        .map_or_else(|| text.parse::<u64>(), |hex| u64::from_str_radix(hex, 16))?;
    if unique_id == 0 {
        return Err("unique ID must be nonzero".into());
    }
    Ok(unique_id)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let unique_id = parse_unique_id(&arguments.next().ok_or(USAGE)?)?;
    if arguments.next().is_some() {
        return Err(USAGE.into());
    }

    // Native queue, VM, allocation, and SDMA custody is created and retained
    // exclusively on this child thread. Only address-free protocol data crosses
    // the process boundary.
    let mut backend =
        KfdRuntimeBackendV1::open_default(unique_id, CopyOnlyQualificationAuthorityV1)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve_runtime_backend_worker_v5_in_place(&mut backend, stdin.lock(), stdout.lock())?;

    // A successful serve return is possible only for the canonical empty
    // shutdown frame. The host must first release its logical handles through
    // V5; this call checks that ledger and then explicitly tears down native
    // storage. Any earlier EOF/error skips this transition, so live KFD custody
    // remains subject to the backend's fail-closed Drop.
    backend
        .shutdown_native_v1()
        .map_err(|error| format!("KFD Worker V5 native shutdown: {error:?}"))?;
    Ok(())
}
