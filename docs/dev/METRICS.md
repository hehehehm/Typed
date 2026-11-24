# Метрики

Три версии на одних и тех же наборах, собраны из релизных коммитов. Движок 0.3.85 зафиксирован в `20430b4`. Харнесс `typist_sweep` один и тот же для всех трёх версий.

Как читать:

- `right` и `recall`: доля исправленных правильно, больше лучше.
- `wrong`: доля неверных исправлений, меньше лучше.
- `fp`: число ложных срабатываний на чистом тексте, меньше лучше.
- `fail` в разделах `fp_*`: ложные срабатывания на миллион проверок, меньше лучше.
- `missed_pct`: доля ошибок без исправления, меньше лучше.

## Итог гейта

Гейт `scripts/release_gate.py` без допусков: любая метрика хуже базовой версии валит релиз.

- 0.3.85 против 0.3.83: **не пройден**, хуже 27 метрик. Все они в доле неверных исправлений и у тестеров. Все разделы `typist_sweep` не хуже 0.3.83. 22.11.2026 пользователь принял этот рост ради прироста исправлений и разрешил выпуск.
- 0.3.84 против 0.3.83: тоже не пройден, хуже 36 метрик.
- `typist_real_failures`: 49 из 49. Curated FP: 6 из 315. Частые слова: 0 из 1320. `typo_audit`: 0 ложных, 0 пропусков из 40.

Метрики хуже 0.3.83: `context/en/layout+typo/wrong`, `context/en/typo1/wrong`, `context/en/typo2/wrong`, `context/ru/layout+typo/wrong`, `context/ru/typo1/wrong`, `context/ru/typo2/wrong`, `correction/En frequent, 2 error(s)/wrong`, `correction/En rare form, 1 error(s)/wrong`, `correction/En typo 1/all/wrong`, `correction/En typo 2/all/wrong`, `correction/En typo 3/all/wrong`, `correction/Ru typo 2/all/wrong`, `correction/Ru typo 3/all/wrong`, `correction/unknown-fp/En lower`, `correction/unknown-fp/Ru Capitalised`, `correction/unknown-fp/Ru lower`, `typist/gramotey/aggressive/missed_pct`, `typist/gramotey/aggressive/wrong_pct`, `typist/gramotey/cautious/missed_pct`, `typist/gramotey/normal/fp`, `typist/gramotey/normal/wrong_pct`, `typist/torpyga/cautious/wrong_pct`, `typist/torpyga/normal/wrong_pct`, `typist/zlodey/aggressive/fixed_pct`, `typist/zlodey/aggressive/fp`, `typist/zlodey/aggressive/wrong_pct`, `typist/zlodey/normal/fp`.

## 0.3.87 против 0.3.86, прогон 24.11.2026 (`iter087j`, коммит `f1b3adb`)

Гейт не пройден: 47 метрик. `typist_real_failures` 117 из 117, curated FP 6 из 315, частые слова 0 из 1320, `typo_audit` без ошибок.

| набор | 0.3.86 верно / неверно | f1b3adb верно / неверно |
|---|---|---|
| Ru typo 1 | 85 / 1.9 | 86 / 2.4 |
| En typo 1 | 84 / 2.5 | 84 / 3.3 |
| Ru typo 2 | 63 / 7.7 | 63 / 7.8 |
| En typo 2 | 63 / 8.3 | 63 / 8.8 |
| Ru layout+typo | 74 / 3.8 | 78 / 4.2 |
| En layout+typo | 75 / 2.7 | 79 / 2.8 |
| Ru частые, 1 ошибка | 81 / 3.7 | 82 / 4.0 |
| En частые, 1 ошибка | 98 / 0.9 | 97 / 1.3 |
| контекст ru layout+typo | 59.7 / 5.86 | 63.3 / 5.74 |
| контекст en layout+typo | 53.1 / 7.73 | 58.3 / 7.43 |
| контекст ru typo1 | 79.9 / 3.89 | 80.9 / 3.92 |
| контекст en typo2 | 34.4 / 16.49 | 34.7 / 16.74 |
| fp_ngram_phrase (ошибок) | 632 | 690 |
| fp_full_forms (ошибок) | 26.9 | 43.1 |
| space_glued_prep (полнота) | 98.70 | 90.38 |

