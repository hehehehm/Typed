# Correction eval: aggressive

Cells: recall (fixed right) / wrong fix, per word length; for layout+typo `(+N%)` = layout switched but the slip left. Typo 2 from 4 letters, typo 3 from 6.

## Ru: recall / wrong

| kind | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | all |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| typo 1 | 0% / 2.1% | 59% / 5.3% | 60% / 4.9% | 79% / 4.7% | 80% / 3.4% | 86% / 2.5% | 90% / 3.4% | 92% / 0.8% | 91% / 0.0% | 93% / 0.0% | 91% / 0.9% | **80% / 2.5%** (n=1179) |
| typo 2 | — | — | 32% / 18.8% | 39% / 19.0% | 51% / 11.3% | 72% / 9.5% | 75% / 7.8% | 80% / 4.4% | 81% / 5.1% | 92% / 0.8% | 91% / 3.4% | **69% / 8.6%** (n=1021) |
| typo 3 | — | — | — | — | 40% / 7.8% | 60% / 7.5% | 57% / 14.3% | 64% / 10.8% | 79% / 4.2% | 86% / 1.7% | 80% / 5.0% | **67% / 7.3%** (n=834) |
| layout | 68% / 0.0% | 88% / 0.0% | 98% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | **98% / 0.0%** (n=1222) |
| layout+typo | 0% / 6.1% | 38% (+5%) / 11.4% | 37% (+6%) / 16.0% | 48% (+7%) / 14.4% | 70% (+8%) / 6.2% | 77% (+5%) / 4.3% | 80% (+7%) / 3.3% | 88% (+4%) / 1.7% | 86% (+8%) / 2.5% | 89% (+5%) / 0.8% | 91% (+3%) / 0.0% | **70% (+6%) / 5.7%** (n=1143) |

## En: recall / wrong

| kind | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | all |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| typo 1 | 0% / 0.0% | 52% / 7.2% | 57% / 3.2% | 70% / 3.6% | 74% / 3.4% | 86% / 0.9% | 84% / 1.7% | 88% / 0.8% | 91% / 0.0% | 95% / 0.0% | 96% / 0.8% | **78% / 1.9%** (n=1148) |
| typo 2 | — | — | 28% / 20.2% | 34% / 23.5% | 45% / 19.1% | 72% / 10.3% | 75% / 9.6% | 87% / 3.5% | 92% / 3.4% | 93% / 1.7% | 96% / 0.0% | **70% / 9.7%** (n=1010) |
| typo 3 | — | — | — | — | 31% / 21.2% | 52% / 11.0% | 62% / 15.8% | 65% / 11.0% | 83% / 1.7% | 80% / 2.5% | 88% / 4.2% | **66% / 9.6%** (n=834) |
| layout | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | **100% / 0.0%** (n=1237) |
| layout+typo | 0% / 3.4% | 37% (+7%) / 13.4% | 37% (+18%) / 7.7% | 47% (+12%) / 16.3% | 64% (+11%) / 10.2% | 71% (+11%) / 1.7% | 81% (+8%) / 0.8% | 83% (+8%) / 0.8% | 78% (+12%) / 0.0% | 87% (+8%) / 0.0% | 95% (+3%) / 0.0% | **69% (+9%) / 4.5%** (n=1118) |

## False positives: unknown-word proxy (changed %)

| set | 2-3 | 4 | 5-6 | 7-8 | 9-12 | all |
|---|---|---|---|---|---|---|
| Ru lower | 0.0% | 27.3% | 35.9% | 42.8% | 27.0% | **31.1%** (n=3000) |
| Ru Capitalised | 0.0% | 22.7% | 12.1% | 8.2% | 3.0% | **5.0%** (n=3000) |
| En lower | 0.0% | 30.4% | 42.3% | 37.9% | 25.8% | **30.6%** (n=3000) |
| En Capitalised | 0.0% | 13.0% | 21.5% | 10.4% | 4.0% | **7.4%** (n=3000) |

## Curated FP set (names, brands, code, URLs, slang): changed 15/297 (5.1%)

["Безос→Безо", "Tatyana→Tatiana", "rustc→rustic", "serde→see", "actix→arctic", "struct→strict", "const→cost", "usize→suzie", "isize→izzie", "РФ→HA", "ничо→ничто", "мерж→мердж", "Bizon→Bison", "Shmel→Shel", "Димон→Дион"]

## Known frequent words changed: 0/1320

[]

## Wrong-fix examples

