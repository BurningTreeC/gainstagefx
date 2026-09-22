#!/usr/bin/env bash
# Run the Twin deterministic solver trace under the accepted solver
# configuration -- the regression oracle for every solver change.
#
#   tools/twin_trace.sh                     # accepted configuration
#   tools/twin_trace.sh --phases            # + per-phase TSC profiling
#   tools/twin_trace.sh --counters          # + the solver control counters
#   tools/twin_trace.sh --narrow            # scalar kernels, for a timing A/B
#   tools/twin_trace.sh VAR=VALUE ...       # add an experiment's switch
#
# The configuration lives here rather than in a doc because getting one
# variable wrong silently produces a different trajectory and a different hash.
# The test asserts the accepted hash itself; adding an experimental switch
# disarms that assertion and says so. See docs/SOLVER_EXPERIMENTS.md.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Rejected experiments. Inherited from the caller's shell they would quietly
# change the trajectory, so clear them rather than trust the environment.
cleared=(
    GAINSTAGEFX_ATTACK_PRESET
    GAINSTAGEFX_TEST_LINEAR_REFINE13
    GAINSTAGEFX_TEST_LINEAR_REFINE13_MAX_BACKWARD_ERROR
    GAINSTAGEFX_TEST_JACOBIAN_TANGENT13
    GAINSTAGEFX_TEST_JACOBIAN_TANGENT13_MAX_NORM
    GAINSTAGEFX_TEST_BOUNDARY_ONLY_RECOVERY13
    GAINSTAGEFX_TEST_CHORD13
    GAINSTAGEFX_TEST_CHORD13_MAX_MOVE_RATIO
    GAINSTAGEFX_TEST_CHORD13_MAX_MERIT_RATIO
    GAINSTAGEFX_TEST_JACOBIAN_INIT13_MAX_MERIT_RATIO
)

# The accepted configuration. The ratio of 3 is load bearing: a microscopic
# change below it moves the trajectory.
accepted=(
    GAINSTAGEFX_TEST_DISABLE_NLSOLVE_DOGLEG_TRUST_REGION=1
    GAINSTAGEFX_TEST_JACOBIAN_INIT13=1
    GAINSTAGEFX_TEST_JACOBIAN_INIT13_MIN_SOURCE_RATIO=3
    GAINSTAGEFX_TEST_JACOBIAN_INIT13_MIN_SOURCE_STEP=0
    GAINSTAGEFX_TEST_JACOBIAN_INIT13_FUSE_FIRST_NEWTON=1
)

extra=()
for argument in "$@"; do
    case "$argument" in
        --phases) extra+=(GAINSTAGEFX_PROFILE_TWIN_POWER_PHASES=1) ;;
        --counters) extra+=(GAINSTAGEFX_PROFILE_SOLVER_CONTROL_TAIL=1) ;;
        --narrow) extra+=(GAINSTAGEFX_TEST_DISABLE_WIDE_KERNELS=1) ;;
        *=*) extra+=("$argument") ;;
        *)
            echo "unknown argument: $argument" >&2
            exit 2
            ;;
    esac
done

unset_flags=()
for name in "${cleared[@]}"; do
    unset_flags+=(-u "$name")
done

exec env "${unset_flags[@]}" "${accepted[@]}" ${extra[@]+"${extra[@]}"} \
    cargo test --release --lib twin_realtime_recording_solver_trace \
    -- --ignored --nocapture --test-threads=1