## 0.3.87 против 0.3.86

Прогон 23.11.2026, ветка `wip-0.3.87`, релиза нет. База: полный прогон 0.3.86 (`iter086`, коммит `d7ef7f8`). Полный набор после правила «всегда выбирать кандидата», самой ранней соседней перестановки, перевода раскладки, который уступает перестановке в своей раскладке, отмены с перебором вариантов, layout+typo соседней клавишей, сочетаний клавиш и лишней буквы в частом слове (`iter087g`). `typist_real_failures`: 103 из 103. Curated FP: 6 из 315. Частые слова: 0 из 1320. `typo_audit`: 0 ложных, 0 пропусков из 40.

Дополнительно: `ult` → «где». Одиночно и после русского уже переводилось и в 0.3.86, и на ветке (`ult` в `en_known.fst`, не в частотном словаре, ngram 0). После английского слова не переводилось из-за защиты коротких full-forms в английском контексте. После правки (незасвидетельствованные короткие full-forms больше не блокируются) изолированный прогон RU rank<20000 длины 2–4 в английской раскладке: miss 434 из 1821, как до правки; после «это » тоже 434; после «the » 465→454. Курируемые 6/315, частые 0/1320.

Соседние перестановки частых слов длиной 4–6 букв (все позиции, включая первую; столкновение — набор сам является словом или известной формой):

| набор | 0.3.86 hit% без столкновений | miss | wrong | после (vs исходное) | miss | wrong |
|---|---|---|---|---|---|---|
| все ranked | 78,59% | 15413 | 10446 | 99,36% | 5 | 765 |
| rank < 1000 | 90,40% | 182 | 247 | 98,94% | 0 | 47 |
| rank < 5000 | 88,47% | 1074 | 1328 | 99,21% | 0 | 164 |

Относительно правила «самая ранняя перестановка» (уникальный набор, без столкновений): 119617 совпадений из 119677, 55 несовпадений, 5 пропусков по правилу, 0 случаев «правило не дало кандидата, набор оставлен». Все 47 неверных относительно исходного слова при rank < 1000 совпадают с правилом.

В штатном `typist_sweep` выборка `typo1_transpose (25%×3)`: recall 89,71% → 98,19%, wrong 4,63% → 0,53%.

Layout+typo: частые русские слова rank < 5000, набранные в английской раскладке, с одной соседней заменой или соседней перестановкой (физическая QWERTY, включая знаки). `meltn` в 0.3.86 и на 27103f9: пропуск `Keep("not confident")`; после правила — «будет».

| набор | 0.3.86 hit / miss / wrong | после |
|---|---|---|
| subst n=224790 | 126705 / 59010 / 39075 | 137778 / 47580 / 39432 |
| transp n=27246 | 23588 / 2659 / 999 | 26087 / 155 / 1004 |

Лишняя буква в частых словах rank < 5000 длины 4–8 (соседняя клавиша или буква из самого слова; набор, который сам уже слово или полная форма, пропущен):

| набор | 0.3.86 unique hit / miss / wrong | после |
|---|---|---|
| RU n=290398 | 85,1% / 7,9% (22946) / 7,0% (20272) | 93,7% / 0,9% (2648) / 5,4% (15681) |
| EN n=275322 | 79,9% / 15,0% (41163) / 5,2% (14294) | 96,7% / 1,3% (3524) / 2,1% (5667) |
| RU interior n=244844 | 96,4% / 2,0% / 1,6% | 97,7% / 0,5% / 1,8% |
| EN interior n=229329 | 97,5% / 1,1% / 1,4% | 97,7% / 0,4% / 1,9% |

Гейт «не хуже 0.3.86» скриптом не пройден: 58 метрик. Главное, что хуже по качеству, а не из-за несравнимости тестера:

