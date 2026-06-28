#!/usr/bin/env bash
# main.sh — run the silent-eclipse experiments and print the results table.
#
# Self-contained: needs only python3 (standard library) plus the two files in
# this folder, securecyclon.py and reproduce_faithful.py. Takes a few minutes
# (15 runs). Override seeds with:  SEEDS=1,2,3,4,5 ./main.sh
set -euo pipefail
cd "$(dirname "$0")"

SEEDS="${SEEDS:-123,1234,4562}"
MUS="0.05 0.10 0.15 0.20 0.30"
STACKS=("concentrate,refuse" "concentrate,refuse,healer" "concentrate,refuse,token_dup")

# run(mu, attacks) -> "A_T_mean eclipse% m/(n-m) detections"
run() {
  python3 reproduce_faithful.py --mu "$1" --attacks "$2" --seeds "$SEEDS" 2>/dev/null | awk '
    /A_T_mean \(victim/ {at=$5; ceil=$10; sub(/\]/,"",ceil)}
    /eclipse%/          {ec=$6}
    /detections/        {det=$4}
    END {printf "%s %s %s %s", at, ec, ceil, det}'
}

echo "Silent targeted-eclipse sweep  (N=200, l=20, s=3, seeds=$SEEDS)"
echo "Cell = A_T_mean (eclipse%); all should be silent (det=0)."
echo

printf '| %-4s | %-7s | %-19s | %-12s | %-13s |\n' "mu" "m/(n-m)" "concentrate,refuse" "+healer" "+token_dup"
printf '|%s|%s|%s|%s|%s|\n' "------" "---------" "---------------------" "--------------" "---------------"

anydet=0
for mu in $MUS; do
  read -r a1 e1 ceil d1 <<<"$(run "$mu" "${STACKS[0]}")"
  read -r a2 e2 _    d2 <<<"$(run "$mu" "${STACKS[1]}")"
  read -r a3 e3 _    d3 <<<"$(run "$mu" "${STACKS[2]}")"
  anydet=$((anydet + d1 + d2 + d3))
  printf '| %-4s | %-7s | %-19s | %-12s | %-13s |\n' \
    "$mu" "$ceil" "$a1 (${e1}%)" "$a2 (${e2}%)" "$a3 (${e3}%)"
done

echo
if [ "$anydet" -eq 0 ]; then
  echo "All cells silent: D3+D4 detections = 0."
else
  echo "WARNING: total detections = $anydet — a cell is NOT silent."
fi
