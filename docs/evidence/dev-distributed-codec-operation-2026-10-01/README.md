# Operation Codec Qualification

This packet qualifies the shared executable operation encoder/decoder at signed
candidate `d0d24274c69043ae1205376489cf41179740a564`. It does not close A2,
distributed transport, hardware qualification, or HIP/HSA parity.

## Results

- All 47 original qualification stages and owned process groups close.
- Three full-root positives verify 99 obligations with zero errors, including
  relocation and the closing positive. All eleven proof inputs are bound.
- Forty fresh actual-body mutations produce the required logical failures.
  Resource failures, frontend failures and historical runs are not counted.
- Independent agent and primary-agent readbacks agree on signed source,
  commands, raw receipts, diagnostics, tool identities and closure records.
- Native CPU evidence is explicitly reused: 1,130 passing tests, 19 existing
  ignores, 21 focused release tests and strict Clippy. The 317 actual compiler
  inputs and sixteen retained executables are unchanged; this packet did not
  rebuild or rerun them.
- All 27 integrated source-workflow commands pass in a fresh clean environment.
  Existing guard changes are metadata rebinding; inherited primitive, field,
  queued-query and journal-observer proof inputs remain exact.

## Contents

`source-and-campaign.tar.gz` is the exact durable archive: 9,680 members,
8,054 ordinary files, signed source and complete qualification records.
Its SHA256 is `5d88b07174d9c0d4731faafe340b28d84fec5b4fc386714ff73eb1ea405d6426`.
`campaign-members.json`, `custody.json` and `independent-readback.json` bind
member contents/modes, durable readback and the independent audit.

`signed-source.bundle` is incremental and requires parent
`da4ff4019ebad1aecde1bdff3942f7f4e0383e33`. `integration-records.tar.gz`
contains clean integration attempt 3, its owner and packaging support. Every
receipt, stdout and stderr buffer is joined to the saved 27-group census.
`manifest.json` records exact output hashes, source locations and exclusions.
Status-only roadmap updates after this check record the unrelated A2/native
resource-gate rejections; no tested runtime or proof source changed.

This is not a self-contained replay environment. Toolchains, OS/loader state,
prior CPU executables and some inherited historical helper/source prerequisites
remain external. Absolute original paths are recorded as provenance, not as
portable replay instructions. Packaging replays no archived command and probes
no historical PID.

## Publication Hygiene

Two earlier local integration attempts inherited an authentication header in
their recorded environment. They were not published. Their unmodified raw
records remain private and are excluded from this packet; attempt 3 uses an
explicit environment allowlist and passes all source controls independently.

A read-only scan of existing evidence and decoded archives found no matches
for the current sensitive environment values. Five generic `secret_scan`
fields in an older snapshot-fixture packet were independently classified as
descriptive scan-policy metadata, using exact member/value hashes. This is
not a universal historical credential clearance: literal JSON-field scanning
has documented limits and does not decode compressed Git bundles. The new
source archive, clean integration records and copied metadata pass the scoped
scan and exact-buffer publication checks.

The signed source contains two already-public, test-only issuer-key fixtures.
Only their exact paths, SHA256 hashes and Git blob identities are allowlisted
in the manifest; no general private-key exemption applies.
