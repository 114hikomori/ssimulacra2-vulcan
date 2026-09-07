#!/bin/bash
# Run the oracle over the fixture corpus; print "pair score" lines.
# Usage: oracle/run_scores.sh [path-to-exe]
export PATH=/ucrt64/bin:$PATH
cd "$(dirname "$0")/.."
EXE="${1:-./build/ssimulacra2.exe}"
for p in photo grad noise step odd s8 s9 s12 s15 alpha gray big; do
  s="$("$EXE" "tests/fixtures/${p}_orig.png" "tests/fixtures/${p}_dist.png" 2>&1)"
  echo "$p $s"
done
echo "identical $("$EXE" tests/fixtures/photo_orig.png tests/fixtures/photo_orig.png 2>&1)"
echo "s7 $("$EXE" tests/fixtures/s7_orig.png tests/fixtures/s7_dist.png 2>&1)"
