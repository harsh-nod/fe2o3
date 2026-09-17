function reject(reason) {
    print "invalid unit roster at line " NR ": " reason > "/dev/stderr"
    bad = 1
    exit 1
}
function complete(p) {
    return headers[p] == 1 && summaries[p] == 1 && running[p] == rows[p] &&
        passed[p] == expected_passed[p] && ignored[p] == expected_ignored[p]
}
function emit(line, fields, name) {
    if (!(package in allowed) || !(package in running) || summaries[package]) reject("row outside package")
    split(line, fields, " ")
    name = package SUBSEP fields[2]
    if (seen[name]++) reject("duplicate test")
    if (line ~ /\.\.\. ok$/) passed[package]++
    else if (line ~ /\.\.\. ignored(,.*)?$/) ignored[package]++
    else reject("unknown test outcome")
    rows[package]++
    print package, line
}
BEGIN {
    allowed["fe2o3_host"] = 1
    allowed["fe2o3_runtime"] = 1
    allowed["fe2o3_runtime_model"] = 1
    if (target == "gnu") prefix = "target/debug/deps/"
    else if (target == "musl") prefix = "target/x86_64-unknown-linux-musl/debug/deps/"
    else reject("target")
}
/Running unittests/ {
    if (pending != "" || (package != "" && !complete(package))) reject("unfinished package")
    path = $NF
    gsub(/[()]/, "", path)
    if (index(path, prefix) != 1 || executables[path]++) reject("executable path")
    package = substr(path, length(prefix) + 1)
    if (package !~ /-[0-9a-f]+$/) reject("executable hash")
    sub(/-[0-9a-f]+$/, "", package)
    if (!(package in allowed) || headers[package]++) reject("package roster")
    next
}
/^running [0-9]+ tests?$/ {
    if (!(package in allowed)) reject("running header outside package")
    if (package in running) {
        # Expected-abort subprocesses print their own header within a split
        # parent result, but never supply the parent package summary.
        if (pending == "" || $2 != 1 || summaries[package]) reject("unexpected nested run")
    } else running[package] = $2
    next
}
/^test [^ ]+ \.\.\. $/ {
    if (pending != "") reject("nested pending result")
    pending = $0
    next
}
/^ok$/ {
    if (pending == "") reject("unmatched split result")
    emit(pending "ok")
    pending = ""
    next
}
/^test [^ ]+ \.\.\./ {
    if (pending != "") reject("unfinished split result")
    emit($0)
    next
}
/^test result:/ {
    if (pending != "" || !(package in running) || summaries[package]++) reject("summary position")
    if ($3 != "ok." || $6 != 0 || $10 != 0 || $12 != 0) reject("nonzero incomplete outcomes")
    expected_passed[package] = $4
    expected_ignored[package] = $8
    if (!complete(package)) reject("summary count mismatch")
}
END {
    if (bad) exit 1
    if (pending != "") reject("truncated split result")
    for (p in allowed) if (!complete(p)) reject("missing or incomplete package")
}
