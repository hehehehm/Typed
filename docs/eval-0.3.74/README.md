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

# Follow-up (same version): real words kept, new words kept, short tokens

Before = abe4975 (the 0.3.74 columns above), after = this commit; same eval code, both rebuilt
and re-run. Reports: `followup-*.md`, `spelling-followup-*.md`, `context-followup-*.md`.

## What changed

* **Real forms are not layout-converted into rare readings** (`engine.rs`). A token of 4+ letters
  that is a full-form word in its own layout (`шару`) converts only when the other reading is
  far more plausible: a ranked word under `FULL_FORM_YIELD_RANK` = 3000, or more than
  `READING_CORPUS_RATIO` = 10 × as frequent in the n-gram unigrams. A reading that turns typed
  letters into punctuation (`иную` → `bye.`, `болю` → `,jk.`) also needs a letter first, rank under
  `SIGNED_READING_MAX_RANK` = 1000 and the corpus ratio. `шару` (→ `ifhe`) stays.
* **Short tokens** (the one sanctioned more-aggressive rule). A 2–3 letter English token whose
  rank is over `SHORT_SWITCH_SRC_RANK` = 2000 (rare or junk; curated extras sit at 2000 and never
  qualify), whose Russian reading is a top-50 word (`SHORT_SWITCH_DST_RANK`), and not after an
  English word: converted. If the trailing keys are letters too (`lf.` = `даю`) the whole reading
  wins when it is a word under rank 3000. Kept by list: `vs` (`SHORT_SWITCH_KEEP`). Russian →
  English is not done (short Russian entries of that rank are slang people type: `ща`, `ша`, `рук`).
  Every 2–3 letter candidate with a reading under rank 300 was checked by eye
  (`examples/short_audit.rs`); newly converting: **`lf` → да, `ns` → ты, `ds` → вы, `bp` → из,
  `ye` → ну**. Untouched: ok, us, cd, ls, id, vs, el, pr, js, db, ui, dj, tt, cj, tot, ghb …
* **New words of regular formation are not typo-fixed into another lemma** (`morph::derivation`,
  `typo.rs`). An unknown Russian word analysed as base + productive suffix + ending (verbal
  `-лк-`, `-льщик-`, `-тель-` on an infinitive: `храни(ть)` + `лк` + `а`; nominal `-щик-`, `-ник-`,
  `-ств-`, `-изм-`, `-ист-` … on a nominative noun; `-ость`, `-еньк-` on an adjective; the stem 5+
  letters, shaped so case and conjugated forms are not bases), or a verb prefix of 3+ letters on
  a known infinitive / past form (`пере` + `залить`): a fix must keep the prefix, or the stem and
  the suffix (`хранилка` → `хранила` drops `лк`: refused; `пространнстве` → `пространстве` keeps
  them: allowed). `хранилка`, `донатщик`, `перезалить` stay.
* **Aggressive: no large shape changes** (`typo_wide.rs`). A candidate costing more than the
  Normal budget for the word's length must not change the length by more than one letter
  (`зранилка` → `заняла`, cost 2.9 against an Aggressive budget of 3.4: refused).

## Layout audit (`examples/layout_audit.rs`: every listed word typed correctly, Normal, typo on)

| real words layout-converted | abe4975 | after |
|---|---|---|
| RU ranked (40 245) | 0 | 0 |
| RU full forms (3 125 003) | 158 | **88** |
| EN ranked (40 200) | 0 | 5 (`lf ye bp ds ns`, the short-token rule) |
| EN full forms (773 396) | 42 | 41 |

No longer converted, e.g.: шару → ifhe, шарую, иную → bye., ерунку → theyre, вузе → dept, сфер,
турф, зире, бедру, берды, хищных, болю, херу, женю; vere → муку. Still converted: rare Russian
forms whose reading is a frequent English word (цуку → were, туче → next, туфе → neat, утешен →
entity, укусе → erect), 2–3 letter Russian forms read with punctuation (бол → ,jk, бук → ,er; as
before), and English tech terms (kbps → лизы, xfce → часу; as before).

## Unknown-word false positives on derived words

`examples/deriv_audit.rs`, every 20th Russian full form outside the ranked list (14 550 words),
the words `morph::derivation` analyses (184): changed by the typo fixer without the guard 39
(Normal) / 46 (Aggressive), with it **3 / 3** (e.g. `блузочки` → «булочки», `бронировала` →
«бронзировала» were made before). Over all 14 550: 3751 → 3731 (Normal).

## correction_eval (recall / wrong)

