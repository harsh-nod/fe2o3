# Multi-Device CPU Qualification Recovery

The qualification is a composition of **17 accepted original gates plus five
completion commands**, not a newly successful single-pass campaign. The original
18-command result remains rejected because its list parser rejected libtest's
singular `1 test` summary. The completion preserves that rejection, checks the
exact singleton grammar, runs its one CLI test, builds the normal example, and
checks a fresh closing attestation.

Results: 1,928 runtime tests passed, 32 native tests ignored, no failures; the
example CLI test passed. Strict runtime/example Clippy and no-default checking
passed. No GPU test, performance comparison, or formal proof rerun occurred.

`records.tar.gz` contains original command stdout/stderr, exit/group-closure
receipts, censuses, source inventories, result records, and three unchanged
reviewed parsers. It contains no executable or full source tree. SHA256:

```
7c38dc4869f16cd5c773882a7a4be8026ad3594d3ac455219536e41f526f2ed3  records.tar.gz
279a3ccc1aa70edbf708be09afac01b9d5b816078aa90bb35d52584dac883720  audit.py
5572b8f393837bf661c4f7c06b3bbed1815fcf37ceac62ca3d075089568db1a1  audit-result.json
```

Replay the public records in a new private directory:

```sh
mkdir records
tar -xzf records.tar.gz -C records
python3 -I -B audit.py records
```

This replays 23 recorded command closures, retaining the original rejected
classification, exact argv/environments, output hashes, actual test rosters,
source identity, Cargo artifact association, and the composed accepted gates.
Without the private full archive, it truthfully reports
`artifact_bytes_verified: false`. Metadata paths are joined to the pinned
source inventory, not reopened at nonexistent original host paths.

The complete recovery archive, retained locally and on `/mnt/c`, is
20,759,281 bytes with SHA256
`1000f7619015ab4cc46e060354eb698fa7ee0e9343a241784ca92a6e4f152217`.
It contains 148 regular files, including all three executable/dep-info pairs.
GNU tar `--hard-dereference` copied the normal Cargo executable's bytes into a
regular archive member without modifying its original two-link inode. The
same original relative path identifies the archived copy. Original size,
mode, inode, link count, device, and SHA256 inventories match before and after
the read-only transfer. Archive and extracted copies were independently checked.

`audit-result.json` records that full verification. To reproduce it with the
private recovery directory, use `--recovery RECOVERY`, where `RECOVERY` contains
`recovered.tar.gz`, `before.audit`, and `after.audit`. Supply the extracted full
archive as `records`. Optional `--repository REPOSITORY` compares all 6,594
source inventory entries and permits only the disclosed post-test status-document
update. The recorded repository check matches 6,593 files; only
`docs/runtime-a1-a2-swarm-current.md` changed after testing.

Frozen source record:
`b1ab447c92cb6a7bd51f549f6489c828db304144b41ff79f1665e5c51730689a`.
Original staged source tree: `6aaca91fba6a54920aac8822f4456fb96c90893b`.
The tested source is an unsigned 18-path overlay on signed base
`dd5e61712b3e7f16c43c7bf9ecb90eaac46bbaaf`; the base alone is not the tested source.

The earlier failed collection packets remain preserved separately, unchanged.
The source CPU root was not changed or deleted during recovery. A credential
pattern scan of every published record and parser found no private-key, GitHub
token, AWS access-key, or inline bearer/password/API-key credential matches.
