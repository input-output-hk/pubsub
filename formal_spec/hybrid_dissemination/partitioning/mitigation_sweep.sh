#!/bin/bash
# mitigation_sweep.sh
#
# For each adversarial budget k = 0 .. N-1, run PRISM to get the exact
# P(isolated) for the per-epoch delivery agent selection model, and
# compare against the analytical formula C(k,RF) / C(N-1,RF).
#
# Usage: bash mitigation_sweep.sh
# Requires: prism on PATH, mitigation_epoch.prism, mitigation_epoch.props

MODEL="mitigation_epoch.prism"
PROPS="mitigation_epoch.props"
DIR="$(cd "$(dirname "$0")" && pwd)"

# Parameters — must match the constants in mitigation_epoch.prism
N=10
RF=2

run_prism() {
    local k="$1"
    prism "$DIR/$MODEL" "$DIR/$PROPS" -const k="$k" 2>/dev/null \
        | grep "^Result" | head -1 | awk '{print $2}'
}

echo "========================================================"
echo "Mitigation sweep — per-epoch agent selection, N=$N, RF=$RF"
echo "P(isolated) = C(k,RF) / C(N-1,RF) = C(k,$RF) / C($((N-1)),$RF)"
echo "========================================================"
echo ""
printf "%-4s  %-10s  %-10s  %-6s\n" "k" "PRISM" "formula" "match"
printf "%-4s  %-10s  %-10s  %-6s\n" "---" "---" "---" "---"

for k in $(seq 0 $((N-1))); do
    p=$(run_prism "$k")
    if [ -z "$p" ]; then
        printf "%-4s  %-10s\n" "$k" "PRISM error"
        continue
    fi

    # Compute formula and compare at full precision via Python
    python3 -c "
from math import comb
k=$k; N=$N; RF=$RF
p_prism = $p
formula = comb(k, RF) / comb(N-1, RF) if k >= RF else 0.0
match = 'OK' if abs(p_prism - formula) < 1e-9 else 'FAIL'
print(f'{k:<4}  {p_prism:<10.6f}  {formula:<10.6f}  {match}')
"
done

echo ""
echo "Analytical bound (large N): adversary needs k ≈ sqrt(ε) * N nodes"
echo "to achieve isolation probability ε per epoch (RF=2)."
echo "Compare: original ring attack achieves ε = e^{-RF} ≈ 13.5% with k=2."