* En layout+typo: ппп→пап (gg), кгтт→кот (run), фпщщ→aigoo (ago), сгее→сне (cut), Ещщз→Troop (top), иккщ→burro (bro), щррн→щрн (ohh), ств→стив (cdn), иекс→икс (btc), Сфии→Софии (cab), аффшк→ашк (fair), кщщад→кад (rofl)
* En typo 1: irlk→ilk (irl), btcf→иеса (btc), gked→плув (gke), tbhu→еирг (tbh), heuh→huh (heh), thuy→ергн (thy), tist→twist (tits), glenm→glenn (glen), yeit→yet (yeti), kellyy→лудднн (kelly), scalop→scallop (scalp), whien→when (whine)
* En typo 2: kissse→kisses (kiss), jokew→jokes (joke), Wetd→Цуев (west), boknr→born (bond), thyys→ернны (thus), tjte→еоеу (tits), Fna→Атф (fran), gklne→gone (glen), lafn→lan (lang), Ryys→Rhys (rays), libh→дшир (limb), Rwegf→Кцупа (reef)
* En typo 3: choeeo→choo (choose), strikjes→strikes (strike), feezesw→freezes (freeze), taynma→tanya (trauma), fceukr→асуглк (fucker), easger→eager (easter), truuuss→truss (trusts), mzarkr→mark (marker), bhnc→иртс (brunch), daiku→daily (darius), stylplew→style (styles), chieec→chief (chiefs)
* FP unknown En Capitalised: Aamft→Aft, Abhorrition→Abortion, Abroma→Aroma, Abysms→Abyss, Acons→Acorns, Adjacency→Adjacent, Adjudgment→Adjustment, Adplls→Ads, Aduwa→Ada, Adverted→Averted, Agued→Argued, Ahaz→Ayaz
* FP unknown En lower: aamft→aft, abdelnasir→abdulnasir, abhorrition→abhorration, ablutions→ablations, aborsements→agreements, abroma→aroma, absolutory→absolutely, abysms→abyss, acanthus→acanthous, accolle→accole, accouters→accounts, accruals→actual
* FP unknown Ru Capitalised: Аврикам→Африкам, Автоворов→Авторов, Агретине→Аргентине, Адресует→Арестует, Азоматом→Ароматом, Айраном→Райаном, Айресовен→Арестован, Албберте→Альберте, Алды→Лады, Алисаг→Алиса, Алиу→Али, Альбитах→Альбертах
* FP unknown Ru lower: абазеровной→абузеровной, абдирайимами→абдираимами, абдрауфов→абдурауфов, абдрашитове→абрашитове, абдубасита→абдулбасита, абдувахита→абдуваита, абдугаффара→абдугафара, абдужелилами→абдужалилами, абдулажонам→абдулажанам, абдулкайюмов→абдулкаюмов, абдулхаера→абдулхаира, абдулхакимам→абдухакимам
* Ru layout+typo: b[[→их[ (их), cre→care (су), gjjl→gil (под), vffv→мама (мам), ijee→ike (шоу), itw→its (шеф), vvvf→мама (ммм), dgre→dre (впк), fncc→fcc (атс), ebf→ума (ура), yjuu→yuu (ног), Ybvk→Ним (нил)
* Ru typo 1: тьт→тот (ть), ещ→to (еще), Руук→Руку (рук), даж→lf; (даю), ккк→как (кек), Злог→Залог (зло), будуу→,else (буду), Ладнр→Ладно (ладн), прис→пэрис (пирс), Кыин→Rsby (квин), жияв→жив (живя), взоле→воле (возле)
* Ru typo 2: бву→,de (буду), пьр→пор (пора), полеен→полезен (поле), спм→сам (спал), бнаа→бан (банк), бюнр→бнр (бюро), ткаеи→ткани (таки), наеус→нас (несу), Кввинн→Квинн (квин), ваеныу→dftyse (вены), пргд→прод (пред), Блав→Бла (бала)
* Ru typo 3: пвермы→пермы (первым), поксда→поезда (покеда), нкшиии→наши (нашими), санйт→сайт (станут), отрдеел→отдел (отделе), мнноогм→многом (многим), грж→uh; (гараже), геам→геем (гением), покыа→пока (подача), ввзаьть→взять (вызвать), Вщзгянт→Dopuzyn (взгляни), угоро→утро (угрозой)

## Miss examples

* En layout+typo: Цыу (we), шпа (if), фыц (as), Ирн (by), нфф (ya), нз (yo), оя (js), тзз (np), цдз (wp), фуп (age), ышлн (sky), ыяд (ssl)
* En typo 1: hii (hi), gsz (gz), npp (np), tyt (ty), hwo (how), Czr (car), seaq (sea), mapl (map), dgm (dmg), ibss (ibs), Ggt (gtg), fgw (ftw)
* En typo 2: awyg (away), beatgf (beat), grp (grow), dogsaa (dogs), ried (rude), ldasd (lads), diaa (dial), rwoe (rake), ccae (cade), drnqak (drank), Tqese (tease), Loahss (loans)
* En typo 3: tosma (thomas), pedodx (period), loivtg (loving), whhopss (whoops), traed (traced), ndwetsr (newest), Weeredd (wedded), maolnoo (manolo), paalaatee (palate), dajjncihn (dancin), Herrnbiia (hernia), Hsaninb (hanging)
* Ru layout: [v (хм), [p (хз), h, (рб), ve; (муж), cg, (спб), nt, (теб)
* Ru layout+typo: ;g (же), yg (ни), jqq (ой), g. (пж), nmm (ть), kff (ла), jyt (она), blv (иди), yfii (наш), Ydl (над), ,juu (бог), ve;; (муж)
* Ru typo 1: нен (не), ог (он), Зв (за), пто (по), сш (со), ож (ох), оф (ой), таа (та), Адд (ад), рлб (рб), чее (че), Дма (дам)
* Ru typo 2: далрр (дали), льдавв (льда), хаат (хант), дцл (дэйл), жл (жюли), кул (кубе), чтолч (чтобы), Згаепт (знает), свеи (своем), снл (сняла), брасн (барнс), Турид (труди)
* Ru typo 3: Ождд (одежду), суипрть (судить), псоутй (пустой), меясццыы (месяцы), люмбо (любому), Счьпов (счетов), удийецк (убийце), Сщыштип (слышим), ночнууу (ночную), пзоаеет (подает), прридт (приедет), Тяелжяа (тяжелая)
