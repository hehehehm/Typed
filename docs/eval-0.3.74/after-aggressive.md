# Correction eval: 0.3.74 aggressive

Cells: recall (fixed right) / wrong fix, per word length; for layout+typo `(+N%)` = layout switched but the slip left. Typo 2 from 4 letters, typo 3 from 6.

## Ru: recall / wrong

| kind | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | all |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| typo 1 | 0% / 2.2% | 58% / 6.0% | 61% / 3.9% | 79% / 4.7% | 80% / 3.4% | 86% / 2.5% | 90% / 3.4% | 92% / 0.8% | 91% / 0.0% | 93% / 0.0% | 91% / 0.9% | **80% / 2.5%** (n=1181) |
| typo 2 | — | — | 32% / 19.8% | 38% / 20.0% | 50% / 12.2% | 71% / 9.5% | 75% / 9.5% | 81% / 5.3% | 81% / 6.0% | 92% / 0.8% | 90% / 3.4% | **69% / 9.3%** (n=1021) |
| typo 3 | — | — | — | — | 38% / 7.8% | 60% / 8.3% | 56% / 15.1% | 64% / 10.8% | 79% / 4.2% | 84% / 3.3% | 80% / 5.0% | **66% / 7.8%** (n=834) |
| layout | 68% / 0.0% | 93% / 0.0% | 98% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | **98% / 0.0%** (n=1222) |
| layout+typo | 0% / 6.1% | 42% (+2%) / 12.4% | 39% (+6%) / 15.1% | 48% (+7%) / 15.3% | 70% (+8%) / 6.2% | 76% (+6%) / 4.3% | 79% (+8%) / 3.3% | 87% (+5%) / 1.7% | 86% (+8%) / 2.5% | 89% (+5%) / 0.8% | 91% (+3%) / 0.0% | **70% (+6%) / 5.8%** (n=1143) |

## En: recall / wrong

| kind | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | all |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| typo 1 | 0% / 5.9% | 47% / 8.4% | 58% / 3.3% | 69% / 3.6% | 74% / 3.4% | 86% / 0.9% | 84% / 1.7% | 88% / 0.8% | 91% / 0.0% | 95% / 0.0% | 96% / 0.8% | **78% / 2.2%** (n=1150) |
| typo 2 | — | — | 29% / 21.4% | 36% / 23.3% | 45% / 19.1% | 71% / 10.3% | 75% / 8.8% | 88% / 2.6% | 92% / 3.4% | 93% / 1.7% | 96% / 0.0% | **71% / 9.6%** (n=1010) |
| typo 3 | — | — | — | — | 31% / 21.2% | 51% / 11.0% | 61% / 15.0% | 66% / 11.0% | 84% / 1.7% | 80% / 2.5% | 87% / 4.2% | **66% / 9.5%** (n=834) |
| layout | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | **100% / 0.0%** (n=1235) |
| layout+typo | 0% / 0.0% | 37% (+12%) / 13.8% | 39% (+17%) / 7.9% | 47% (+12%) / 16.3% | 64% (+12%) / 9.3% | 71% (+11%) / 1.7% | 81% (+8%) / 0.8% | 83% (+8%) / 0.8% | 78% (+12%) / 0.0% | 87% (+8%) / 0.0% | 95% (+3%) / 0.0% | **69% (+10%) / 4.3%** (n=1113) |

## False positives: unknown-word proxy (changed %)

| set | 2-3 | 4 | 5-6 | 7-8 | 9-12 | all |
|---|---|---|---|---|---|---|
| Ru lower | 0.0% | 27.3% | 37.7% | 43.3% | 27.8% | **31.9%** (n=3000) |
| Ru Capitalised | 0.0% | 22.7% | 12.1% | 8.2% | 3.0% | **5.0%** (n=3000) |
| En lower | 0.0% | 30.4% | 43.0% | 38.4% | 25.8% | **30.8%** (n=3000) |
| En Capitalised | 0.0% | 13.0% | 21.5% | 10.4% | 4.0% | **7.5%** (n=3000) |

## Curated FP set (names, brands, code, URLs, slang): changed 15/297 (5.1%)

["Безос→Безо", "Tatyana→Tatiana", "rustc→rustic", "serde→see", "actix→arctic", "struct→strict", "const→cost", "usize→suzie", "isize→izzie", "РФ→HA", "ничо→ничто", "мерж→мердж", "Bizon→Bison", "Shmel→Shel", "Димон→Дион"]

## Known frequent words changed: 0/1320

[]

## Engine::decide latency (recall cases): mean 3.03 ms, p99 22.45 ms, max 46.42 ms (нен)

## Wrong-fix examples