| | Normal before | Normal after | Aggressive before | Aggressive after |
|---|---|---|---|---|
| RU typo 1 | 79 / 2.1 | 79 / 2.1 | 80 / 2.5 | 80 / 2.5 |
| RU typo 2 | 63 / 7.1 | 63 / 7.1 | 69 / 9.3 | 69 / 9.0 |
| RU typo 3 | 44 / 5.8 | 44 / 5.8 | 66 / 7.8 | 65 / 7.7 |
| RU layout | 98 / 0.0 | **99** / 0.0 | 98 / 0.0 | **99** / 0.0 |
| RU layout+typo | 70 / 3.5 | 69.9 / 3.5 (one case) | 70 / 5.8 | 70 / 5.2 |
| EN typo 1 / 2 / 3 | 77/1.8, 63/7.3, 42/6.8 | same | 78/2.2, 71/9.6, 66/9.5 | 78/2.2, 71/9.3, 64/9.1 |
| EN layout+typo | 69 / 2.6 | 69 / 2.6 | 69 / 4.3 | 69 / 3.6 |
| FP unknown RU lower | 25.7% | 25.6% | 31.9% | 30.7% |
| FP unknown RU Capitalised | 4.9% | 4.8% | 5.0% | 4.9% |
| FP unknown EN lower | 17.3% | 17.3% | 30.8% | 27.7% |
| FP unknown EN Capitalised | 6.9% | 6.9% | 7.5% | 7.1% |
| curated set changed | 12/297 | 12/297 | 15/297 | 15/297 |

Per case (Normal, every sample compared): RU layout gains 8 (`lf.` → даю, `,ju` → бог, `ve;` → муж,
`le[` → дух, `,ele` → буду, `cg,` → спб, `ju'` → огэ, `nt,` → теб), losses 0; RU layout+typo loses one
(`Hfp;tdfkrt` = `разжевалке`, which reads as `разжева(ть)` + `лк`, is no longer made «раздевалке»).
Aggressive typo 3 loses 13 RU / 13 EN recall cases to the shape rule (`Пршстоош` → «Просто»,
`inevuble` → «inevitable»), and gains in wrong fixes and FP.

## spelling_eval (right / wrong)

Normal: identical to before in every row. Aggressive: RU frequent 1 80/3.4 → 80/3.3, RU frequent 2
72/10.0 → 72/9.8, RU rare 1 71/4.3 → 71/3.8, RU rare 2 28/12.3 → 28/11.3, EN frequent 2 87/4.9 →
87/4.4, EN rare 1 wrong 8.1 → 6.8, EN rare 2 wrong 18.6 → 14.0 (recall unchanged).

## context_eval (ctx on, right / wrong)

| | Normal before | Normal after | Aggressive before | Aggressive after |
|---|---|---|---|---|
| RU typo1 | 76.0 / 2.92 | 75.9 / 2.92 | 76.6 / 3.85 | 76.6 / 3.81 |
| RU typo2 | 45.5 / 9.56 | 45.5 / 9.55 | 56.7 / 11.88 | 56.6 / 11.54 |
| RU layout | 96.5 / 0.26 | **98.1** / 0.26 | 96.5 / 0.36 | **98.1** / 0.36 |
| RU layout+typo | 57.1 / 5.45 | 57.0 / 5.57 | 58.6 / 7.77 | 59.3 / 6.90 |
| RU rare words as unknown, changed (on / off) | 23.83 / 27.72 | 23.42 / 27.30 | 32.60 / 37.66 | 31.00 / 35.93 |
| RU clean sentences changed | 5 | 4 | 8 | 7 |
| EN typo1 / typo2 | 72.9/2.85, 35.5/14.24 | same | 73.2/3.11, 51.6/14.87 | 73.2/3.11, 51.4/14.78 |
| EN rare words changed (on / off) | 22.30 / 25.18 | same | 31.65 / 37.41 | 31.65 / 36.69 |
| EN clean sentences changed | 5 | 5 | 6 | 5 |

(The per-sentence samples shift by one or two cases where the engine's decisions differ, so
n moves by 1; the RU layout+typo Normal change is two cases of 1725.)

## Cases

| typed | abe4975 Normal / Aggressive | after |
|---|---|---|
| шару | ifhe / ifhe | unchanged |
| хранилка | хранила / хранила | unchanged |
| зранилка | — / заняла | unchanged (not «хранилка»: no general rule gives it) |
| lf, ns ghfd | lf, ns прав | да, ты прав |
| get ns | unchanged | unchanged |
| `есть зранилка типа nas, образ лежит на хосте -- зас сделают шару? я смогу напрямик скачать на нее? и надо почистить boot -- как лучше сделать?` | `ifhe&` on every level; Aggressive also `заняла` | unchanged on Cautious, Normal and Aggressive |