- `correction_eval`: доля неверных typo 1 RU 1,9→2,5, EN 2,5→3,5; typo 2 RU 7,7→7,9, EN 8,3→8,8; typo 3 EN 7,3→7,4; Ru layout+typo 74% / 3,8% → 78% / 4,5%; En layout+typo 75% / 2,7% → 79% / 3,1%. Spelling En frequent 1 right 98→97, wrong 0,9→1,8; Ru frequent 1 81/3,7 → 82/4,6; frequent 2 RU wrong 8,9→9,4. Curated FP 6/315, как в 0.3.86. Частые слова 0/1320. Recall typo 1 RU 85→86.
- `context_eval` (ctx on, all): typo1 wrong RU 3,89→4,29, EN 3,58→3,53 (чуть лучше); typo2 RU 10,60→10,79, EN 16,49→17,16; layout+typo recall выше, wrong тоже (RU 5,86→6,32, EN 7,73→8,27).
- Склейки с предлогом в `typist_sweep` ниже, потому что перестановка теперь важнее («кгода» → «когда», а не «к года»): `space_glued_prep` 98,70→98,21, CAPS 98,70→98,32, corpus_bi 96,93→96,60.
- Выборочные typo1: `insert_nb` 97,49→97,62 (wrong 1,06→1,19), `double` 98,32→98,16, `omit` 80,46→80,38 (wrong 6,93→8,94), `subst_nb` 89,27→89,26 (wrong 3,83→4,50). Полный строчный сдвиг рук 88,49→88,39. `typo_hyphen` 97,17→96,60. Перевод раскладки чуть ниже (на 0,04 п. п.), потому что перестановка частого слова в своей раскладке важнее более редкого перевода. `fp_full_forms` 321→323, `fp_ngram_phrase` 495→553.
- `typist_eval`: у gramotey Normal fp 18→25, wrong 0,16→0,19; Aggressive числа слов близки, fp 8=8, wrong 0,24→0,26, missed чуть выше. У torpyga и zlodey число слов разошлось из-за других замен тестера, доли с 0.3.86 напрямую не сравнимы.

Дальнейший цикл по доле неверных typo 1/2 остановлен: всегда выбирать перестановку частого слова неизбежно поднимает wrong там, где один набор — перестановка двух частых слов. Правило ранней перестановки ещё увеличивает число неверных относительно исходного слова (rank < 1000: 5→47), потому что часто выбирает не самое частое из двух слов. Правило лишней буквы добавляет ещё немного неверных (Ru frequent 1 wrong 4,1→4,6 относительно прогона без этого правила).

Релиза нет.

## 0.3.86 против 0.3.85

Прогон 23.11.2026, релиз 0.3.86, коммит кода `da525b2`. База: полный прогон 0.3.85 (`iter/ev6`, движок `20430b4`). `typist_real_failures`: 55 из 55. Curated FP: 6 из 315. Частые слова: 0 из 1320. `typo_audit`: 0 ложных, 0 пропусков из 40.

Гейт «не хуже 0.3.85» скриптом не пройден: 29 метрик. Из них стабильные оценки не хуже:

- Все таблицы `correction_eval` и `context_eval` совпали с 0.3.85, включая долю неверных.
- Ложные `typist_sweep` (`fp_full_forms`, `fp_mat`, `fp_hyphen`, `fp_ngram_phrase`, `fp_ranked`) совпали. Все разделы перевода раскладки совпали. Сдвиг рук на 100% строчных совпал: 88,49%. Лишний пробел внутри слова чуть выше: 82,64 против 82,41.

Хуже по скрипту, но это не падение качества движка:

- `typist_eval` (fixed/missed у всех трёх персонажей, несколько fp). Знак препинания чаще даёт замену сразу, тестер из-за этого иначе расходует случайные числа (`undo_retype`), число слов расходится (например torpyga/normal 30096 против 29893). Доли с 0.3.85 напрямую не сравнимы.
- Выборочные разделы `typist_sweep` на 25% слов: `typo1_all5_*` (92,78→92,73 и рядом), `typo1_transpose` (90,04→89,71), сдвиг рук Title/CAPS (88,75→88,46). Полный строчный сдвиг рук не изменился. Такие выборки и раньше плавали примерно на 0,3 п. п.

