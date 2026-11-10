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
