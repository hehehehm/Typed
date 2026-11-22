# Метрики

Три версии на одних и тех же наборах. 0.3.83 и 0.3.84 собраны из релизных коммитов, 0.3.85 из ветки `wip-0.3.85` (коммит `20430b4`). Харнесс `typist_sweep` один и тот же для всех трёх версий.

Как читать:

- `right` и `recall`: доля исправленных правильно, больше лучше.
- `wrong`: доля неверных исправлений, меньше лучше.
- `fp`: число ложных срабатываний на чистом тексте, меньше лучше.
- `fail` в разделах `fp_*`: ложные срабатывания на миллион проверок, меньше лучше.
- `missed_pct`: доля ошибок без исправления, меньше лучше.

## Итог гейта

Гейт `scripts/release_gate.py` без допусков: любая метрика хуже базовой версии валит релиз.

- 0.3.85 против 0.3.83: **не пройден**, хуже 27 метрик. Все они в доле неверных исправлений и у тестеров. Все разделы `typist_sweep` не хуже 0.3.83.
- 0.3.84 против 0.3.83: тоже не пройден, хуже 36 метрик.
- `typist_real_failures`: 49 из 49. Curated FP: 6 из 315. Частые слова: 0 из 1320. `typo_audit`: 0 ложных, 0 пропусков из 40.

Метрики хуже 0.3.83: `context/en/layout+typo/wrong`, `context/en/typo1/wrong`, `context/en/typo2/wrong`, `context/ru/layout+typo/wrong`, `context/ru/typo1/wrong`, `context/ru/typo2/wrong`, `correction/En frequent, 2 error(s)/wrong`, `correction/En rare form, 1 error(s)/wrong`, `correction/En typo 1/all/wrong`, `correction/En typo 2/all/wrong`, `correction/En typo 3/all/wrong`, `correction/Ru typo 2/all/wrong`, `correction/Ru typo 3/all/wrong`, `correction/unknown-fp/En lower`, `correction/unknown-fp/Ru Capitalised`, `correction/unknown-fp/Ru lower`, `typist/gramotey/aggressive/missed_pct`, `typist/gramotey/aggressive/wrong_pct`, `typist/gramotey/cautious/missed_pct`, `typist/gramotey/normal/fp`, `typist/gramotey/normal/wrong_pct`, `typist/torpyga/cautious/wrong_pct`, `typist/torpyga/normal/wrong_pct`, `typist/zlodey/aggressive/fixed_pct`, `typist/zlodey/aggressive/fp`, `typist/zlodey/aggressive/wrong_pct`, `typist/zlodey/normal/fp`.

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
