#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$here/../.."
work="${1:-/tmp/typed-wordsets}"; mkdir -p "$work"; cd "$work"
python3 -m venv venv && ./venv/bin/pip -q install pymorphy3 pymorphy3-dicts-ru wordfreq
LO=https://raw.githubusercontent.com/LibreOffice/dictionaries/master
curl -sSfL -o ru_RU.dic $LO/ru_RU/ru_RU.dic; curl -sSfL -o ru_RU.aff $LO/ru_RU/ru_RU.aff
for v in en_US en_GB; do curl -sSfL -o $v.dic $LO/en/$v.dic; curl -sSfL -o $v.aff $LO/en/$v.aff; done
curl -sSfL -o scowl.tar.gz https://downloads.sourceforge.net/project/wordlist/SCOWL/2020.12.07/scowl-2020.12.07.tar.gz
curl -sSfL -o kaikki_ru.jsonl https://kaikki.org/dictionary/Russian/kaikki.org-dictionary-Russian.jsonl
curl -sSfL -o kaikki_en.jsonl https://kaikki.org/dictionary/English/kaikki.org-dictionary-English.jsonl
./venv/bin/python "$here/opencorpora.py"
./venv/bin/python "$here/unmunch.py" ru ru_RU.dic ru_RU.aff hs_ru.txt
./venv/bin/python "$here/unmunch.py" en en_US.dic en_US.aff hs_en_us.txt
./venv/bin/python "$here/unmunch.py" en en_GB.dic en_GB.aff hs_en_gb.txt
tar xzf scowl.tar.gz
( cd scowl-2020.12.07/final && cat $(ls | grep -E '^(english|american|british|canadian|australian|british_z|variant_[12])-(words|upper|abbreviations|contractions|proper-names)\.(10|20|35|40|50|55|60|70|80)$') ) \
  | iconv -f latin1 -t utf-8 | tr A-Z a-z | grep -E '^[a-z]+$' | sort -u > scowl80.txt
python3 "$here/kaikki.py" kaikki_ru.jsonl wikt_ru.txt ru
python3 "$here/kaikki.py" kaikki_en.jsonl wikt_en.txt en
./venv/bin/python "$here/wordfreq_dump.py"
python3 "$here/merge.py"
cd "$repo"
for l in ru en; do
  cargo run --release -q -p layout_keeper_core --example build_wordset -- "$work/${l}_all.txt" crates/core/data/${l}_known.fst
done
