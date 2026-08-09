# Parity Row Evidence V1 (Legacy)

V1 row records and manifests are not promotion authority. They used
candidate-committed SHA-256 checksums and therefore proved archive integrity
only after the candidate had chosen both the bytes and checksum. They did not
establish who ran a test or who reviewed a Complete claim.

The V1 policy file has been removed. It covered only rows that happened to be
Missing when generated and assigned identical requirements to Partial and
Complete. It must not be reconstructed or used by CI.

Existing V1 result records remain readable through:

    scripts/parity-evidence.sh verify-record --archive-only \
      --archive-root ARCHIVE RECORD

That command verifies internal digests only. It cannot promote a parity row,
and the dashboard no longer accepts V1 promotion options.

New evidence must use protected-base public-key and row policies,
runner-signed V2 result attestations, signed MI300X queue manifests for
hardware, separately signed reviewer authorization for Complete, and the
promotion gate documented in
[Signed Parity Evidence V2](parity-signed-evidence-v2.md).
