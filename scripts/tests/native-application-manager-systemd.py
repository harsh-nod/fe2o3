#!/usr/bin/env python3
"""Closed native-parent unit contract; not a systemd or application execution test."""
import configparser
import pathlib


ROOT = pathlib.Path(__file__).resolve().parents[2]
UNIT = ROOT / "deployment/systemd/fe2o3-native-application-manager.service"
SERVICE = {
    "Type": "exec",
    "ExecStart": "/usr/libexec/fe2o3/fe2o3-native-application-manager",
    "User": "root",
    "Group": "root",
    "UMask": "0077",
    "LimitCORE": "0",
    "TasksMax": "256",
    "Restart": "no",
    "Slice": "system.slice",
    "Delegate": "",
    "KillMode": "control-group",
    "SendSIGKILL": "yes",
    "TimeoutStopSec": "30",
    "CapabilityBoundingSet": (
        "CAP_BPF CAP_CHOWN CAP_DAC_READ_SEARCH CAP_KILL CAP_NET_ADMIN CAP_SETFCAP "
        "CAP_SETGID CAP_SETPCAP CAP_SETUID CAP_SYS_PTRACE"
    ),
    "AmbientCapabilities": "",
    "NoNewPrivileges": "no",
}
RECORDS = {
    "/etc/fe2o3/proof-custodian/application-native-deployment-v1",
    "/etc/fe2o3/proof-custodian/native-manager-deployment-v1",
    "/etc/fe2o3/proof-custodian/native-conditional-root-policy-v1",
}


def validate(text):
    # systemd permits repeated ConditionPathExists, unlike ConfigParser. Keep
    # this one allowed repeated key explicit; all service duplicates refuse.
    records = []
    lines = []
    section = None
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            section = stripped
        if stripped.startswith("ConditionPathExists="):
            if section != "[Unit]":
                raise ValueError("condition outside Unit")
            records.append(stripped.partition("=")[2])
        else:
            lines.append(line)
    parser = configparser.ConfigParser(interpolation=None, strict=True)
    parser.optionxform = str
    parser.read_string("\n".join(lines))
    if len(records) != 3 or set(records) != RECORDS:
        raise ValueError("native record conditions differ")
    if parser.defaults() or set(parser.sections()) != {"Unit", "Service"}:
        raise ValueError("native manager must remain manual-only")
    if set(parser["Unit"]) != {"Description", "Documentation"}:
        raise ValueError("external lifetime coupling or unreviewed condition")
    if parser["Unit"]["Documentation"] != (
        "https://github.com/harsh-nod/fe2o3/blob/main/docs/native-application-manager-v3.md"
    ):
        raise ValueError("native joined startup documentation differs")
    if dict(parser["Service"]) != SERVICE:
        raise ValueError("native parent or external-custodian contract differs")


def main():
    text = UNIT.read_text(encoding="ascii")
    validate(text)
    hostile = [
        text.replace("KillMode=control-group", "KillMode=process"),
        text.replace("Restart=no", "Restart=always"),
        text.replace("NoNewPrivileges=no", "NoNewPrivileges=yes"),
        text.replace("SendSIGKILL=yes", "SendSIGKILL=no"),
        text.replace("[Service]", "[Service]\nSystemCallFilter=@system-service"),
        text.replace("[Service]", "[Service]\nPrivateUsers=yes"),
        text.replace("[Unit]", "[Unit]\nPartOf=fe2o3-compiler-execution.service"),
        text.replace("[Unit]", "[Unit]\nRequires=fe2o3-compiler-execution.service"),
        text.replace("[Unit]", "[Unit]\nAfter=fe2o3-compiler-execution.service"),
        text.replace("Restart=no", "Restart=no\nRestart=no"),
        text + "\n[Install]\nWantedBy=multi-user.target\n",
        text.replace("application-native-deployment-v1", "application-deployment-v1"),
        text.replace("/usr/libexec/fe2o3/fe2o3-native-application-manager", "/bin/true"),
    ]
    for candidate in hostile:
        try:
            validate(candidate)
        except (ValueError, configparser.Error):
            continue
        raise AssertionError("altered service contract was accepted")
    print(f"native manager unit contract: 1 positive, {len(hostile)} refusals")


if __name__ == "__main__":
    main()