* En layout+typo: кгтт→кот (run), фпщщ→aigoo (ago), сгее→сне (cut), Ещщз→Troop (top), иккщ→burro (bro), щррн→щрн (ohh), икки→кики (brb), пое→поле (git), Сфии→Софии (cab), аффшк→ашк (fair), шшвс→idc (iirc), еешы→tis (tits)
* En typo 1: bv→им (bc), gvg→пап (gg), ibsa→isa (ibs), ino→into (imo), rnj→кто (rnn), ssao→sao (sso), wgo→who (wto), heuh→huh (heh), thuy→ергн (thy), tist→twist (tits), glenm→glenn (glen), yeit→yet (yeti)
* En typo 2: kissse→kisses (kiss), jokew→jokes (joke), Wetd→Цуев (west), aipe→ape (apis), boknr→born (bond), thyys→ернны (thus), tjte→еоеу (tits), Fna→Атф (fran), gklne→gone (glen), lafn→lan (lang), Ryys→Rhys (rays), libh→дшир (limb)
* En typo 3: choeeo→choo (choose), strikjes→strikes (strike), feezesw→freezes (freeze), taynma→tanya (trauma), fceukr→асуглк (fucker), easger→eager (easter), truuuss→truss (trusts), mzarkr→mark (marker), bhnc→иртс (brunch), daiku→daily (darius), stylplew→style (styles), chieec→chief (chiefs)
* FP unknown En Capitalised: Aamft→Aft, Abhorrition→Abortion, Abroma→Aroma, Abysms→Abyss, Acons→Acorns, Adjacency→Adjacent, Adjudgment→Adjustment, Adplls→Ads, Aduwa→Ada, Adverted→Averted, Agued→Argued, Ahaz→Ayaz
* FP unknown En lower: aamft→aft, abdelnasir→abdulnasir, abhorrition→abhorration, ablutions→ablations, aborsements→agreements, abroma→aroma, absolutory→absolutely, abysms→abyss, acanthus→acanthous, accolle→accole, accouters→accounts, accruals→actual
* FP unknown Ru Capitalised: Аврикам→Африкам, Автоворов→Авторов, Агретине→Аргентине, Адресует→Арестует, Азоматом→Ароматом, Айраном→Райаном, Айресовен→Арестован, Албберте→Альберте, Алды→Лады, Алисаг→Алиса, Алиу→Али, Альбитах→Альбертах
* FP unknown Ru lower: абазеровной→абузеровной, абдирайимами→абдираимами, абдрауфов→абдурауфов, абдрашитове→абрашитове, абдубасита→абдулбасита, абдувахита→абдуваита, абдугаффара→абдугафара, абдужелилами→абдужалилами, абдулажонам→абдулажанам, абдулкайюмов→абдулкаюмов, абдулхаера→абдулхаира, абдулхакимам→абдухакимам
* Ru layout+typo: b[[→их[ (их), cre→care (су), gjjl→gil (под), vffv→мама (мам), ijee→ike (шоу), itw→its (шеф), vvvf→мама (ммм), depo→deep (вуз), juj'→ого' (огэ), cqt→cat (сей), ebf→ума (ура), yjuu→yuu (ног)
* Ru typo 1: тьт→тот (ть), ещ→to (еще), Руук→Руку (рук), даж→lf; (даю), пвор→пор (пво), тлли→талли (тли), Злог→Залог (зло), будуу→,else (буду), прис→пэрис (пирс), Кыин→Rsby (квин), жияв→жив (живя), взоле→воле (возле)
* Ru typo 2: бву→,de (буду), пьр→пор (пора), полеен→полезен (поле), спм→сам (спал), бнаа→бан (банк), бюнр→бнр (бюро), ткаеи→ткани (таки), прирас→прирос (пирс), наеус→нас (несу), Кввинн→Квинн (квин), ваеныу→dftyse (вены), пргд→прод (пред)
* Ru typo 3: пвермы→пермы (первым), нкшиии→наши (нашими), санйт→сайт (станут), отрдеел→отдел (отделе), мнноогм→многом (многим), увсауть→усвоить (уснуть), грж→uh; (гараже), геам→геем (гением), покыа→пока (подача), скасзам→скажем (сказали), ввзаьть→взять (вызвать), Вщзгянт→Dopuzyn (взгляни)

## Miss examples

* En layout+typo: Цыу (we), шпа (if), фыц (as), Ирн (by), нфф (ya), нз (yo), пм (gg), олл (jk), гчч (ux), Сйк (car), щлак (okr), ыыт (sns)
* En typo 1: hii (hi), jkk (jk), jss (js), tyt (ty), hadd (had), seaq (sea), icd (ice), Vna (van), bbr (brb), cmk (cmo), fyo (fyi), rarf (rar)
* En typo 2: awyg (away), beatgf (beat), grp (grow), dogsaa (dogs), brdr (brad), scazk (sack), Maddc (marc), nko (niko), Mewz (mesa), Fllae (flame), ethse (ethel), chakol (chalk)
* En typo 3: tosma (thomas), pedodx (period), loivtg (loving), whhopss (whoops), traed (traced), ndwetsr (newest), Weeredd (wedded), maolnoo (manolo), paalaatee (palate), dajjncihn (dancin), Herrnbiia (hernia), Hsaninb (hanging)
* Ru layout: [v (хм), g; (пж), w, (цб), lf. (даю), ju' (огэ)
* Ru layout+typo: ;g (же), yg (ни), jqq (ой), rfe (как), jyt (она), nhv (три), ifui (шаг), f[c (аэс), ,nhb (бти), pfmj (зао), byg (инн), ayyc (фнс)
* Ru typo 1: нен (не), ог (он), Зв (за), пто (по), сш (со), ож (ох), оф (ой), таа (та), Адд (ад), мбб (мб), цбю (цб), енн (ен)
* Ru typo 2: мааам (мама), биьол (билл), миилр (мили), бьто (бьет), ньы (ноты), доби (дьюи), жвмя (живя), свеи (своем), люблли (любил), шаы (шансы), Обещфй (общей), нал (налей)
* Ru typo 3: пемде (поедем), суипрть (судить), меясццыы (месяцы), люмбо (любому), выцсоие (высоте), верррхм (верхом), амеляи (амелия), моьиск (мостик), соерл (сгорел), Моксвув (москву), пипятно (приятно), зепапису (записку)