Гейт скриптом не пройден. 23.11.2026 пользователь принял этот прогон и разрешил выпуск 0.3.86 как есть.

## Слова 2–3 буквы в чужой раскладке с заглавной и капсом

В отчёте 0.3.84 для 0.3.83 стояли 73,3 и 78,0% (RU→EN) и 93,4% (EN→RU). Эти числа сняты с промежуточной сборки во время работы над 0.3.84, а не с релиза 0.3.83. Полный прогон того же харнесса на релизе 0.3.83 даёт:

| Направление | 0.3.83 | 0.3.84 | 0.3.85 |
|---|---|---|---|
| RU→EN Title | 21,48 | 49,30 | 66,72 |
| RU→EN CAPS | 21,94 | 52,31 | 69,63 |
| EN→RU Title | 18,99 | 48,90 | 85,00 |
| EN→RU CAPS | 19,06 | 48,95 | 85,01 |

## typist_sweep

`fp_mat` для 0.3.83 не измерен: раздел на 0.3.83 идёт часами. Числа с выборкой 25% плавают между прогонами на ±0,3 п. п.

| Метрика | 0.3.83 | 0.3.84 | 0.3.85 | 0.3.85 против 0.3.83 |
|---|---|---|---|---|
| fp_full_forms/fail | 40.858 | 34.398 | 26.931 | не хуже |
| fp_hyphen/fail | 0.000 | 0.000 | 0.000 | не хуже |
| fp_mat/fail | — | 0.000 | 0.000 | нет данных 0.3.83 |
| fp_ngram_phrase/fail | 3460.676 | 2365.008 | 632.116 | не хуже |
| fp_ranked/fail | 0.000 | 0.000 | 0.000 | не хуже |
| layout EN→RU 2–3 CAPS/recall | 19.060 | 48.950 | 85.010 | не хуже |
| layout EN→RU 2–3 Title/recall | 18.990 | 48.900 | 85.000 | не хуже |
| layout EN→RU 2–3 lower/recall | 19.060 | 19.060 | 19.060 | не хуже |
| layout EN→RU CAPS/recall | 95.660 | 97.250 | 99.120 | не хуже |
| layout EN→RU Title/recall | 95.660 | 97.250 | 99.120 | не хуже |
| layout EN→RU lower/recall | 95.660 | 95.660 | 95.660 | не хуже |
| layout RU→EN 2–3 CAPS/recall | 21.940 | 52.310 | 69.630 | не хуже |
| layout RU→EN 2–3 Title/recall | 21.480 | 49.300 | 66.720 | не хуже |
| layout RU→EN 2–3 lower/recall | 21.940 | 21.940 | 21.940 | не хуже |
| layout RU→EN CAPS/recall | 97.230 | 98.190 | 98.840 | не хуже |
| layout RU→EN Title/recall | 97.220 | 98.100 | 98.750 | не хуже |
| layout RU→EN lower/recall | 97.220 | 97.150 | 97.220 | не хуже |
| space_extra_inside (5% ranked)/recall | 0.000 | 0.000 | 82.410 | не хуже |
| space_glued_bigram (10%)/recall | 22.220 | 25.810 | 79.150 | не хуже |
| space_glued_prep/recall | 30.140 | 99.830 | 98.700 | не хуже |
| space_glued_prep_CAPS/recall | 30.730 | 99.820 | 98.700 | не хуже |
| space_glued_prep_corpus_bi/recall | 28.290 | 97.490 | 96.930 | не хуже |
| space_glued_trigram (10%)/recall | 17.420 | 21.840 | 87.420 | не хуже |
| space_nb_bigram (10%)/recall | 0.000 | 0.000 | 54.370 | не хуже |
| space_prep_valid/fail | 0.000 | 0.000 | 0.000 | не хуже |
| space_prep_valid_CAPS/fail | 0.000 | 0.000 | 0.000 | не хуже |
| typo1_all5_CAPS/recall | 88.290 | 88.050 | 92.650 | не хуже |
| typo1_all5_Title/recall | 88.290 | 88.050 | 89.050 | не хуже |
| typo1_all5_lower/recall | 89.230 | 88.920 | 92.780 | не хуже |
| typo1_double (25%x3)/recall | 93.220 | 93.320 | 98.390 | не хуже |
| typo1_insert_nb (25%x3)/recall | 96.830 | 96.640 | 97.480 | не хуже |
| typo1_omit (25%x3)/recall | 75.610 | 74.800 | 80.210 | не хуже |
| typo1_subst_nb (25%x3)/recall | 86.460 | 86.550 | 89.320 | не хуже |
| typo1_transpose (25%x3)/recall | 88.640 | 87.990 | 90.040 | не хуже |
| typo_hand_shift_mat (5%)/recall | 0.160 | 0.160 | 99.970 | не хуже |
| typo_hand_shift_ranked (100% lower)/recall | 41.630 | 41.630 | 88.490 | не хуже |
| typo_hand_shift_ranked (25% CAPS)/recall | 41.290 | 41.900 | 88.750 | не хуже |
| typo_hand_shift_ranked (25% Title)/recall | 41.290 | 41.900 | 88.750 | не хуже |
| typo_hyphen (3 cases)/recall | 0.000 | 96.880 | 97.170 | не хуже |

