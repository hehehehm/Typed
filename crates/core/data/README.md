# Vendored word lists

`en.txt` and `ru.txt` contain the 40 000 most frequent words (one per line, most frequent first)
from [hermitdave/FrequencyWords](https://github.com/hermitdave/FrequencyWords) — `content/2018/{en,ru}/*_50k.txt`,
derived from OpenSubtitles 2018. Content license: **CC-BY-SA-4.0** (code of that project: MIT).
(Up to 0.3.2 the cutoff was 20 000; the first 20 000 lines are unchanged and are the only ones used
for letter-bigram statistics.)

Processing: lowercased, `ё` → `е`, only tokens made purely of `a-z` / `а-я` kept, single-letter tokens
kept only for real one-letter words (`a`, `i`; `я в к с о и у а`), duplicates removed.

`en_extra.txt` and `ru_extra.txt` are curated by LayoutKeeper (MIT): clean chat slang and abbreviations
(lol, brb, imho, имхо, кек, спс…), tech/file terms typed as words (exe, dll, api, json…) and some
rare/old words. `#` lines are comments. They are merged into the dictionary with rank
`min(list rank, 2000)` so short ones pass the engine's short-word check; they never override a known
word in the typed layout. No slurs or pornographic slang.

Urban Dictionary content is **not** included (copyright, terms of service, NSFW content).

## Full-form "known words" sets (since 0.3.64)

`ru_known.fst` (3 162 995 forms, 2.0 MB) and `en_known.fst` (810 138 forms, 3.4 MB) are
[fst](https://crates.io/crates/fst) sets of every word form we could collect: all cases, numbers,
genders, tenses, participles, comparatives, plurals… They are embedded with `include_bytes!` and
used straight from the binary (no unpacking, no start-up cost, no network). They answer only
"is this a real word?" — a word found only here is never changed by the typo fixer and is never
offered as a correction (fix targets still come from the ranked lists above). In the wrong-layout
engine a full-form hit keeps a 4+ letter word, unless its other-layout reading is a ranked word
(`еру` → `the`), and converts a 5+ letter word that is unknown in the typed layout.

Keys are one byte per letter: `a`–`z` as ASCII, `а`–`я` as 0xE0–0xFF (cp1251 positions), `ё` → `е`.
Letters-only forms of 2+ letters; hyphenated forms are dropped (the engine never sees them as one word).

| Source | License | Forms (after filtering) | New |
|---|---|---|---|
| **RU** [OpenCorpora](https://opencorpora.org) dictionary, via `pymorphy3-dicts-ru` 2.4.417150 (all paradigms enumerated) | CC-BY-SA 3.0 | 3 017 630 | 3 017 630 |
| **RU** Hunspell `ru_RU` from [LibreOffice dictionaries](https://github.com/LibreOffice/dictionaries) (A. Lebedev, affixes expanded by `tools/wordsets/unmunch.py`) | BSD-3-Clause-style, © 1997-2008 A. I. Lebedev; modified here (affixes expanded, lowercased, ё → е) | 1 366 402 | 98 773 |
| **RU** Wiktionary inflection tables, [kaikki.org](https://kaikki.org) Russian extract (wiktextract) | CC-BY-SA 4.0 / GFDL | 889 912 | 45 296 |
| **RU** [wordfreq](https://github.com/rspeer/wordfreq) 3.1 `large`, Zipf ≥ 2.5 | CC-BY-SA 4.0 (data) | 120 673 | 1 296 |
| **EN** [SCOWL](http://wordlist.aspell.net) 2020.12.07, sizes ≤ 80, all spelling variants, words + proper names + abbreviations | MIT-like SCOWL license (see `README` in SCOWL) | 285 574 | 285 574 |
| **EN** Hunspell `en_US` (LibreOffice) | SCOWL-derived, BSD/MIT-like | 87 950 | 366 |
| **EN** Hunspell `en_GB` (LibreOffice, M. Pinto) | LGPL (word list © Marco A.G. Pinto et al.); the only LGPL source, drop it from `merge.py` if that matters | 175 908 | 37 794 |
| **EN** Wiktionary, kaikki.org English extract | CC-BY-SA 4.0 / GFDL | 778 284 | 486 169 |
| **EN** wordfreq `large`, Zipf ≥ 2.5 | CC-BY-SA 4.0 | 47 600 | 235 |

Misspelling filter: Wiktionary and wordfreq also contain common misspellings (`teh`, `seperate`,
`becuase`, `recieve`), so a word found only in those two is dropped when it is one edit (letter
added, dropped, replaced, or neighbours swapped) away from a word of the curated dictionaries
(OpenCorpora + Hunspell for RU, SCOWL + Hunspell for EN): 47 229 + 4 881 RU and 227 571 + 3 451 EN
forms were dropped. Wiktionary entries whose senses are all tagged "misspelling" are skipped.

Not used: FrequencyWords `*_full.txt` (subtitle tokens: lots of typos — used only for the
before/after audit), SCOWL size 95 (contains `teh`, `adn`, `thier`), Urban Dictionary (never).
OpenCorpora's own export URL was down (404) on 2026-11-10, so the same dictionary was taken from
the pymorphy3 package.

Rebuild: `tools/wordsets/fetch.sh` (downloads ≈ 4.5 GB, then `examples/build_wordset.rs`).
Audit: `cargo run --release -p layout_keeper_core --example wordset_audit -- ru_full.txt en_full.txt`
and `--example holdout -- 20000 2000 full`.

## Brand / product names (since 0.3.65)

`en_brands.txt` (14 211 tokens) and `ru_brands.txt` (6 425 tokens) are known-only names: a word
found here is never changed by the typo fixer and is never offered as a correction; in the
wrong-layout engine it counts like a full-form word (kept when typed right, `куфдьу` → `realme`).
Only tokens that are not already in the lists / full-form sets are stored.

| Source | License | Tokens added |
|---|---|---|
| [Wikidata](https://www.wikidata.org) labels (en + ru) of items that are instances / subclasses of brand (Q431289), business (Q4830453), company (Q783794), enterprise (Q6881511), software (Q7397), mobile app (Q620615), video game (Q7889), website (Q35127), online service (Q19967801), smartphone model (Q19723451), automobile model (Q3231690), automaker (Q786820), cryptocurrency (Q13479982), product (Q2424752), operating system (Q9135), web browser (Q6368), programming language (Q9143), video game series (Q7058673), social network (Q3220391), restaurant chain (Q18534), retail chain (Q507619), airline (Q46970), with a per-class sitelinks threshold (2–8); fetched 2026-11-11 by `tools/brands/fetch_wikidata.sh` | **CC0** | EN 13 076, RU 1 317 |
| `tools/brands/curated_en.txt`, `curated_ru.txt`: popular brands (realme, oppo, poco, tecno, infinix…), Russian spellings with case forms generated by `build.py` (айфона, айфоне, айфоном…), and a few new words missing from the dictionaries (нейросетка, промпт, дипфейк…) | MIT (Typed) | EN 1 135, RU 5 108 |

Filters (`tools/brands/build.py`): letters-only tokens of 4–20 letters, lowercased, `ё` → `е`;
multi-word labels are split into words; a token whose reading in the other layout is a real word
there is dropped (`letv` = `дуем`), so a brand never blocks the conversion of a real word; a
Wikidata token one edit away from a frequent word (rank < 20 000) is dropped (it could be a typo
of that word) unless it is the whole label of a well-known item (25+ sitelinks).
Rebuild: `tools/brands/fetch_wikidata.sh` (in an empty dir), then
`python3 tools/brands/build.py <that dir> <dir with ru_all.txt/en_all.txt from merge.py> .`
Audit: `cargo run --release -p layout_keeper_core --example brands_audit`.

## Missing-space data (since 0.3.65)

- `ru_split_tags.txt`: words of `ru.txt` / `ru_extra.txt` tagged `n` (name / no dictionary
  parse), `g` (place) or `v` (verb form) from OpenCorpora tags via pymorphy3 (**CC-BY-SA 3.0**),
  made by `tools/brands/split_tags.py`. Names are never the second half of a split, verb forms
  only after «не».
- `ru_glued.txt`: curated (MIT) glued forms that the frequency lists carry as words although
  they are mistakes (`незнаю`, `вобщем`, `впорядке`); they are split anyway.
Audit: `cargo run --release -p layout_keeper_core --example split_audit -- ru_full.txt ru_all.txt`.

## Word n-gram context model (since 0.3.72)

`ru_ngrams.fst` (4.4 MB) and `en_ngrams.fst` (3.5 MB): an fst map of word unigrams, bigrams
and trigrams with quantised counts (`round(4·log2 count) + 1`, so ±9 %), read by
`src/ngram.rs`. The typo fixer uses it to choose between close candidates and to veto a fix
that does not fit after the previous one or two words (or when the typed word fits better).

| Source | License | Used |
|---|---|---|
| [Tatoeba](https://tatoeba.org) sentence exports `rus_sentences.tsv.bz2`, `eng_sentences.tsv.bz2` (downloads.tatoeba.org/exports/per_language, fetched 2026-11-19) | **CC BY 2.0 FR** (© Tatoeba contributors) | all sentences except those with id % 50 == 0 (held out) |
| Wikipedia dump parts `ruwiki-latest-pages-articles1.xml-p1p224167.bz2`, `enwiki-latest-pages-articles1.xml-p1p41242.bz2` (dumps.wikimedia.org, fetched 2026-11-19) | **CC BY-SA 4.0** (© Wikipedia contributors) | the first 25 M tokens of article text (markup stripped) |

Only counts derived from the texts are stored, no text. Tokens: lowercase letter runs of the
language (`ё` → `е`); vocabulary = `{lang}.txt` + `{lang}_extra.txt` + corpus words seen 30+
times; digits, other scripts, unknown words and sentence ends break the context (`<s>` marks a
sentence start).

| | tokens | unigrams | bigrams (kept / count ≥ 2) | trigrams (kept / count ≥ 3) | keys | size |
|---|---|---|---|---|---|---|
| ru | 26.95 M | 77 181 | 300 000 / 1 659 822 | 120 000 / 690 433 | 497 181 | 4.39 MB |
| en | 39.11 M | 48 683 | 300 000 / 1 634 612 | 120 000 / 1 360 335 | 468 683 | 3.45 MB |

Most frequent n-grams are kept (doubling the tables to 600 k / 300 k changed the
evaluation by ≤ 0.5 point). Rebuild: `tools/ngrams/fetch.sh` (in an empty dir), then
`tools/ngrams/build.sh <that dir>` from the repository root (count: `tools/ngrams/count.py`,
fst: `examples/build_ngrams.rs`). The same run writes `tests/data/heldout_{ru,en}.txt`:
3 000 held-out Tatoeba sentences per language (**CC BY 2.0 FR**) for `tests/context_eval.rs`.
Debug: `cargo run --release -p layout_keeper_core --example ctx_fit -- ru "я не" знаю знаб`.

## Proper-name forms (since 0.3.74)

`ru_names.fst` (408 142 forms, 0.3 MB): OpenCorpora word forms (via `pymorphy3-dicts-ru`,
**CC-BY-SA 3.0**) whose every parse is tagged `Name`, `Surn`, `Patr`, `Geox`, `Orgn` or
`Trad`, made by `tools/wordsets/oc_names.py` and `examples/build_wordset.rs`. Used only to
keep such forms out of spelling-error fixes into unranked forms (`typo_wide`).
