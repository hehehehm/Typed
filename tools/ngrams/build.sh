#!/bin/sh
# Rebuilds crates/core/data/{ru,en}_ngrams.fst and the held-out samples of the evaluation.
# Usage: tools/ngrams/build.sh <dir with the files of fetch.sh>   (from the repository root)
set -e
src=$1
python3 tools/ngrams/count.py ru "$src/ru" "$src/rus_sentences.tsv.bz2" "$src/ruwiki-latest-pages-articles1.xml-p1p224167.bz2" 25000000
python3 tools/ngrams/count.py en "$src/en" "$src/eng_sentences.tsv.bz2" "$src/enwiki-latest-pages-articles1.xml-p1p41242.bz2" 25000000
for l in ru en; do
  cargo run --release -p layout_keeper_core --example build_ngrams -- "$src/$l/ngrams.tsv" crates/core/data/${l}_ngrams.fst 300000 120000
  awk 'NR%8==0' "$src/$l/heldout.txt" | head -3000 > crates/core/tests/data/heldout_$l.txt
done
