#!/bin/bash
# adversarial_sweep.sh
#
# For each adversary budget k = 0..4, enumerate all C(5,k) kill sets,
# run PRISM to get exact P(partition), and report the maximum (adversary's
# best strategy) and which set achieves it.
#
# Usage: bash adversarial_sweep.sh
# Requires: prism on PATH, ringcast_n6_adversarial_rf2.prism, adversarial.props

MODEL="ringcast_n6_adversarial_rf2.prism"
PROPS="adversarial.props"
DIR="$(cd "$(dirname "$0")" && pwd)"

run_prism() {
    local consts="$1"
    # Extract only the first P=? result (partition probability)
    prism "$DIR/$MODEL" "$DIR/$PROPS" -const "$consts" 2>/dev/null \
        | grep "^Result" | head -1 | awk '{print $2}'
}

nodes=(1 2 3 4 5)

echo "============================================================"
echo "Adversarial partitioning sweep — RingCast H(6,2), RF=2, N=6"
echo "Source = node 0 (always alive). Adversary kills k non-source nodes."
echo "P(partition) = P(some live node unreachable from source)."
echo "============================================================"

for k in 0 1 2 3 4; do
    echo ""
    echo "--- k=$k ---"
    best_p="0"
    best_set=""

    # Generate all C(5,k) subsets of {1,2,3,4,5}
    # Use recursive enumeration via nested loops (max k=4 so 4 levels suffice)
    case $k in
    0)
        subsets=("none")
        ;;
    1)
        subsets=()
        for a in 1 2 3 4 5; do
            subsets+=("$a")
        done
        ;;
    2)
        subsets=()
        for a in 1 2 3 4; do
            for b in $(seq $((a+1)) 5); do
                subsets+=("$a $b")
            done
        done
        ;;
    3)
        subsets=()
        for a in 1 2 3; do
            for b in $(seq $((a+1)) 4); do
                for c in $(seq $((b+1)) 5); do
                    subsets+=("$a $b $c")
                done
            done
        done
        ;;
    4)
        subsets=()
        for a in 1 2 3 4; do
            for b in $(seq $((a+1)) 5); do
                # All 4-subsets of {1..5}
                :
            done
        done
        for a in 1; do
          for b in 2; do
            for c in 3; do
              for d in 4; do subsets+=("$a $b $c $d"); done
              for d in 5; do subsets+=("$a $b $c $d"); done
            done
            for c in 4; do
              for d in 5; do subsets+=("$a $b $c $d"); done
            done
          done
          for b in 3; do
            for c in 4; do
              for d in 5; do subsets+=("$a $b $c $d"); done
            done
          done
        done
        for a in 2; do
          for b in 3; do
            for c in 4; do
              for d in 5; do subsets+=("$a $b $c $d"); done
            done
          done
        done
        ;;
    esac

    for subset in "${subsets[@]}"; do
        # Build the -const string
        consts=""
        for n in 1 2 3 4 5; do
            is_dead=false
            for s in $subset; do
                [ "$s" = "$n" ] && is_dead=true && break
            done
            consts+="dead${n}=${is_dead},"
        done
        consts="${consts%,}"  # strip trailing comma

        p=$(run_prism "$consts")
        if [ -z "$p" ]; then
            echo "  S={$subset}: PRISM error"
            continue
        fi

        # Format set label
        if [ "$subset" = "none" ]; then
            label="{}"
        else
            label="{$subset}"
        fi
        printf "  S=%-12s P(partition) = %s\n" "$label" "$p"

        # Track maximum (compare as floats via awk)
        if awk "BEGIN{exit !($p > $best_p)}"; then
            best_p="$p"
            best_set="$label"
        fi
    done

    echo "  >> Adversary's best: S=$best_set  P(partition) = $best_p"
done

echo ""
echo "Done."
