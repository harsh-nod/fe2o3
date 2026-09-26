#!/usr/bin/env python3
"""Retain this exact development scratch tree before removing owned build trees."""
import hashlib
import json
import os
from pathlib import Path
import runpy
import shutil
import stat
import sys

HERE = Path(__file__).resolve().parent
SCRATCH = Path("/home/harsh/.codex-tmp/fe2o3-mixed-duration-20260925-Fi4yH01q")
TARGETS = [
    Path("/dev/shm/fe2o3-mixed-duration-target-20260925-NohI5JUH"),
    Path("/dev/shm/fe2o3-mixed-duration-cold-20260925-sXxJH5mI"),
    Path("/dev/shm/fe2o3-mixed-duration-cold-v2-20260925-EJqXyU7y"),
]


def need(value, message):
    if not value:
        raise ValueError(message)


def save(path, data):
    with path.open("x") as output:
        json.dump(data, output, sort_keys=True, indent=2)
        output.write("\n")


def inventory(root):
    need(root.is_absolute() and root.resolve() == root and root.is_dir() and not root.is_symlink(), "ordinary owned root")
    result = {}
    device = root.stat().st_dev
    for path in [root, *sorted(root.rglob("*"))]:
        info = path.lstat()
        need(info.st_uid == os.getuid(), "owned entry")
        need(info.st_dev == device, "no nested filesystem")
        name = str(path.relative_to(root))
        row = dict(device=info.st_dev, inode=info.st_ino, uid=info.st_uid, mode=info.st_mode,
                   bytes=info.st_size, allocated=info.st_blocks * 512)
        if stat.S_ISREG(info.st_mode):
            row["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
        elif stat.S_ISLNK(info.st_mode):
            need(name in ("cargo-home/registry", "cargo-home/git"), "only private Cargo cache links")
            row["link"] = os.readlink(path)
            need(row["link"] == "/home/harsh/.cargo/" + path.name, "expected cache link")
        else:
            need(stat.S_ISDIR(info.st_mode), "regular files/directories only")
        result[name] = row
    return result


def no_groups(records):
    for record in records:
        need(record.get("group_absent") is True and type(record.get("status")) is int, "terminal process receipt")
        need(type(record.get("process_group")) is int and record["process_group"] > 1, "exact positive process group")
        need(type(record.get("started_ns")) is int and type(record.get("finished_ns")) is int
             and record["finished_ns"] >= record["started_ns"], "terminal interval")
        try:
            os.killpg(record["process_group"], 0)
        except ProcessLookupError:
            continue
        raise ValueError("recorded process group is still present")


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    need(shutil.rmtree.avoids_symlink_attacks, "descriptor-relative cleanup")
    verify = runpy.run_path(str(HERE / "verify.py"))["verify"]
    verified = verify(SCRATCH / "signed-cpu-v2")
    records = [json.loads(path.read_text()) for path in SCRATCH.rglob("record.json")]
    no_groups(records)
    before = {str(root): inventory(root) for root in [SCRATCH, *TARGETS]}
    need(not any("link" in row for row in before[str(SCRATCH)].values()), "scratch contains no links")
    destination = HERE / "retained"
    need(not destination.exists(), "fresh retained tree")
    shutil.copytree(SCRATCH, destination, symlinks=True)
    retained = inventory(destination)
    original_files = {name: row["sha256"] for name, row in before[str(SCRATCH)].items() if "sha256" in row}
    retained_files = {name: row["sha256"] for name, row in retained.items() if "sha256" in row}
    need(original_files == retained_files, "byte-exact full scratch retention")
    need(verify(destination / "signed-cpu-v2") == verified, "independent retained replay")
    save(HERE / "retention.json", dict(files=retained_files, verification=verified, records=len(records)))
    save(HERE / "cleanup-before.json", before)
    no_groups(records)
    need({str(root): inventory(root) for root in [SCRATCH, *TARGETS]} == before, "immediate owned-tree continuity")
    for root in [*TARGETS, SCRATCH]:
        shutil.rmtree(root)
    no_groups(records)
    need(all(not root.exists() and not root.is_symlink() for root in [SCRATCH, *TARGETS]), "independent path absence")
    need({name: hashlib.sha256((destination / name).read_bytes()).hexdigest() for name in retained_files} == retained_files,
         "retention survives cleanup")
    unique_allocations = {(row["device"], row["inode"]): row["allocated"] for tree in before.values() for row in tree.values()}
    result = dict(removed=[str(root) for root in [SCRATCH, *TARGETS]],
                  inode_accounted_allocated_bytes=sum(unique_allocations.values()),
                  retained_files=len(retained_files), groups_absent=True, paths_absent=True,
                  remote_resources_created=False)
    save(HERE / "cleanup-after.json", result)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
