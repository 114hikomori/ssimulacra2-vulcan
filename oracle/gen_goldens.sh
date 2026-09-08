#!/bin/bash
# Generate per-stage oracle dumps for the fixture corpus (dump build required).
# Usage: oracle/gen_goldens.sh <run-tag>   (e.g. run1 / run2 for reproducibility check)
# 'big' is score-only (dump volume would be >1.5 GB); its golden is the CLI score.
export PATH=/ucrt64/bin:$PATH
cd "$(dirname "$0")/.."
RUN="${1:?usage: gen_goldens.sh <run-tag>}"
EXE=./build-dump/ssimulacra2
[ -x "$EXE.exe" ] && EXE="$EXE.exe"
for p in photo grad noise step odd s8 s9 s12 s15 alpha gray identical; do
  d="dumps/$p/$RUN"
  rm -rf "$d"
  mkdir -p "$d"
  case $p in
    identical) a=photo_orig; b=photo_orig ;;
    *) a=${p}_orig; b=${p}_dist ;;
  esac
  SSIMULACRA2_DUMP_DIR="$d" "$EXE" "tests/fixtures/$a.png" "tests/fixtures/$b.png" > "dumps/$p.$RUN.score" 2>&1
  echo "$p -> $(cat dumps/$p.$RUN.score)"
done
