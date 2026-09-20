#!/usr/bin/env python3
"""Repeated, alternating A/B runs of the release Twin mono solver fixture.

A enables the optimization; B sets its disable switch. Profiling is always off
for timing runs. Optional control runs are separate and excluded from medians.
The harness itself warms circuit state before measuring. No system settings are
changed. Use an otherwise idle machine; pinning does not isolate a CPU.
"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
from datetime import datetime, timezone


ROOT = Path(__file__).resolve().parents[1]
TEST = "twin_realtime_recording_solver_trace"
ROW = "American Twin,solver_trace_mono,"
CONTROL = "twin_solver_control_profile,"
OUTPUT_HASH = "twin_solver_output_hash,"
# print_realtime_pass has no header in the focused solver fixture.
COLUMNS = (
    "voice,mode,blocks,mean_us,p99_us,max_us,compute_misses,cpu_mean_us,"
    "cpu_p99_us,cpu_max_us,cpu_compute_misses,preempt_mean_us,preempt_p99_us,"
    "preempt_max_us,deadline_misses,late_starts,max_start_late_us,"
    "max_finish_late_us,max_meter_db,right_solves,first_compute_miss,"
    "first_cpu_compute_miss,first_deadline_miss,gain_passes_per_solve,"
    "gain_unsettled,gain_backtracks,power_passes_per_solve,power_unsettled,"
    "power_backtracks,iron_passes_per_solve,reverb_passes_per_solve,"
    "gain_predictor_suppressions,gain_continuation_attempts,"
    "gain_continuation_midpoint_successes,gain_continuation_successes,"
    "gain_continuation_actual_rescues,power_predictor_suppressions,"
    "power_continuation_attempts,power_continuation_midpoint_successes,"
    "power_continuation_successes,power_continuation_actual_rescues"
).split(",")
METRICS = ("cpu_mean_us", "cpu_p99_us", "cpu_max_us", "cpu_compute_misses")


def split_trace_row(row):
    # The harness mixes CSV with Rust Debug fields such as Some((12, 34)).
    # Only top-level commas are separators; csv.reader cannot preserve tuples.
    values = []
    depth = 0
    start = 0
    for index, char in enumerate(row):
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth < 0:
                raise ValueError("unbalanced debug field in timing row")
        elif char == "," and depth == 0:
            values.append(row[start:index])
            start = index + 1
    if depth:
        raise ValueError("unbalanced debug field in timing row")
    return values + [row[start:]]


def parse_log(log):
    """Fail closed on missing/duplicate rows or a changed CSV schema."""
    rows = [line[line.index(ROW):] for line in log.splitlines() if ROW in line]
    if len(rows) != 1:
        raise ValueError(f"expected one timing row, found {len(rows)}")
    values = split_trace_row(rows[0])
    if len(values) != len(COLUMNS):
        raise ValueError(f"CSV schema changed: expected {len(COLUMNS)} fields, got {len(values)}")
    result = dict(zip(COLUMNS, values))
    for name in COLUMNS[2:]:
        if name.startswith("first_"):
            continue
        value = float(result[name]) if any(char in result[name].lower() for char in ".e") else int(result[name])
        if not math.isfinite(value):
            raise ValueError(f"nonfinite {name}")
        result[name] = value
    control_rows = [line[line.index(CONTROL):] for line in log.splitlines() if CONTROL in line]
    if len(control_rows) > 1:
        raise ValueError("duplicate control summary")
    control = None
    if control_rows:
        control = {}
        for field in control_rows[0][len(CONTROL):].split(","):
            key, value = field.split("=", 1)
            if key in control:
                raise ValueError(f"duplicate control counter {key}")
            control[key] = int(value)
        if not {"solves", "newton_passes", "searched_passes", "unsettled"} <= control.keys():
            raise ValueError("incomplete control summary")
    return result, control


def differences(left, right):
    return {
        key: {"A": left.get(key), "B": right.get(key)}
        for key in sorted(left.keys() | right.keys())
        if left.get(key) != right.get(key)
    }


def parse_output_hash(log):
    rows = [line[line.index(OUTPUT_HASH):] for line in log.splitlines() if OUTPUT_HASH in line]
    if not rows:
        return None  # Older test executables do not print audio hashes.
    if len(rows) != 1:
        raise ValueError("duplicate output hash")
    match = re.fullmatch(r"twin_solver_output_hash,fnv1a64=([0-9a-fA-F]{16}),frames=([0-9]+)", rows[0])
    if match is None or int(match[2]) == 0:
        raise ValueError("malformed output hash")
    return {"fnv1a64": match[1].lower(), "frames": int(match[2])}


def capture(command):
    try:
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
        return result.stdout.strip() if result.returncode == 0 else result.stderr.strip()
    except OSError as error:
        return str(error)


def metadata(binary):
    with binary.open("rb") as executable:
        binary_sha256 = hashlib.file_digest(executable, "sha256").hexdigest()
    governors = {}
    for path in sorted(Path("/sys/devices/system/cpu/cpufreq").glob("policy*/scaling_governor")):
        try:
            governors[str(path)] = path.read_text().strip()
        except OSError:
            pass
    return {
        "utc": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(),
        "cpu": capture(["lscpu"]),
        "allowed_cpus": sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
        "governors": governors,
        "rustc": capture(["rustc", "-Vv"]),
        "git_head": capture(["git", "rev-parse", "HEAD"]),
        "git_status": capture(["git", "status", "--short"]),
        "binary": str(binary),
        "binary_sha256": binary_sha256,
        "binary_mtime": binary.stat().st_mtime,
    }


def build_binary(output):
    command = ["cargo", "test", "--release", "--lib", "--no-run", "--message-format=json"]
    print("Building release library test executable...", flush=True)
    with (output / "build.stdout.jsonl").open("x") as stdout, (output / "build.stderr.log").open("x") as stderr:
        subprocess.run(command, cwd=ROOT, stdout=stdout, stderr=stderr, check=True)
    binaries = set()
    for line in (output / "build.stdout.jsonl").read_text().splitlines():
        message = json.loads(line)
        if (message.get("reason") == "compiler-artifact"
                and message.get("target", {}).get("name") == "gainstagefx"
                and message.get("profile", {}).get("test")
                and message.get("executable")):
            binaries.add(message["executable"])
    if len(binaries) != 1:
        raise ValueError(f"expected one library test executable, found {sorted(binaries)}")
    return Path(binaries.pop())


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, help="existing release library test executable; skips Cargo")
    parser.add_argument("--output", type=Path, required=True, help="new directory for logs and summary (must not exist)")
    parser.add_argument("--preset", default="Ultra Lead")
    parser.add_argument("--repetitions", type=int, default=3, help="pairs of timing runs, minimum 3")
    parser.add_argument("--cpu", help="taskset CPU list, e.g. 2 or 2,3; inherited by test threads")
    parser.add_argument("--seconds", type=float, default=8.0, help="fixture duration; 0 uses entire recording")
    parser.add_argument("--disable-switch", default="GAINSTAGEFX_TEST_DISABLE_FIXED_13_STAMPED_MERIT")
    parser.add_argument("--control-profile", action="store_true", help="additional A/B pair with control profiling")
    parser.add_argument("--assert-control-equal", action="store_true", help="require exact control-counter equality and equal audio hashes when available; implies --control-profile")
    args = parser.parse_args()
    if args.repetitions < 3:
        parser.error("--repetitions must be at least 3")
    if not math.isfinite(args.seconds) or args.seconds < 0:
        parser.error("--seconds must be finite and nonnegative")
    if not re.fullmatch(r"GAINSTAGEFX_TEST_DISABLE_[A-Z0-9_]+", args.disable_switch):
        parser.error("--disable-switch must be a GAINSTAGEFX_TEST_DISABLE_* variable")
    return args


def main():
    args = arguments()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve() if args.binary else build_binary(output)
    env = os.environ.copy()
    removed = {}
    for key in list(env):
        if key.startswith(("GAINSTAGEFX_PROFILE_", "GAINSTAGEFX_TRACE_")) or key == args.disable_switch:
            removed[key] = env.pop(key)
    env["GAINSTAGEFX_ATTACK_PRESET"] = args.preset
    env["GAINSTAGEFX_REALTIME_SECONDS"] = str(args.seconds)
    command = [str(binary), TEST, "--ignored", "--nocapture", "--test-threads=1"]
    if args.cpu:
        command = ["taskset", "--cpu-list", args.cpu, *command]
    summary = {
        "metadata": metadata(binary),
        "configuration": {
            "preset": args.preset, "repetitions": args.repetitions,
            "seconds": args.seconds, "cpu": args.cpu, "sample_rate": 48000,
            "block_samples": 64, "callback_budget_us": 64 / 48000 * 1e6,
            "A": "enabled", "B": "disabled", "disable_switch": args.disable_switch,
            "command": command, "removed_environment": removed,
            "environment": {k: v for k, v in env.items() if k.startswith(("GAINSTAGEFX_", "RUSTFLAGS", "CARGO_"))},
            "note": "Unpaced thread CPU timings; cpu_compute_misses count budget overruns. Medians of maxima are not a hard realtime guarantee. Precompiled binary compiler provenance is caller supplied.",
        },
        "timing_runs": [], "control_runs": [],
    }

    def save():
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")

    def run(label, repetition, profiling=False):
        run_env = env.copy()
        if label == "B":
            run_env[args.disable_switch] = "1"
        if profiling:
            run_env["GAINSTAGEFX_PROFILE_SOLVER_CONTROL_TAIL"] = "1"
        name = f"{'control' if profiling else 'timing'}-{repetition:02}-{label}.log"
        print(f"Running {name}", flush=True)
        with (output / name).open("x") as log:
            result = subprocess.run(command, cwd=ROOT, env=run_env, stdout=log, stderr=subprocess.STDOUT, check=False)
        entry = {"label": label, "repetition": repetition, "log": name, "returncode": result.returncode}
        summary["control_runs" if profiling else "timing_runs"].append(entry)
        save()
        if result.returncode:
            raise RuntimeError(f"fixture failed: {name}")
        log = (output / name).read_text()
        if not re.search(r"test result: ok\. 1 passed; 0 failed;", log):
            raise ValueError(f"expected exactly one successful test in {name}")
        entry["metrics"], entry["control"] = parse_log(log)
        entry["output_hash"] = parse_output_hash(log)
        save()
        if profiling and entry["control"] is None:
            raise ValueError(f"missing control summary in {name}")
        if not profiling and entry["control"] is not None:
            raise ValueError(f"unexpected profiling in timing run {name}")
        if (entry["metrics"]["gain_unsettled"] or entry["metrics"]["power_unsettled"]
                or (entry["control"] is not None and entry["control"]["unsettled"])):
            raise ValueError(f"unsettled solves in {name}; benchmark health check failed")

    save()
    try:
        for repetition in range(1, args.repetitions + 1):
            for label in ("AB" if repetition % 2 else "BA"):
                run(label, repetition)
        summary["medians"] = {
            label: {key: statistics.median(entry["metrics"][key] for entry in summary["timing_runs"] if entry["label"] == label) for key in METRICS}
            for label in "AB"
        }
        budget = summary["configuration"]["callback_budget_us"]
        summary["cpu_budget_observations"] = {
            label: {
                "worst_cpu_max_us": max(entry["metrics"]["cpu_max_us"] for entry in summary["timing_runs"] if entry["label"] == label),
                "all_runs_within_budget": all(
                    entry["metrics"]["cpu_p99_us"] < budget
                    and entry["metrics"]["cpu_max_us"] < budget
                    and entry["metrics"]["cpu_compute_misses"] == 0
                    for entry in summary["timing_runs"] if entry["label"] == label
                ),
            }
            for label in "AB"
        }
        if args.control_profile or args.assert_control_equal:
            for label in "AB":
                run(label, 1, profiling=True)
            left, right = (entry["control"] for entry in summary["control_runs"])
            summary["control_differences"] = differences(left, right)
            if args.assert_control_equal and summary["control_differences"]:
                raise ValueError("A/B control counters differ; see summary.json")
        runs = summary["timing_runs"] + summary["control_runs"]
        available_hashes = [entry["output_hash"] for entry in runs if entry["output_hash"] is not None]
        if available_hashes:
            reference = available_hashes[0]
            summary["output_hash_differences"] = {
                entry["log"]: entry["output_hash"]
                for entry in runs if entry["output_hash"] != reference
            }
            summary["output_hash_reference"] = reference
            summary["output_hash_verification"] = "different_or_missing" if summary["output_hash_differences"] else "identical"
            if args.assert_control_equal and summary["output_hash_differences"]:
                raise ValueError("A/B audio hashes differ or are missing from some runs; see summary.json")
        else:
            summary["output_hash_verification"] = "unavailable_in_test_executable"
        summary["status"] = "complete"
    except (Exception, KeyboardInterrupt) as error:
        summary["status"] = "failed"
        summary["error"] = str(error) or type(error).__name__
        raise
    finally:
        save()
    print(json.dumps(summary["medians"], indent=2))
    print(json.dumps(summary["cpu_budget_observations"], indent=2))
    print(f"Summary: {output / 'summary.json'}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
