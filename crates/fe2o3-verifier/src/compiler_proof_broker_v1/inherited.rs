//! Retain both original input owners across every admission refusal.
use super::*;

pub(super) fn admit_pair(
    sources: [OwnedFd; 2],
    wrapper_sha256: [u8; 32],
) -> Result<(Endpoint, Peer)> {
    for source in &sources {
        require(
            rustix::io::fcntl_getfd(source)? == rustix::io::FdFlags::empty(),
            "compiler-proof input is not original inherited descriptor",
        )?;
        rustix::io::fcntl_setfd(source, rustix::io::FdFlags::CLOEXEC)?;
    }
    let [endpoint, broker] = sources;
    let endpoint = Endpoint::admit(endpoint)?;
    let broker = Peer::admit(broker, endpoint.creator, wrapper_sha256)?;
    Ok((endpoint, broker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::pipe::{PipeFlags, pipe_with};

    #[test]
    fn either_flag_or_endpoint_refusal_closes_both_original_sources() {
        super::super::tests::isolated_process_case(
            "compiler_proof_broker_v1::inherited::tests::either_flag_or_endpoint_refusal_closes_both_original_sources",
            || {
                for cloexec_index in [Some(0), Some(1), None] {
                    let (first_reader, first_writer) = pipe_with(PipeFlags::NONBLOCK).unwrap();
                    let (second_reader, second_writer) = pipe_with(PipeFlags::NONBLOCK).unwrap();
                    let sources = [first_writer, second_writer];
                    if let Some(index) = cloexec_index {
                        rustix::io::fcntl_setfd(&sources[index], rustix::io::FdFlags::CLOEXEC)
                            .unwrap();
                    }
                    let error = admit_pair(sources, [0; 32]).err().unwrap();
                    if cloexec_index.is_some() {
                        assert!(
                            error
                                .to_string()
                                .contains("not original inherited descriptor")
                        );
                    }
                    for reader in [first_reader, second_reader] {
                        assert_eq!(rustix::io::read(&reader, &mut [0]).unwrap(), 0);
                    }
                }
            },
        );
    }
}
