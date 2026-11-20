# 0.3.74: spelling errors (double letters, unstressed vowels)

Before = 0.3.73 (d65a8d6), after = 0.3.74, same eval code. Both evals were made deterministic
in 0.3.74 (correction_eval: frequent-word pools sorted by rank, then word — extras share one
rank and the hash-map order used to pick them; context_eval: per-sentence random streams, so a
sentence dropped by the engine does not shift every later sample). Numbers here therefore differ
slightly from `docs/eval-0.3.72/`; both columns below come from the new code.

## What changed (crates/core/src/typo_wide.rs)

| cost (tenths of a slip) | 0.3.73 | 0.3.74 |
|---|---|---|
| doubled letter typed once (`стелаж`) — `DOUBLE_MISSING` | 7 | **5** |
| letter typed twice (`адресс`) — `DOUBLE_EXTRA` | 6 | 6 (4–5 tried: more wrong fixes of keyboard slips) |
| RU unstressed vowel pair а/о е/и е/а е/я и/я а/я, 6+ letters, not the last letter — `VOWEL_COST` | 11 | **7** |
| other vowel pairs | 11 (15 up to 5 letters) | same |

* Spelling candidates: real forms of the full-form set (outside the 40 000-word list) reachable
  by ≤ 2 spelling operations (double letter added / removed, RU vowel pair), never at the first
  letter, each letter touched once, the doubled pair never the first two letters (`вв-` prefix).
  Score as rank 40 000 (`SPELL_RANK`). Not proper-name forms (`ru_names.fst`), not brands, not
  another ending of the same word (also reflexive: `бахнулись` / `бахнулась`); last two letters
  kept or only a vowel pair before the last letter (`сталлеж` → `стеллаж`). A single vowel
  change alone into such a form only on Aggressive. Russian only (English forms are mostly
  UK/US variants and names).
* `CLOSER_GAP` = 10: candidates one whole slip costlier than the best spelling candidate drop
  out (`сталлеж`: `стеллаж` at 14 vs `стала` at 24).

## correction_eval (keyboard slips; recall / wrong)

| | Normal 0.3.73 | Normal 0.3.74 | Aggressive 0.3.73 | Aggressive 0.3.74 |
|---|---|---|---|---|
| RU typo 1 | 79 / 2.1 | 79 / 2.1 | 80 / 2.5 | 80 / 2.5 |
| RU typo 2 | 63 / 7.0 | 63 / 7.1 | 69 / 8.6 | 69 / 9.3 |
| RU typo 3 | 44 / 5.2 | 44 / 5.8 | 67 / 7.2 | 66 / 7.8 |
| RU layout+typo | 70 / 3.5 | 70 / 3.5 | 70 / 5.8 | 70 / 5.8 |
| EN typo 1 | 77 / 1.8 | 77 / 1.8 | 78 / 2.2 | 78 / 2.2 |
| EN typo 2 | 63 / 7.3 | 63 / 7.3 | 70 / 9.8 | 71 / 9.6 |
| EN typo 3 | 42 / 6.8 | 42 / 6.8 | 66 / 9.6 | 66 / 9.5 |
| EN layout+typo | 69 / 2.6 | 69 / 2.6 | 69 / 4.4 | 69 / 4.3 |
| FP unknown RU lower | 25.3% | 25.7% | 31.1% | 31.9% |
| FP unknown RU Capitalised | 4.8% | 4.9% | 5.0% | 5.0% |
| FP unknown EN lower | 17.2% | 17.3% | 30.6% | 30.8% |
| FP unknown EN Capitalised | 6.9% | 6.9% | 7.4% | 7.5% |
| curated set changed | 12/297 | 12/297 | 15/297 | 15/297 |
| known frequent words changed | 0/1320 | 0/1320 | 0/1320 | 0/1320 |

## spelling_eval (new: one or two spelling errors; right / wrong)

| | Normal 0.3.73 | Normal 0.3.74 | Aggressive 0.3.73 | Aggressive 0.3.74 |
|---|---|---|---|---|
| RU frequent, 1 | 78 / 3.0 | 79 / 3.0 | 80 / 3.3 | 80 / 3.4 |
| RU frequent, 2 | 60 / 7.9 | 61 / 8.4 | 70 / 9.7 | 72 / 10.0 |
| RU rare form, 1 | 68 / 2.6 | 69 / 2.7 | 68 / 4.4 | 71 / 4.3 |
| RU rare form, 2 | 6 / 9.7 | **28** / 9.8 | 6 / 12.9 | **28** / 12.3 |
| EN frequent, 1 | 80 / 0.8 | 80 / 0.8 | 80 / 1.1 | 80 / 0.9 |
| EN frequent, 2 | 79 / 3.7 | **85** / 3.7 | 87 / 5.6 | 87 / 4.9 |
| EN rare form, 1 (right only; 2 errors: 1%) | 71 | 71 | 71 | 71 |

The rare-form samples come from the start of the alphabet of the full-form sets (every 97th
form), where name forms dominate; those are now refused on purpose.

## context_eval (3000 sentences per language; ctx on / off)

| | Normal 0.3.73 | Normal 0.3.74 | Aggressive 0.3.73 | Aggressive 0.3.74 |
|---|---|---|---|---|
| RU typo1 right/wrong (on) | 75.8 / 2.92 | 76.0 / 2.92 | 76.4 / 3.93 | 76.6 / 3.85 |
| RU typo2 right/wrong (on) | 45.6 / 9.29 | 45.5 / 9.56 | 57.0 / 11.47 | 56.7 / 11.88 |
| RU rare words treated as unknown, changed (on / off) | 23.70 / 27.59 | 23.83 / 27.72 | 32.21 / 36.49 | 32.60 / 37.66 |
| RU clean sentences changed (on) | 5 | 5 | 8 | 8 |
| EN typo1 right/wrong (on) | 72.9 / 2.85 | 72.9 / 2.85 | 73.2 / 3.11 | 73.2 / 3.11 |
| EN typo2 right/wrong (on) | 35.5 / 14.28 | 35.5 / 14.24 | 51.6 / 14.91 | 51.6 / 14.87 |
| EN rare words changed (on / off) | 22.30 / 25.18 | 22.30 / 25.18 | 30.94 / 36.69 | 31.65 / 37.41 |
| EN clean sentences changed (on) | 5 | 5 | 6 | 6 |

All rare held-out words (not the eval's half sample; `examples/ctx_unknown_dump.rs`), Normal,
context on: RU 361 → 366 of 1540 changed (+0.3 points), EN 55 → 56 of 282.

## Cases

| typed | 0.3.73 Normal / Aggressive | 0.3.74 Normal / Aggressive |
|---|---|---|
| стелаж | — / сделаю | стеллаж / стеллаж |
| стелажу | — / — | стеллажу / стеллажу |
| сталлежу | — / слежу | стеллажу / стеллажу |
| сталлеж | — / — | стеллаж / стеллаж |
| програма, калектив, адресс, adress | fixed | fixed |
| `Столько, по сталлежу, просят чек или лптенжноу поручение.` | unchanged / `по слежу` | `по стеллажу` (both levels); «Столько» (a real word at the sentence start) and «лптенжноу» (four errors) stay |

Still not handled: `occured` (it is in the frequency list itself, a common misspelling in
subtitles), `колонада` → «колорадо» (the strict rules fix it first, into a ranked word),
`сумашедший` (a missing letter that is not a double), `коллектов` (о/и is not an unstressed
pair), `стилаж` (a single vowel error into an unranked form: not on Normal by design, and ambiguous on Aggressive).
