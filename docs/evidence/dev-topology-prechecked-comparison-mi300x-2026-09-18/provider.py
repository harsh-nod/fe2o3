"""Strict read-only topology-summary parser for the declared eight-card host."""

import re

MAX_BYTES = 32768
HEADER = "root generation boot_id kernel_release module_version module_srcversion platform nodes gpus".split()
GPU = "node gpu_id target render_minor unique_id hive_id pci pci_revision partition domain location_id fw_version sdma_fw_version wavefront simds xccs".split()
U32 = (1 << 32) - 1
U64 = (1 << 64) - 1


def need(value, message):
    if not value:
        raise RuntimeError(message)


def fields(line, keys):
    need(
        len(line) <= 2048 and all(32 <= ord(c) < 127 for c in line), "bounded ASCII row"
    )
    parts = line.split(" ")
    need(len(parts) == len(keys), "exact provider field count")
    result = {}
    for part, expected in zip(parts, keys):
        need(part.count("=") == 1, "provider key/value framing")
        key, value = part.split("=")
        need(key == expected and value, "exact ordered provider keys")
        result[key] = value
    return result


def unsigned(value, maximum=U64, minimum=0):
    need(re.fullmatch(r"0|[1-9][0-9]{0,19}", value), "canonical provider decimal")
    result = int(value)
    need(minimum <= result <= maximum, "bounded provider decimal")
    return result


def parse(data, roster):
    need(type(data) is bytes and 0 < len(data) <= MAX_BYTES, "bounded provider output")
    need(
        data.endswith(b"\n") and b"\r" not in data and b"\0" not in data,
        "provider framing",
    )
    try:
        lines = data.decode("utf-8").split("\n")[:-1]
    except UnicodeDecodeError as error:
        raise RuntimeError("provider UTF-8") from error
    need(len(lines) == 9, "one header and eight GPU rows")
    header = fields(lines[0], HEADER)
    need(header["root"] == "/sys/class/kfd/kfd/topology", "real KFD topology root")
    header["generation"] = unsigned(header["generation"], minimum=1)
    need(
        re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", header["boot_id"]),
        "canonical boot UUID",
    )
    need(
        re.fullmatch(r"[A-Za-z0-9._+-]{1,128}", header["kernel_release"]),
        "bounded kernel release",
    )
    for key, grammar in (
        ("module_version", r"[A-Za-z0-9._+-]{1,128}"),
        ("module_srcversion", r"[A-F0-9]{1,128}"),
    ):
        need(
            header[key] == "<absent>" or re.fullmatch(grammar, header[key]),
            "bounded module identity",
        )
    platform = header["platform"].split(":")
    need(len(platform) == 3, "three platform fields")
    header["platform"] = [
        unsigned(platform[0], minimum=1),
        unsigned(platform[1], minimum=1),
        unsigned(platform[2], maximum=U32),
    ]
    header["nodes"] = unsigned(header["nodes"], 10, 10)
    header["gpus"] = unsigned(header["gpus"], 8, 8)
    need(
        len(roster) == 8 and [r[0] for r in roster] == list(range(8)),
        "complete declared card roster",
    )
    expected = {(bdf, int(uid, 16)) for _, bdf, uid in roster}
    need(len(expected) == 8, "distinct declared physical cards")
    rows = []
    limits = {
        "node": (0, 9),
        "gpu_id": (1, U32),
        "render_minor": (128, 255),
        "unique_id": (1, U64),
        "hive_id": (0, U64),
        "domain": (0, 65535),
        "location_id": (0, U32),
        "fw_version": (0, U32),
        "sdma_fw_version": (0, U32),
        "wavefront": (64, 64),
        "simds": (1216, 1216),
        "xccs": (8, 8),
    }
    for line in lines[1:]:
        row = fields(line, GPU)
        for key, (minimum, maximum) in limits.items():
            row[key] = unsigned(row[key], maximum, minimum)
        need(
            row["target"] == "gfx942" and row["partition"] == "SPX/NPS1",
            "declared MI300X profile",
        )
        need(
            re.fullmatch(r"0x[0-9a-f]{2}", row["pci_revision"]),
            "canonical PCI revision",
        )
        pci = re.fullmatch(
            r"([0-9a-f]{4}):([0-9a-f]{2}):([01][0-9a-f])\.([0-7])", row["pci"]
        )
        need(pci, "canonical PCI address")
        domain, bus, device, function = (int(value, 16) for value in pci.groups())
        need(
            row["domain"] == domain
            and row["location_id"] == (bus << 8 | device << 3 | function),
            "PCI/KFD correlation",
        )
        rows.append(row)
    for key in ("node", "gpu_id", "render_minor", "unique_id", "pci"):
        need(len({row[key] for row in rows}) == 8, "unique provider " + key)
    need(
        [r["node"] for r in rows] == sorted(r["node"] for r in rows),
        "ordered GPU nodes",
    )
    need(
        {(r["pci"], r["unique_id"]) for r in rows} == expected,
        "exact complete physical GPU roster",
    )
    return {"header": header, "gpus": rows}
