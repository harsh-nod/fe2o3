# Published Custodian Handoff Qualification

Date: 2026-10-04. Source:
`aafe2598d986f831550f32da4b51cb131b78923c`, based on
`cc6ab8311e445085f6fcc11b777570efd119d897`.

This qualifies authenticated sender-side pending application custody, not a
deployed proof manager, Ready/Activate handoff, retained remote proof, ordinary
multi-GPU launch or performance result.

## Results

- 306 CPU tests: broker 201, runtime protocol 48, compiler client 26, coordinator 31.
  The normal suites retain their explicit hardware/root/helper ignores.
- 63 doctests: broker 62, protocol 1, including opaque-owner and no-downgrade controls.
- Strict Clippy for broker/protocol, all targets; changed Rust source formatting.
- Five private real-root campaigns, 70 reported case groups: observation 38,
  legacy session 16, pending custodian handoff 8, registry transitions 6,
  both root/supervisor wire registration routes 2.
- Original source and test-binary hashes checked after qualification. Every private
  PID namespace was independently checked empty after its campaign.

The new cases require issuer binding and exact publication record plus EOF before
extraction. The same-Cargo alternate-pair rejection keeps the original pair alive,
then restores and successfully revalidates it. Compiler cancellation, exact issuer
exit and registry Drop leave the extracted original application alive; its queued
byte is received unchanged and a reply succeeds afterward. Dropping pending custody
then contains the original application.

Reattachment of the same live process rejects after extraction. The full-capacity
check uses a synthetic private reservation array and executes the production
capacity check; it is not sixteen actual deployed sessions. Identity reservations
remain conservative for the registry lifetime.

The positive handoff fixture emits a four-slot ACK to test historical descriptor
lifetime, without receiving production custodian Ready. The C fixture envelope is
deliberately noncanonical. No analyzer, Verus worker, proof controller, independently
approved manager or GPU is launched by these campaigns. No new formal proof is claimed.

## Reproduction And Cleanup

`evidence.tar.gz` contains the exact source patch, source/binary hashes, Cargo JSON,
raw logs, native review summary and scripts. Patch SHA-256:
`10e29d24dc224e86e680913d58089a68ea10d48a23aeaa8c8f020e5eac83971c`.

Archive SHA-256:
`e57142bb0b31fd0ab09690b4912e3590b269e194d4fb7efbb84756602f692fa2`.

Apply the patch to the source base or use the source commit. Scripts pin nightly
2026-04-03, offline locked dependencies, one test thread and four build jobs. Update
only their repository/scratch paths in a new owned directory. Run `qualify.sh`,
`root-campaign.sh` and `downstream.sh`. `package.sh` additionally checks the exact
recorded commit and patch hash.

Root qualification used WSL root only inside bwrap private PID/IPC/UTS/network
namespaces, a read-only host filesystem, private /tmp and narrowly selected
credential/inspection capabilities. It changed no installed service or cgroup.
MI300X was not used. Owned local capture scratch is removed after archiving;
unrelated workspace evidence and existing build caches are preserved.
