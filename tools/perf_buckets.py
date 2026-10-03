#!/usr/bin/env python3
"""Where a solver's time goes, grouped: a `perf` recording split into the parts
of a Newton pass.

The flat profile names a hundred functions; this sums them into the categories
`docs/SOLVER_OPTIMIZATION.md` reasons about -- the compiled LU kernels, the
generic replay, the solve wrapper, the internal recovery, the Schur RHS, the
device stamps, libm, the Newton driver, the acoustics -- so two recordings can be
compared at a glance. libm is counted by shared object, because glibc's ifunc
variants of `exp` and `pow` have no names `perf` can show.

Release builds strip symbols. Build a copy that keeps them, with the same code:

    CARGO_TARGET_DIR=target/prof CARGO_PROFILE_RELEASE_STRIP=none \\
        CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \\
        cargo build --release --example rt_scenario
    perf record -F 4000 -o jazz.data \\
        target/prof/release/examples/rt_scenario --preset "Jazz Chorus" --seconds 19
    uv run tools/perf_buckets.py jazz.data [more.data ...]
"""

import re
import subprocess
import sys

CATEGORIES = [
    ("LU (compiled kernels)", r"partition::kernels::kernel_"),
    ("LU (generic replay)", r"solve_dense_planned|solve_masked|factor"),
    ("solve wrapper (copy, coupling, test)", r"solve_stamped_with_delta|finish_stamp|subtract"),
    ("internal recovery", r"recover_"),
    ("RHS (Schur responses, sources)", r"accumulate_rhs|prepare_rhs|rhs"),
    ("device stamps", r"::stamp|Stamper|linearis|Bipolar|Triode|Pentode|Diode|Jfet|Core|OpAmp|Transconductor|limit"),
    ("libm (exp/pow/log...)", r"^libm|\bexp\b|\bpow\b|\blog\b|atan|ln_1p"),
    ("Newton driver", r"iterate|Simulation>::process|settled|line_search|trial|converge|Simulation>::"),
    ("acoustics (cabinet, mics)", r"acoustics::"),
    ("oversampling, tone, rest of chain", r"oversample|voice::|Chain>|Tank|Tremolo|Brigade|Biquad|filters"),
]


def buckets(path):
    report = subprocess.run(
        ["perf", "report", "-i", path, "--no-children", "--sort", "dso,sym", "--stdio", "-g", "none"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    totals = {name: 0.0 for name, _ in CATEGORIES}
    other = []
    for line in report.splitlines():
        m = re.match(r"\s+([\d.]+)%\s+(\S+)\s+\[.\]\s+(.*)", line)
        if not m:
            continue
        pct, dso, sym = float(m.group(1)), m.group(2), m.group(3).strip()
        key = ("libm " if "libm" in dso else "") + sym
        for name, pattern in CATEGORIES:
            if re.search(pattern, key):
                totals[name] += pct
                break
        else:
            other.append((pct, sym))
    return totals, sorted(other, reverse=True)


def main():
    for path in sys.argv[1:]:
        totals, other = buckets(path)
        print(f"== {path}")
        for name, _ in CATEGORIES:
            print(f"  {totals[name]:5.1f}%  {name}")
        top = "; ".join(f"{sym[:40]} {pct:.1f}" for pct, sym in other[:4])
        print(f"  {sum(p for p, _ in other):5.1f}%  other  (top: {top})")


if __name__ == "__main__":
    main()
