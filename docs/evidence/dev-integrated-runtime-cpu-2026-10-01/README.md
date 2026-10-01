# Combined Runtime CPU Regression

Signed source: `1cfb7580f36090c41cd9f96141c5d6b2952750ab`.

All 15 stages passed in one fresh campaign: signature brackets, parser and
workspace controls, tool identities, locked/offline metadata, no-default library
check, all-feature build, complete and ignored rosters, full runtime tests, and
strict all-target/all-feature Clippy. The fresh executable ran **1,901 passing
tests, 32 existing hardware ignores, zero failures and zero filtered tests**.
The complete roster has 1,933 tests. All 15 owned process groups closed.

An independent readback checked the 6,558 signed source inputs, explicit helper
and tool identities, fresh executable, raw command receipts and streams, exact
rosters and serial test classification. It did not rerun the tests or probe
historical PIDs. This closes the combined CPU regression needed after the codec
and journal-wrapper integrations, not an A1/A2 or HIP/HSA parity milestone.

## Records

- [records.tar.gz](records.tar.gz): all command streams and receipts, input and
  result records, full rosters, independent readback, controllers and pinned
  parser/support sources.
- [manifest.json](manifest.json): every archived member's original path, size,
  mode and SHA-256, plus the retained executable's identity.
- Archive SHA-256:
  `89b32c4d8ea797f27900e8126e33ad969aefece9ae879adaf376f8ea5ac965eb`.
- Executable SHA-256:
  `408f21efc4bbfd91c7a45b1921b514e1af1c6886cac259266a1d4435618aaf01`.

Every archive member was read back and compared with its original. The signed
source is available at the recorded Git commit; the 43,605,904-byte executable
remains in the original local records and is not in this compact archive.
Toolchain, registry, system libraries and OS are not bundled or hermetically
attested. Controllers retain original absolute paths and require explicit path
mapping for relocation; this is not a turnkey portable replay package.

Disk, storage-budget and elapsed-time checks occurred at stage boundaries, not
continuously. There was no automatic retry, GPU execution, new solver run,
hardware-ignore removal or performance qualification in this campaign.
