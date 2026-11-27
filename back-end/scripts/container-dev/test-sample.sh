#!/bin/sh
# Harmless test sample for the container provider. Submit it as a Linux task:
#
#   malbox task submit scripts/container-dev/test-sample.sh \
#     --platform linux --snapshot-id <active snapshot id> --timeout 30
#
# It records who ran it and where, then keeps running so guest plugins that
# inspect processes (guest-exec-checker) see it at the end of the analysis.
# POSIX sh only: Alpine has no bash.

out=./malbox-test-sample.out
{
    echo "malbox test sample"
    echo "user:    $(id -un) (uid $(id -u))"
    echo "cwd:     $(pwd)"
    echo "host:    $(hostname)"
    echo "kernel:  $(uname -sr)"
    echo "started: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} | tee "$out"

# Outlive the analysis timeout; the container is reverted afterwards.
sleep "${MALBOX_TEST_SLEEP:-600}"
