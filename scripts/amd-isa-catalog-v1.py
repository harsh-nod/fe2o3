#!/usr/bin/env python3
"""Offline, pinned public AMD metadata import. No extraction or execution."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import sys
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tools/amd-isa-catalog-v1"
RUST = ROOT / "crates/fe2o3-amdgcn-model/src"
SCHEMA = "fe2o3-amd-isa-spec-catalog-v1"
MAX_XML = 9 * 1024 * 1024
MAX_NODES = 140_000
MAX_DEPTH = 48
MAX_OUTPUT = 8 * 1024 * 1024
SECTIONS = {
    "encodings": ("Encodings", "Encoding", "EncodingName"),
    "instructions": ("Instructions", "Instruction", "InstructionName"),
    "data_formats": ("DataFormats", "DataFormat", "DataFormatName"),
    "operand_types": ("OperandTypes", "OperandType", "OperandTypeName"),
    "functional_groups": ("FunctionalGroups", "FunctionalGroup", "Name"),
    "functional_subgroups": ("FunctionalSubgroups", "FunctionalSubgroup", "Name"),
}
FLAGS = ("IsBranch", "IsConditionalBranch", "IsIndirectBranch",
         "IsProgramTerminator", "IsImmediatelyExecuted")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def json_bytes(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def read_regular(path, limit):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        info = os.fstat(fd)
        require(stat.S_ISREG(info.st_mode), "input is not a regular file")
        require(0 <= info.st_size <= limit, "input size exceeds bound")
        result = bytearray()
        while len(result) <= limit:
            part = os.read(fd, min(65536, limit + 1 - len(result)))
            if not part:
                break
            result.extend(part)
        require(len(result) <= limit, "input grew beyond bound")
        return bytes(result)
    finally:
        os.close(fd)


def no_duplicate_json(pairs):
    out = {}
    for key, value in pairs:
        require(key not in out, "duplicate JSON key")
        out[key] = value
    return out


def read_json(path):
    return decode_json(read_regular(path, 65536))


def decode_json(raw):
    return json.loads(raw.decode("utf-8"),
                      object_pairs_hook=no_duplicate_json)


def validate_coverage(coverage):
    require(coverage["schema"] == "fe2o3-amd-isa-reviewed-coverage-v1"
            and coverage["target"] == "gfx942" and coverage["scalar"] == "u32"
            and coverage["authority"] is False, "coverage profile")
    require(coverage["instruction_names"] ==
            ["V_ADD_U32", "V_AND_B32", "V_MOV_B32", "V_OR_B32", "V_SUB_U32", "V_XOR_B32"],
            "coverage must be the exact reviewed six source marker spellings")
    for key in ("encoding_selection", "encoding_level_authoring", "encoding_level_simulation",
                "encoding_level_proof", "hardware_qualification"):
        require(coverage[key] == "unavailable", "coverage must not grant encoding stages")
    for key in ("implicit_EXEC_semantics", "register_resource_legality"):
        require(coverage[key] == "not_established_by_catalog", "coverage must not infer semantics")


def checked_archive(raw, manifest):
    pin = manifest["archive"]
    require(len(raw) == pin["bytes"] and sha(raw) == pin["sha256"],
            "archive size/hash mismatch")
    archive = zipfile.ZipFile(io.BytesIO(raw))
    entries = archive.infolist()
    require(len(entries) == pin["member_count"] == 10, "ZIP member count")
    seen = set()
    for item in entries:
        require(item.filename not in seen, "duplicate ZIP member")
        seen.add(item.filename)
        require(re.fullmatch(r"amdgpu_isa_(?:cdna[1-5]|rdna[1-4]|rdna3_5)\.xml",
                             item.filename) is not None, "unexpected ZIP member")
        require(not item.is_dir() and not item.flag_bits & 1, "ZIP entry kind")
        require(item.compress_type == zipfile.ZIP_DEFLATED, "ZIP compression")
        mode = item.external_attr >> 16
        require(not stat.S_ISLNK(mode), "ZIP symlink")
        require(0 < item.file_size <= 18 * 1024 * 1024
                and 0 < item.compress_size <= len(raw), "ZIP member bounds")
    result = {}
    for pin in manifest["members"]:
        item = archive.getinfo(pin["member"])
        require(item.file_size == pin["bytes"] <= MAX_XML, "XML member size")
        with archive.open(item) as stream:
            content = stream.read(pin["bytes"] + 1)
        require(len(content) == pin["bytes"] and sha(content) == pin["sha256"],
                "XML member hash")
        result[pin["architecture"]] = content
    archive.close()
    return result


def parse_xml(raw):
    require(len(raw) <= MAX_XML, "XML bound")
    text = raw.decode("utf-8", errors="strict")
    require(not re.search(r"<!\s*(?:DOCTYPE|ENTITY)", text, re.I), "XML declarations")
    root = ET.fromstring(text)
    stack = [(root, 1)]
    count = 0
    while stack:
        node, depth = stack.pop()
        count += 1
        require(count <= MAX_NODES and depth <= MAX_DEPTH, "XML structural bound")
        require(isinstance(node.tag, str) and "{" not in node.tag, "XML namespace")
        require(len(node.attrib) <= 16 and len(node.text or "") <= 262144,
                "XML value bound")
        require(not (node.tail or "").strip(), "XML mixed tail")
        require(not len(node) or not (node.text or "").strip(), "XML mixed text")
        stack.extend((child, depth + 1) for child in node)
    return root


def one(node, tag):
    found = node.findall(tag)
    require(len(found) == 1, "missing or duplicate " + tag)
    return found[0]


def text(node, tag):
    item = one(node, tag)
    require(not len(item), "non-leaf " + tag)
    return (item.text or "").strip()


def number(value, maximum=2**32 - 1):
    require(re.fullmatch(r"[0-9]+", value) is not None, "invalid unsigned number")
    result = int(value)
    require(result <= maximum, "number range")
    return result


def unique(nodes, key):
    result = {}
    for node in nodes:
        name = text(node, key)
        require(name and name not in result, "duplicate/empty " + key)
        result[name] = node
    return result


def boolean(value, uppercase=False):
    allowed = ("FALSE", "TRUE") if uppercase else ("false", "true")
    require(value in allowed, "invalid boolean")
    return value == allowed[1]


def validate(root, pin, manifest):
    require(root.tag == "Spec" and [c.tag for c in root] == ["Document", "ISA"],
            "unexpected document root")
    document = one(root, "Document")
    require({n.tag: (n.text or "").strip() for n in document} == manifest["document"]
            and len(document) == len(manifest["document"]), "document identity")
    isa = one(root, "ISA")
    require(text(one(isa, "Architecture"), "ArchitectureName") ==
            pin["architecture_name"], "architecture name")
    require({c.tag for c in isa} == {"Architecture"} |
            {s[0] for s in SECTIONS.values()} and len(isa) == 7, "ISA sections")
    sections = {}
    for key, (container, tag, name) in SECTIONS.items():
        parent = one(isa, container)
        require(all(n.tag == tag for n in parent), "section node kind")
        sections[key] = unique(parent, name)
        if key in pin:
            require(len(parent) == pin[key], "section count " + key)
    aliases = {}
    canonical = sections["instructions"]
    for name, instruction in canonical.items():
        for alias in instruction.findall("AliasedInstructionNames/InstructionName"):
            value = (alias.text or "").strip()
            require(value and value not in aliases and value not in canonical,
                    "duplicate or colliding alias")
            aliases[value] = name
    conditions = {}
    fields = {}
    duplicates = []
    for name, encoding in sections["encodings"].items():
        bits = number(text(encoding, "BitCount"), 4096)
        require(bits > 0, "zero encoding width")
        conditions[name] = {}
        for condition in one(encoding, "EncodingConditions"):
            require(condition.tag == "EncodingCondition", "condition node")
            label = text(condition, "ConditionName")
            conditions[name].setdefault(label, []).append(condition)
        for label, declarations in conditions[name].items():
            if len(declarations) > 1:
                observed = {"encoding": name, "condition": label, "count": len(declarations)}
                require(observed == manifest["reviewed_duplicate_condition"],
                        "unreviewed duplicate condition")
                require(all(ET.tostring(c).strip() == ET.tostring(declarations[0]).strip()
                            for c in declarations),
                        "duplicate condition differs")
                expression = one(declarations[0], "CondtionExpression")
                require(len(expression) == 1 and expression[0].tag == "lit"
                        and expression[0].attrib == {"val": "1"} and not len(expression[0]),
                        "reviewed duplicate condition expression changed")
                duplicates.append(observed)
        bitmap = one(one(encoding, "MicrocodeFormat"), "BitMap")
        fields[name] = unique(bitmap, "FieldName")
        for field in bitmap:
            layout = one(field, "BitLayout")
            require(number(layout.get("RangeCount", "")) == len(layout), "range count")
            orders = set()
            for part in layout:
                require(part.tag == "Range", "range node")
                order = number(part.get("Order", ""))
                require(order not in orders, "duplicate range order")
                orders.add(order)
                count = number(text(part, "BitCount"), 4096)
                offset = number(text(part, "BitOffset"), 4096)
                require(count > 0 and offset + count <= bits, "field outside encoding")
    require(duplicates == [manifest["reviewed_duplicate_condition"]],
            "reviewed duplicate-condition count changed")
    for operand_type in sections["operand_types"].values():
        for subtype in operand_type.findall("Subtypes/Subtype"):
            require((subtype.text or "").strip() in sections["operand_types"],
                    "unknown operand subtype")
    missing = []
    for name, instruction in canonical.items():
        flags = one(instruction, "InstructionFlags")
        require({n.tag for n in flags} == set(FLAGS) and len(flags) == 5, "flag fields")
        for flag in flags:
            boolean((flag.text or "").strip(), uppercase=True)
        group = one(instruction, "FunctionalGroup")
        require(text(group, "Name") in sections["functional_groups"], "unknown group")
        for subgroup in group.findall("FunctionalSubgroups/Subgroup"):
            require(subgroup.text in sections["functional_subgroups"]
                    or subgroup.text == "NOT_ASSIGNED", "unknown subgroup")
        seen = set()
        alternatives = one(instruction, "InstructionEncodings")
        require(0 < len(alternatives) <= 32, "encoding alternative bound")
        for alternative in alternatives:
            require(alternative.tag == "InstructionEncoding", "alternative node")
            enc = text(alternative, "EncodingName")
            condition = text(alternative, "EncodingCondition")
            require(enc in sections["encodings"], "unknown encoding")
            opcode_node = one(alternative, "Opcode")
            require(opcode_node.get("Radix") == "10", "unsupported opcode radix")
            opcode = number(opcode_node.text or "")
            key = (enc, condition, opcode)
            require(key not in seen, "duplicate instruction encoding")
            seen.add(key)
            if condition not in conditions[enc]:
                require([enc, condition] in manifest["reviewed_missing_conditions"]
                        and not conditions[enc], "unknown encoding condition")
                missing.append({"instruction": name, "encoding": enc, "condition": condition,
                                "condition_definition_available": False})
            orders = set()
            operands = one(alternative, "Operands")
            require(len(operands) <= 16, "operand bound")
            for operand in operands:
                require(operand.tag == "Operand", "operand node")
                require(set(operand.attrib) == {"Input", "Output", "IsImplicit",
                        "IsBinaryMicrocodeRequired", "Order"}, "operand attributes")
                order = number(operand.get("Order", ""), 255)
                require(order not in orders, "duplicate operand order")
                orders.add(order)
                for key in ("Input", "Output", "IsImplicit", "IsBinaryMicrocodeRequired"):
                    boolean(operand.get(key))
                require(text(operand, "OperandType") in sections["operand_types"],
                        "unknown operand type")
                require(text(operand, "DataFormatName") in sections["data_formats"],
                        "unknown data format")
                number(text(operand, "OperandSize"), 65535)
                field = operand.find("FieldName")
                if field is not None:
                    require(len(operand.findall("FieldName")) == 1
                            and field.text in fields[enc], "unknown operand field")
    require(len(missing) == pin["missing_default_condition_references"],
            "reviewed missing-condition count changed")
    return sections, aliases, conditions, missing


def compact_tree(root, dictionary):
    return [dictionary[root.tag],
            [dictionary[v] for pair in sorted(root.attrib.items()) for v in pair],
            dictionary[(root.text or "").strip()],
            [compact_tree(c, dictionary) for c in root if c.tag != "Description"]]


def collect_strings(node, result):
    if node.tag == "Description":
        return
    result.update([node.tag, (node.text or "").strip()])
    for key, value in node.attrib.items():
        result.update([key, value])
    for child in node:
        collect_strings(child, result)


def metadata(root, pin, manifest, sections, missing):
    values = set()
    collect_strings(root, values)
    strings = sorted(values)
    dictionary = {value: i for i, value in enumerate(strings)}
    chunks = []
    size = 0
    ranges = {}
    def add(raw):
        nonlocal size
        chunks.append(raw)
        size += len(raw)
    header = {"schema": SCHEMA, "architecture": pin["architecture"],
              "archive_sha256": manifest["archive"]["sha256"], "member": pin["member"],
              "member_sha256": pin["sha256"], "strings": strings,
              "node_format": "[tag_string_id,flat_attribute_string_ids,text_string_id,children]",
              "document": compact_tree(one(root, "Document"), dictionary),
              "architecture_metadata": compact_tree(one(one(root, "ISA"), "Architecture"), dictionary),
              "duplicate_condition_declarations": [manifest["reviewed_duplicate_condition"]],
              "unresolved_conditions": missing}
    add(json_bytes(header)[:-1] + b',"sections":{')
    for index, (key, values) in enumerate(sections.items()):
        if index:
            add(b",")
        add(json_bytes(key) + b":[")
        for position, (name, node) in enumerate(values.items()):
            if position:
                add(b",")
            start = size
            add(json_bytes(compact_tree(node, dictionary)))
            ranges[(key, name)] = (start, size)
        add(b"]")
    add(b"}}\n")
    require(size <= MAX_OUTPUT, "generated metadata size")
    return b"".join(chunks), strings, ranges


def rust_string(value):
    return json.dumps(value, ensure_ascii=False)


def rust_list(values):
    return "&[" + ",".join(values) + "]"


def rust_files(pin, manifest, sections, aliases, conditions, metadata_raw, strings, ranges):
    arch = pin["architecture"]
    prefix = "amd_isa_spec_catalog_v1_" + arch
    files = {}
    instructions = []
    for name, instruction in sorted(sections["instructions"].items()):
        alternatives = []
        for alt in instruction.findall("InstructionEncodings/InstructionEncoding"):
            encoding = text(alt, "EncodingName")
            condition = text(alt, "EncodingCondition")
            operands = []
            for operand in alt.findall("Operands/Operand"):
                flags = sum(boolean(operand.get(key)) << i for i, key in enumerate(
                    ("Input", "Output", "IsImplicit", "IsBinaryMicrocodeRequired")))
                field = operand.findtext("FieldName")
                operands.append("OperandData(%s,%s,%s,%s,%s,%s)" % (
                    operand.get("Order"), flags,
                    "None" if field is None else "Some(" + rust_string(field) + ")",
                    rust_string(text(operand, "OperandType")),
                    rust_string(text(operand, "DataFormatName")), text(operand, "OperandSize")))
            alternatives.append("EncodingData(%s,%s,%s,%s,%s)" % (
                rust_string(encoding), rust_string(condition), text(alt, "Opcode"),
                len(conditions[encoding].get(condition, [])), rust_list(operands)))
        flags = sum(boolean(text(one(instruction, "InstructionFlags"), key), True) << i
                    for i, key in enumerate(FLAGS))
        names = [rust_string(a) for a, canonical in sorted(aliases.items()) if canonical == name]
        start, end = ranges[("instructions", name)]
        instructions.append("InstructionData(%s,%s,%s,%s,(%s,%s))," % (
            rust_string(name), rust_list(names), flags, rust_list(alternatives), start, end))
    parts = []
    for index in range(0, len(instructions), 500):
        filename = prefix + "_instructions_" + str(index // 500) + "_generated.rs"
        files[RUST / filename] = ("// Generated; AMD MIT attribution: tools/amd-isa-catalog-v1/ATTRIBUTION.md\n"
            "&[\n" + "\n".join(instructions[index:index + 500]) + "\n]\n").encode()
        parts.append('include!(' + rust_string(filename) + ')')
    lines = ["// Generated; do not edit. See AMD MIT attribution in tools/amd-isa-catalog-v1.",
             "use super::*;", "#[rustfmt::skip]", "pub(super) static DATA: CatalogData = CatalogData {",
             "architecture: AmdIsaArchitectureV1::" + arch.capitalize() + ",",
             "member: " + rust_string(pin["member"]) + ",",
             "member_sha256: " + rust_string(pin["sha256"]) + ",",
             "metadata_sha256: " + rust_string(sha(metadata_raw)) + ",",
             'metadata: include_str!("../../../tools/amd-isa-catalog-v1/' + arch + '.json"),',
             "strings: " + rust_list(map(rust_string, strings)) + ",",
             "instructions: " + rust_list(parts) + ",",
             "aliases: " + rust_list("(" + rust_string(a) + "," + rust_string(c) + ")"
                                    for a, c in sorted(aliases.items())) + ","]
    for key in ("encodings", "operand_types", "data_formats"):
        lines.append(key + ": &[")
        for name, node in sorted(sections[key].items()):
            start, end = ranges[(key, name)]
            bits = node.findtext("BitCount")
            width = "None" if bits is None else "Some(" + str(number(bits, 65535)) + ")"
            lines.append("NamedData(%s,%s,(%s,%s))," % (rust_string(name), width, start, end))
        lines.append("],")
    lines.append("};")
    files[RUST / (prefix + "_generated.rs")] = ("\n".join(lines) + "\n").encode()
    return files


def generate(archive_path):
    manifest = read_json(DATA / "manifest.json")
    require(manifest["schema"] == "fe2o3-amd-isa-catalog-inputs-v1", "manifest schema")
    require([p["architecture"] for p in manifest["members"]] == ["cdna3", "cdna4"],
            "manifest architectures")
    require(manifest["metadata_profile_targets"] == {"gfx942": "cdna3", "gfx950": "cdna4"},
            "reviewed metadata target map")
    content = checked_archive(read_regular(archive_path, 3 * 1024 * 1024), manifest)
    files = {}
    for pin in manifest["members"]:
        root = parse_xml(content[pin["architecture"]])
        sections, aliases, conditions, missing = validate(root, pin, manifest)
        raw, strings, ranges = metadata(root, pin, manifest, sections, missing)
        files[DATA / (pin["architecture"] + ".json")] = raw
        files.update(rust_files(pin, manifest, sections, aliases, conditions, raw, strings, ranges))
    coverage_raw = read_regular(DATA / "reviewed-coverage.json", 65536)
    coverage = decode_json(coverage_raw)
    validate_coverage(coverage)
    names = coverage["instruction_names"]
    require(len(names) == 6 and sorted(set(names)) == names, "coverage names")
    cdna3 = parse_xml(content["cdna3"])
    canonical = {text(n, "InstructionName") for n in cdna3.findall("ISA/Instructions/Instruction")}
    require(set(names) <= canonical, "coverage references")
    files[RUST / "amd_isa_spec_catalog_v1_coverage_generated.rs"] = (
        "// Generated from separately reviewed coverage overlay; no executable semantics.\n"
        + "#[rustfmt::skip]\npub(super) const NAMES: &[&str] = "
        + rust_list(map(rust_string, names)) + ";\n"
        + 'pub(super) const SHA256: &str = "' + sha(coverage_raw) + '";\n').encode()
    index = {"schema": "fe2o3-amd-isa-catalog-generated-files-v1",
             "files": [{"path": str(path.relative_to(ROOT)), "bytes": len(raw), "sha256": sha(raw)}
                       for path, raw in sorted(files.items())]}
    files[DATA / "generated-index.json"] = json_bytes(index) + b"\n"
    return files


def publish(files, check):
    for path, raw in files.items():
        require(len(raw) <= MAX_OUTPUT, "output bound")
        if check:
            require(read_regular(path, MAX_OUTPUT) == raw,
                    "generated output mismatch: " + path.name)
        else:
            # Fixed repository-owned output paths only. No XML path is opened.
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o644)
            try:
                require(stat.S_ISREG(os.fstat(fd).st_mode), "output is not a regular file")
                os.ftruncate(fd, 0)
                view = memoryview(raw)
                while view:
                    count = os.write(fd, view)
                    require(count > 0, "short output write")
                    view = view[count:]
            finally:
                os.close(fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", required=True, type=Path, help="exact pinned local ZIP")
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--check", action="store_true", help="compare all generated bytes, write nothing")
    modes.add_argument("--write", action="store_true", help="regenerate only fixed metadata/index files")
    args = parser.parse_args()
    try:
        files = generate(args.archive)
        publish(files, args.check)
    except (ValueError, OSError, ET.ParseError, zipfile.BadZipFile, KeyError) as error:
        print("AMD ISA catalog rejected: " + str(error), file=sys.stderr)
        return 1
    print(json.dumps({"schema": SCHEMA, "mode": "check" if args.check else "write",
                      "files": len(files), "authority": False}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