## correction_eval

| Метрика | 0.3.83 | 0.3.84 | 0.3.85 | 0.3.85 против 0.3.83 |
|---|---|---|---|---|
| En frequent, 1 error(s)/right | 80.000 | 80.000 | 98.000 | не хуже |
| En frequent, 1 error(s)/wrong | 0.900 | 0.900 | 0.900 | не хуже |
| En frequent, 2 error(s)/right | 85.000 | 85.000 | 86.000 | не хуже |
| En frequent, 2 error(s)/wrong | 3.700 | 3.700 | 3.900 | хуже |
| En layout+typo/all/right | 69.000 | 69.000 | 75.000 | не хуже |
| En layout+typo/all/wrong | 2.700 | 2.800 | 2.700 | не хуже |
| En layout/all/right | 100.000 | 100.000 | 100.000 | не хуже |
| En layout/all/wrong | 0.000 | 0.000 | 0.000 | не хуже |
| En rare form, 1 error(s)/right | 71.000 | 71.000 | 71.000 | не хуже |
| En rare form, 1 error(s)/wrong | 3.200 | 3.200 | 3.800 | хуже |
| En rare form, 2 error(s)/right | 1.000 | 1.000 | 1.000 | не хуже |
| En rare form, 2 error(s)/wrong | 4.100 | 4.100 | 4.100 | не хуже |
| En typo 1/all/right | 77.000 | 78.000 | 84.000 | не хуже |
| En typo 1/all/wrong | 1.900 | 1.800 | 2.500 | хуже |
| En typo 2/all/right | 63.000 | 63.000 | 63.000 | не хуже |
| En typo 2/all/wrong | 7.300 | 7.300 | 8.300 | хуже |
| En typo 3/all/right | 41.000 | 41.000 | 41.000 | не хуже |
| En typo 3/all/wrong | 6.800 | 6.800 | 7.300 | хуже |
| Ru frequent, 1 error(s)/right | 78.000 | 77.000 | 81.000 | не хуже |
| Ru frequent, 1 error(s)/wrong | 3.900 | 4.400 | 3.700 | не хуже |
| Ru frequent, 2 error(s)/right | 61.000 | 60.000 | 61.000 | не хуже |
| Ru frequent, 2 error(s)/wrong | 9.200 | 9.500 | 8.900 | не хуже |
| Ru layout+typo/all/right | 69.000 | 69.000 | 74.000 | не хуже |
| Ru layout+typo/all/wrong | 3.900 | 3.900 | 3.800 | не хуже |
| Ru layout/all/right | 99.000 | 99.000 | 99.000 | не хуже |
| Ru layout/all/wrong | 0.000 | 0.000 | 0.000 | не хуже |
| Ru rare form, 1 error(s)/right | 69.000 | 68.000 | 69.000 | не хуже |
| Ru rare form, 1 error(s)/wrong | 3.000 | 3.600 | 2.800 | не хуже |
| Ru rare form, 2 error(s)/right | 28.000 | 28.000 | 28.000 | не хуже |
| Ru rare form, 2 error(s)/wrong | 10.200 | 10.500 | 9.800 | не хуже |
| Ru typo 1/all/right | 79.000 | 79.000 | 85.000 | не хуже |
| Ru typo 1/all/wrong | 2.500 | 2.700 | 1.900 | не хуже |
| Ru typo 2/all/right | 63.000 | 63.000 | 63.000 | не хуже |
| Ru typo 2/all/wrong | 7.300 | 7.500 | 7.700 | хуже |
| Ru typo 3/all/right | 43.000 | 42.000 | 43.000 | не хуже |
| Ru typo 3/all/wrong | 5.900 | 6.200 | 6.100 | хуже |
| curated_fp | 7 | 7 | 6 | не хуже |
| frequent_changed | 0 | 0 | 0 | не хуже |
| unknown-fp/En Capitalised | 7.200 | 7.200 | 7.200 | не хуже |
| unknown-fp/En lower | 17.300 | 17.300 | 17.700 | хуже |
| unknown-fp/Ru Capitalised | 9.400 | 9.500 | 9.500 | хуже |
| unknown-fp/Ru lower | 28.900 | 28.900 | 29.000 | хуже |

