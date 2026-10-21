#!/usr/bin/env bash
# Runs the tests of every part*.egg with the reference interpreter.
#
#   ./test.sh                 all days
#   ./test.sh y2022           one year
#   ./test.sh y2022/day03     one day
#   ./test.sh -v y2022/day03  also show the passing tests
#
# Exits non-zero if any test fails.

set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(dirname "$here")"

verbose=0
if [[ "${1:-}" == "-v" ]]; then
  verbose=1
  shift
fi
filter="${1:-}"

cargo build --quiet --release --manifest-path "$root/reference/Cargo.toml" || exit 1
egg="$root/reference/target/release/eggnog-ref"

if [[ -t 1 ]]; then
  red=$'\e[31m' green=$'\e[32m' yellow=$'\e[33m' reset=$'\e[0m'
else
  red='' green='' yellow='' reset=''
fi

passed=0
failed=()
for file in $(cd "$here" && find . -name 'part*.egg' -path "*${filter}*" | sed 's|^\./||' | sort); do
  if [[ ! -f "$here/$(dirname "$file")/input.txt" ]]; then
    note=" ${yellow}(no input.txt)${reset}"
  else
    note=""
  fi
  if output="$("$egg" test "$here/$file" </dev/null 2>&1)"; then
    passed=$((passed + 1))
    echo "${green}ok${reset}    $file  ($(echo "$output" | tail -n 1))$note"
    if [[ $verbose == 1 ]]; then echo "$output" | sed '$d'; fi
  else
    failed+=("$file")
    echo "${red}FAIL${reset}  $file  ($(echo "$output" | tail -n 1))$note"
    echo "$output" | sed '$d'
  fi
done

total=$((passed + ${#failed[@]}))
if [[ $total == 0 ]]; then
  echo "no part*.egg files match '$filter'"
  exit 1
fi
echo
if [[ ${#failed[@]} == 0 ]]; then
  echo "${green}all $total files passed${reset}"
else
  echo "${red}${#failed[@]} of $total files failed:${reset} ${failed[*]}"
  exit 1
fi
