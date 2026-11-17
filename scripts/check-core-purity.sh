#!/usr/bin/env bash
# Fails when crates/core stops being pure.
#
# crates/core is the only crate whose tests can pin arithmetic without a real machine behind
# them, and that only holds while nothing in it reads a file, spawns a process, opens a socket
# or looks at a clock. A reader's output is whatever this laptop's sensors say that second, so
# code that mixes the two cannot be asserted on. This gate makes the rule enforced rather than
# documented.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="$root/crates/core/src"

denylist='std::fs|std::net|std::process|std::env|std::thread|std::os::|std::io::|SystemTime|Instant::now|unsafe '

if matches="$(grep -rnE "$denylist" "$src")"; then
    echo "crates/core must stay pure (no I/O, clocks, threads or unsafe). Offending lines:" >&2
    echo "$matches" >&2
    exit 1
fi

# A dependency could do I/O on the crate's behalf, so the manifest is part of the gate.
# serde is the single allowed entry: later plans derive it on the snapshot types. Every
# dependency table counts (dev-, build- and target-specific ones too), because each of them
# can bring code into the build. serde must be declared inline (`serde = { … }`); the table form
# `[dependencies.serde]` would read as a violation here.
manifest="$root/crates/core/Cargo.toml"
deps="$(awk '/^\[(.*\.)?(dev-|build-)?dependencies(\..*)?\]/{flag=1; next} /^\[/{flag=0}
    flag && NF && $0 !~ /^#/' "$manifest" | grep -vE '^serde( |=)' || true)"
if [[ -n "$deps" ]]; then
    echo "crates/core may depend on serde only. Found:" >&2
    echo "$deps" >&2
    exit 1
fi

# A build script runs arbitrary code at compile time.
if [[ -e "$root/crates/core/build.rs" ]] || grep -qE '^\s*build\s*=' "$manifest"; then
    echo "crates/core must not have a build script." >&2
    exit 1
fi

echo "core purity: ok"
