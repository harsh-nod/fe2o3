// Executable wire order shared by the native implementation and its proof.

macro_rules! distributed_codec_write_digest_body_v1 {
    ($expr:ident, $bytes:expr, $offset:expr, $value:expr) => {
        $expr! { { put($bytes, $offset, $value.as_bytes()); } }
    };
}

macro_rules! distributed_codec_read_digest_body_v1 {
    ($expr:ident, $bytes:expr, $offset:expr) => {
        $expr! { { Ok(IdentityDigestV1::from_untrusted_bytes(fixed($bytes, $offset)?)) } }
    };
}

macro_rules! distributed_codec_encode_operation_body_v1 {
    ($expr:ident, $check:ident, $binding:expr) => {
        $expr! {{
            let mut bytes = [0; DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1];
            let mut offset = 0;
            write_header(&mut bytes, &mut offset, DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1);
            let c = $binding.coordinates;
            write_digest(&mut bytes, &mut offset, c.runtime_instance.digest());
            write_digest(&mut bytes, &mut offset, c.participant.digest());
            write_u64(&mut bytes, &mut offset, c.participant_incarnation);
            write_digest(&mut bytes, &mut offset, c.coordinator.digest());
            write_u64(&mut bytes, &mut offset, c.coordinator_epoch);
            write_digest(&mut bytes, &mut offset, c.membership.digest());
            write_u64(&mut bytes, &mut offset, c.membership_epoch);
            write_digest(&mut bytes, &mut offset, c.run.digest());
            write_digest(&mut bytes, &mut offset, c.operation.digest());
            write_u64(&mut bytes, &mut offset, c.attempt);
            write_digest(&mut bytes, &mut offset, c.artifact.digest());
            write_digest(&mut bytes, &mut offset, c.execution_plan.digest());
            write_digest(&mut bytes, &mut offset, c.placement_plan.digest());
            write_digest(&mut bytes, &mut offset, c.target.digest());
            write_digest(&mut bytes, &mut offset, c.runtime_model.digest());
            $check!(offset, DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1);
            bytes
        }}
    };
}

macro_rules! distributed_codec_decode_operation_body_v1 {
    ($expr:ident, $bytes:expr) => {
        $expr! {{
            if $bytes.len() != DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1 {
                return Err(DistributedPublicationContractErrorV1::WrongLength);
            }
            let mut offset = 0;
            read_header($bytes, &mut offset, DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1)?;
            let coordinates = UntrustedDistributedOperationCoordinatesV1 {
                runtime_instance: DistributedRuntimeInstanceIdV1::from_untrusted_digest(
                    read_digest($bytes, &mut offset)?),
                participant: DistributedParticipantIdV1::from_untrusted_digest(
                    read_digest($bytes, &mut offset)?),
                participant_incarnation: read_u64($bytes, &mut offset)?,
                coordinator: DistributedParticipantIdV1::from_untrusted_digest(
                    read_digest($bytes, &mut offset)?),
                coordinator_epoch: read_u64($bytes, &mut offset)?,
                membership: DistributedMembershipIdV1::from_untrusted_digest(
                    read_digest($bytes, &mut offset)?),
                membership_epoch: read_u64($bytes, &mut offset)?,
                run: DistributedRunIdV1::from_untrusted_digest(read_digest($bytes, &mut offset)?),
                operation: DistributedOperationIdV1::from_untrusted_digest(
                    read_digest($bytes, &mut offset)?),
                attempt: read_u64($bytes, &mut offset)?,
                artifact: RuntimeArtifactIdV1::from_untrusted_digest(read_digest($bytes, &mut offset)?),
                execution_plan: DistributedExecutionPlanIdV1::from_untrusted_digest(
                    read_digest($bytes, &mut offset)?),
                placement_plan: DistributedPlacementPlanIdV1::from_untrusted_digest(
                    read_digest($bytes, &mut offset)?),
                target: DistributedTargetDescriptionIdV1::from_untrusted_digest(
                    read_digest($bytes, &mut offset)?),
                runtime_model: RuntimeModelIdV1::from_untrusted_digest(read_digest($bytes, &mut offset)?),
            };
            finish($bytes, offset)?;
            ModelDistributedOperationBindingV1::from_untrusted_coordinates(coordinates)
        }}
    };
}
