"""Independent, read-only audit of signed R65 copy-graph/idle-drain evidence."""
import hashlib
import io
import json
import pathlib
import re
import subprocess
import sys
import tarfile

capture, repo, trusted_signers = map(pathlib.Path, sys.argv[1:4])
commit = sys.argv[4]
assert re.fullmatch(r"[0-9a-f]{40}", commit)


def digest(value):
    return hashlib.sha256(value).hexdigest()


with tarfile.open(capture) as archive:
    members = archive.getmembers()
    prefixes = {member.name + '/' for member in members
                if member.isdir() and re.fullmatch(r'output/r65-owner-[0-9a-f]{32}', member.name)}
    assert len(prefixes) == 1
    prefix = prefixes.pop()
    assert len({member.name for member in members}) == len(members)
    assert all(member.isfile() or member.isdir() for member in members)
    payload = {member.name[len(prefix):]: archive.extractfile(member).read()
               for member in members if member.isfile() and member.name.startswith(prefix)}
    assert archive.extractfile("exit-status.txt").read() == b"0\n"
manifest = json.loads(payload.pop("sha256.json"))
assert manifest == {name: digest(value) for name, value in payload.items()}
provenance = json.loads(payload["provenance.json"])
assert provenance["source_commit"] == commit
assert provenance["qualification_runs"] == 2
assert provenance["performance_claim"] is False
assert provenance["source_archive_sha256"] == digest(payload["source.tar"])
assert provenance["binary_sha256"]["kfd"] == digest(payload["owner-binary"])
for name, expected in provenance["snapshot_input_sha256"].items():
    assert digest(payload[name]) == expected
git = ["git", "-C", str(repo)]
subprocess.run(git + ["-c", "gpg.format=ssh", "-c",
    f"gpg.ssh.allowedSignersFile={trusted_signers}", "verify-commit", commit], check=True)
assert payload["signed-commit.txt"] == subprocess.check_output(git + ["cat-file", "commit", commit])
fresh_source = subprocess.check_output(git + ["archive", "--format=tar", commit])
assert fresh_source == payload["source.tar"]
with tarfile.open(fileobj=io.BytesIO(fresh_source)) as archive:
    source_files = {member.name: digest(archive.extractfile(member).read())
                    for member in archive.getmembers() if member.isfile()}
assert source_files == json.loads(payload["source-files.json"])
commands = json.loads(payload["commands.json"])
assert len(commands) == 32
assert all(command["returncode"] == 0 for command in commands)
expected_output = ("PASS schema=fe2o3.runtime.r65-graph-versions-idle-drain.v1 bytes=1048832 "
                   "owner_threads=1 streams=4 nodes=12 copies=5 executions=2 "
                   "versions=21 version_inputs=10 current_versions=13 occurrences=distinct "
                   "joins=host canaries=complete submissions=released drain=idle-quiescent "
                   "admission=closed cleanup=complete\n").encode()
for index in range(2):
    assert payload[f"owner-{index}.jsonl"] == expected_output
print(json.dumps({"capture_sha256": digest(capture.read_bytes()), "source_commit": commit,
    "manifest_payloads": len(manifest), "source_files": len(source_files),
    "source_archive_matches_fresh_git_archive": True, "trusted_signature_verified": True,
    "successful_commands": len(commands), "exact_passes": 2,
    "graphs_per_pass": 2, "drain_scope": "idle-quiescent", "performance_claim": False,
    "binary_sha256": digest(payload["owner-binary"]), "binary_bytes": len(payload["owner-binary"])},
    sort_keys=True))