## context_eval

| Метрика | 0.3.83 | 0.3.84 | 0.3.85 | 0.3.85 против 0.3.83 |
|---|---|---|---|---|
| en/layout+typo/right | 46.900 | 46.900 | 53.100 | не хуже |
| en/layout+typo/wrong | 7.260 | 7.380 | 7.730 | хуже |
| en/layout/right | 99.600 | 99.600 | 99.600 | не хуже |
| en/layout/wrong | 0.050 | 0.050 | 0.050 | не хуже |
| en/typo1/right | 70.000 | 69.900 | 77.600 | не хуже |
| en/typo1/wrong | 3.200 | 3.540 | 3.580 | хуже |
| en/typo2/right | 34.400 | 34.400 | 34.400 | не хуже |
| en/typo2/wrong | 14.200 | 14.360 | 16.490 | хуже |
| ru/layout+typo/right | 55.700 | 55.800 | 59.700 | не хуже |
| ru/layout+typo/wrong | 5.680 | 5.680 | 5.860 | хуже |
| ru/layout/right | 98.100 | 98.300 | 98.300 | не хуже |
| ru/layout/wrong | 0.260 | 0.260 | 0.260 | не хуже |
| ru/typo1/right | 74.400 | 74.800 | 79.900 | не хуже |
| ru/typo1/wrong | 3.330 | 4.330 | 3.890 | хуже |
| ru/typo2/right | 44.500 | 44.600 | 44.900 | не хуже |
| ru/typo2/wrong | 9.400 | 9.930 | 10.600 | хуже |

## typist_eval

Seed 76, 5000 фраз у `gramotey`, по 1000 у `torpyga` и `zlodey`.

