//! One receive/transition/send schedule for legacy and native custody.
use crate::service::{
    ExternalAnchorDaemonErrorV1 as Error, ExternalAnchorServiceReportV1 as Report,
    ServiceBoundaryV1 as Boundary, ServiceHooksV1, service_checkpoint,
};
use fe2o3_external_anchor_protocol::ANCHOR_CHALLENGE_WIRE_LEN_V1;
use std::{os::fd::OwnedFd, time::Duration};

pub(crate) trait PeerSession {
    type Error: From<Error>;
    type Observation;
    fn receive(
        &mut self,
        peer: &OwnedFd,
    ) -> Result<Option<[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1]>, Self::Error>;
    fn exchange(
        &mut self,
        challenge: &[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1],
    ) -> Result<Self::Observation, Self::Error>;
    fn send(
        &mut self,
        peer: &OwnedFd,
        observation: &Self::Observation,
        timeout: Duration,
    ) -> Result<(), Self::Error>;
    fn retire(&mut self, observation: Self::Observation) -> Result<(), Self::Error>;
}

pub(crate) fn run<S: PeerSession>(
    session: &mut S,
    peer: &OwnedFd,
    response_timeout: Duration,
    hooks: &mut impl ServiceHooksV1,
) -> Result<Report, S::Error> {
    run_from_count(session, peer, response_timeout, hooks, 0)
}

fn run_from_count<S: PeerSession>(
    session: &mut S,
    peer: &OwnedFd,
    response_timeout: Duration,
    hooks: &mut impl ServiceHooksV1,
    mut exchanges: u64,
) -> Result<Report, S::Error> {
    if response_timeout.is_zero() {
        return Err(Error::InvalidResponseTimeout.into());
    }
    loop {
        service_checkpoint(hooks, Boundary::BeforeReceive)?;
        let Some(challenge) = session.receive(peer)? else {
            return Ok(Report { exchanges });
        };
        service_checkpoint(hooks, Boundary::AfterReceive)?;
        service_checkpoint(hooks, Boundary::BeforeExchange)?;
        let observation = session.exchange(&challenge)?;
        service_checkpoint(hooks, Boundary::AfterExchange)?;
        service_checkpoint(hooks, Boundary::BeforeSend)?;
        session.send(peer, &observation, response_timeout)?;
        service_checkpoint(hooks, Boundary::AfterSend)?;
        session.retire(observation)?;
        exchanges = exchanges
            .checked_add(1)
            .ok_or(Error::ExchangeCountOverflow)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::NoopServiceHooksV1;

    #[derive(Default)]
    struct Session {
        received: usize,
        sent: usize,
        retired: usize,
    }
    impl PeerSession for Session {
        type Error = Error;
        type Observation = ();
        fn receive(
            &mut self,
            _: &OwnedFd,
        ) -> Result<Option<[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1]>, Error> {
            self.received += 1;
            Ok(Some([0; ANCHOR_CHALLENGE_WIRE_LEN_V1]))
        }
        fn exchange(&mut self, _: &[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1]) -> Result<(), Error> {
            Ok(())
        }
        fn send(&mut self, _: &OwnedFd, _: &(), _: Duration) -> Result<(), Error> {
            self.sent += 1;
            Ok(())
        }
        fn retire(&mut self, _: ()) -> Result<(), Error> {
            self.retired += 1;
            Ok(())
        }
    }

    #[test]
    fn zero_timeout_rejects_before_receive() {
        let peer = std::fs::File::open("/dev/null").unwrap().into();
        let mut s = Session::default();
        assert!(matches!(
            run(&mut s, &peer, Duration::ZERO, &mut NoopServiceHooksV1),
            Err(Error::InvalidResponseTimeout)
        ));
        assert_eq!((s.received, s.sent, s.retired), (0, 0, 0));
    }

    #[test]
    fn counter_overflow_retires_the_sent_response_and_never_wraps() {
        let peer = std::fs::File::open("/dev/null").unwrap().into();
        let mut s = Session::default();
        assert!(matches!(
            run_from_count(
                &mut s,
                &peer,
                Duration::from_secs(1),
                &mut NoopServiceHooksV1,
                u64::MAX
            ),
            Err(Error::ExchangeCountOverflow)
        ));
        assert_eq!((s.received, s.sent, s.retired), (1, 1, 1));
    }
}
