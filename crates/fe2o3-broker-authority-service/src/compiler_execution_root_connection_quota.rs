use super::*;

/// Additional peak above all retained inputs, and cumulative logical work.
/// No fresh account, storage reservation, deadline or authority is supplied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootConnectionQuotaV3 {
    work: usize,
    scratch: usize,
}
impl RootConnectionQuotaV3 {
    pub const fn work(&self) -> usize {
        self.work
    }
    pub const fn scratch(&self) -> usize {
        self.scratch
    }
}

fn peer() -> Result<RootConnectionQuotaV3> {
    Ok(RootConnectionQuotaV3 {
        work: sum(&[
            LOCAL_WORK,
            2 * PlainChild::OPERATION_WORK,
            Namespaces::REVALIDATE_PROCESS_WORK,
            ENTRY,
            observations::PROCESS_VALIDATE_WORK,
        ])?,
        scratch: sum(&[
            FRAME,
            2 * PlainChild::OPERATION_SCRATCH,
            Namespaces::REVALIDATE_PROCESS_SCRATCH,
            observations::PROCESS_VALIDATE_SCRATCH,
        ])?,
    })
}

impl RootConnectionV3<'_> {
    /// Worst-case finite handshake including original trace checks, exact-child
    /// observations, all transport attempts and two actual image measurements.
    /// Keep root session, original trace, actual issuer, policy/manifest/readiness
    /// and incoming channel inputs prepaid independently. The returned full owner
    /// charge replaces the consumed channel charge only after successful return.
    /// The bound is deliberately conservative: unused attempts are NOT charged.
    pub fn handshake_quota(image_length: u64) -> Result<RootConnectionQuotaV3> {
        let image = retained_issuer_image_quota_v3(image_length)?;
        handshake(image.work(), image.scratch(), Self::MAX_HANDSHAKE_ATTEMPTS)
    }

    /// One validation of an already challenge-completed connection. Input floors
    /// remain prepaid; this does not fund publication, retirement or a new RPC.
    pub fn validation_quota(image_length: u64) -> Result<RootConnectionQuotaV3> {
        let image = retained_issuer_image_quota_v3(image_length)?;
        let peer = peer()?;
        Ok(RootConnectionQuotaV3 {
            work: sum(&[
                LOCAL_WORK,
                RootControlSessionV3::VALIDATE_WORK,
                CODEC_WORK,
                Channel::WORK,
                peer.work,
                image.work(),
            ])?,
            scratch: sum(&[
                FRAME,
                RootControlSessionV3::VALIDATE_SCRATCH,
                CODEC_SCRATCH,
                Channel::SCRATCH,
                peer.scratch,
                image.scratch(),
            ])?,
        })
    }
}

fn handshake(
    image_work: usize,
    image_scratch: usize,
    attempts: usize,
) -> Result<RootConnectionQuotaV3> {
    if attempts == 0 || attempts > RootConnectionV3::MAX_HANDSHAKE_ATTEMPTS {
        return Err(Error::Refused("invalid root handshake attempt quote"));
    }
    let peer = peer()?;
    let one = sum(&[
        LOCAL_WORK,
        RootControlSessionV3::VALIDATE_WORK,
        peer.work,
        Channel::PACKET_WORK,
        CODEC_WORK,
        GATE_WORK,
    ])?;
    let repeated = attempts.checked_mul(one).ok_or(Resource::Arithmetic)?;
    Ok(RootConnectionQuotaV3 {
        work: sum(&[
            LOCAL_WORK,
            2 * RootControlSessionV3::VALIDATE_WORK,
            Channel::WORK,
            READY_WORK,
            PlainChild::OPERATION_WORK,
            2 * peer.work,
            image_work,
            image_work,
            CODEC_WORK,
            GATE_WORK,
            Replay::WORK,
            repeated,
        ])?,
        // The constructor scope retains new descriptors and codec owners through
        // return. Charge conservative codec/window frames as their owner ceilings.
        // Iterations are sequential: scratch does not multiply by attempts.
        scratch: sum(&[
            FRAME,
            FD_STORAGE,
            3 * CODEC_SCRATCH,
            Replay::STORAGE,
            Replay::SCRATCH,
            RootControlSessionV3::VALIDATE_SCRATCH,
            Channel::SCRATCH,
            READY_SCRATCH,
            PlainChild::OPERATION_SCRATCH,
            peer.scratch,
            image_scratch,
            GATE_SCRATCH,
            FRAME,
            RootControlSessionV3::VALIDATE_SCRATCH,
            peer.scratch,
            Channel::PACKET_SCRATCH,
            BYTES,
            CODEC_SCRATCH,
            GATE_SCRATCH,
        ])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_quote_covers_all_attempts_and_rejects_arithmetic_overflow() {
        let one = handshake(31, 37, 1).unwrap();
        let many = handshake(31, 37, RootConnectionV3::MAX_HANDSHAKE_ATTEMPTS).unwrap();
        let p = peer().unwrap();
        let attempt = LOCAL_WORK
            + RootControlSessionV3::VALIDATE_WORK
            + p.work
            + Channel::PACKET_WORK
            + CODEC_WORK
            + GATE_WORK;
        assert_eq!(
            many.work - one.work,
            (RootConnectionV3::MAX_HANDSHAKE_ATTEMPTS - 1) * attempt
        );
        assert_eq!(many.scratch, one.scratch);
        let larger = handshake(131, 137, 1).unwrap();
        assert_eq!(
            (larger.work - one.work, larger.scratch - one.scratch),
            (200, 100)
        );
        for args in [(usize::MAX, 0, 1), (0, usize::MAX, 1)] {
            assert!(matches!(
                handshake(args.0, args.1, args.2),
                Err(Error::Resource(Resource::Arithmetic))
            ));
        }
        for attempts in [0, RootConnectionV3::MAX_HANDSHAKE_ATTEMPTS + 1, usize::MAX] {
            assert!(matches!(handshake(1, 1, attempts), Err(Error::Refused(_))));
        }
    }

    #[test]
    fn public_quotes_validate_image_length_before_any_process_or_io() {
        for length in [0, u64::MAX] {
            assert!(RootConnectionV3::handshake_quota(length).is_err());
            assert!(RootConnectionV3::validation_quota(length).is_err());
        }
        let image = retained_issuer_image_quota_v3(4096).unwrap();
        let handshake = RootConnectionV3::handshake_quota(4096).unwrap();
        let validation = RootConnectionV3::validation_quota(4096).unwrap();
        assert!(handshake.work() > validation.work());
        assert!(handshake.scratch() > validation.scratch());
        assert!(validation.work() >= image.work() + RootControlSessionV3::VALIDATE_WORK);
        assert!(validation.scratch() >= image.scratch() + RootControlSessionV3::VALIDATE_SCRATCH);
    }
}