| Метрика | 0.3.83 | 0.3.84 | 0.3.85 | 0.3.85 против 0.3.83 |
|---|---|---|---|---|
| gramotey/aggressive/fixed_pct | 2.225 | 2.207 | 2.356 | не хуже |
| gramotey/aggressive/fp | 12 | 7 | 6 | не хуже |
| gramotey/aggressive/missed_pct | 1.985 | 1.985 | 2.339 | хуже |
| gramotey/aggressive/wrong_pct | 0.171 | 0.188 | 0.252 | хуже |
| gramotey/cautious/fixed_pct | 1.874 | 1.749 | 1.996 | не хуже |
| gramotey/cautious/fp | 4 | 1 | 4 | не хуже |
| gramotey/cautious/missed_pct | 2.181 | 2.337 | 2.213 | хуже |
| gramotey/cautious/wrong_pct | 0.194 | 0.151 | 0.166 | не хуже |
| gramotey/normal/fixed_pct | 2.183 | 2.198 | 2.231 | не хуже |
| gramotey/normal/fp | 15 | 12 | 21 | хуже |
| gramotey/normal/missed_pct | 2.210 | 2.248 | 2.017 | не хуже |
| gramotey/normal/wrong_pct | 0.173 | 0.197 | 0.214 | хуже |
| torpyga/aggressive/fixed_pct | 16.351 | 16.095 | 18.191 | не хуже |
| torpyga/aggressive/fp | 3 | 2 | 0 | не хуже |
| torpyga/aggressive/missed_pct | 18.061 | 17.886 | 17.007 | не хуже |
| torpyga/aggressive/wrong_pct | 3.137 | 3.274 | 3.064 | не хуже |
| torpyga/cautious/fixed_pct | 13.914 | 14.665 | 14.868 | не хуже |
| torpyga/cautious/fp | 0 | 0 | 0 | не хуже |
| torpyga/cautious/missed_pct | 21.956 | 20.894 | 21.279 | не хуже |
| torpyga/cautious/wrong_pct | 2.325 | 2.425 | 2.558 | хуже |
| torpyga/normal/fixed_pct | 16.632 | 16.483 | 17.527 | не хуже |
| torpyga/normal/fp | 3 | 4 | 2 | не хуже |
| torpyga/normal/missed_pct | 18.306 | 18.301 | 17.517 | не хуже |
| torpyga/normal/wrong_pct | 2.525 | 2.736 | 2.565 | хуже |
| zlodey/aggressive/fixed_pct | 17.295 | 15.783 | 16.994 | хуже |
| zlodey/aggressive/fp | 1 | 0 | 2 | хуже |
| zlodey/aggressive/missed_pct | 34.000 | 34.803 | 33.210 | не хуже |
| zlodey/aggressive/wrong_pct | 5.371 | 5.659 | 5.696 | хуже |
| zlodey/cautious/fixed_pct | 12.227 | 12.169 | 13.482 | не хуже |
| zlodey/cautious/fp | 1 | 1 | 1 | не хуже |
| zlodey/cautious/missed_pct | 39.503 | 38.894 | 38.613 | не хуже |
| zlodey/cautious/wrong_pct | 4.599 | 4.457 | 4.432 | не хуже |
| zlodey/normal/fixed_pct | 15.006 | 15.007 | 16.192 | не хуже |
| zlodey/normal/fp | 6 | 4 | 8 | хуже |
| zlodey/normal/missed_pct | 36.499 | 36.373 | 35.060 | не хуже |
| zlodey/normal/wrong_pct | 4.879 | 5.021 | 4.824 | не хуже |

## Команды

Все команды из корня репозитория. Каталог результата задаётся переменной `OUT`.

```
T="cargo test --offline --release -p layout_keeper_core"
$T --test correction_eval -- --ignored --nocapture --test-threads=2 > $OUT/correction.txt
$T --test context_eval -- --ignored --nocapture > $OUT/context.txt
EVAL_SEED=76 EVAL_N=5000 EVAL_N_OTHER=1000 EVAL_OUT=$OUT/typist_eval.md $T --test typist_eval typist_eval -- --ignored --nocapture --exact
$T --test typist_real_failures -- --include-ignored --nocapture > $OUT/real_failures.txt
$T --test typo_audit -- --include-ignored --nocapture
SWEEP_OUT=$OUT/sweep.txt $T --test typist_sweep typist_sweep -- --ignored --nocapture --exact
python3 scripts/release_gate.py $OUT $BASE
```

`SWEEP_SECTIONS=1,2,4` запускает только нужные разделы `typist_sweep`. `EVAL_ALL=1` выводит в `typist_eval` все примеры, а не 30 первых.
