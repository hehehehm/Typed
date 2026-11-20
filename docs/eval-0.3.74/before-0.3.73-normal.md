# Correction eval: 0.3.73 normal

Cells: recall (fixed right) / wrong fix, per word length; for layout+typo `(+N%)` = layout switched but the slip left. Typo 2 from 4 letters, typo 3 from 6.

## Ru: recall / wrong

| kind | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | all |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| typo 1 | 0% / 2.2% | 55% / 5.0% | 63% / 2.9% | 79% / 3.7% | 80% / 2.6% | 83% / 2.5% | 90% / 3.4% | 92% / 0.8% | 90% / 0.0% | 92% / 0.0% | 91% / 0.9% | **79% / 2.1%** (n=1181) |
| typo 2 | — | — | 26% / 15.8% | 32% / 16.2% | 40% / 8.7% | 62% / 7.8% | 71% / 4.3% | 74% / 3.5% | 79% / 5.1% | 88% / 0.8% | 87% / 2.5% | **63% / 7.0%** (n=1021) |
| typo 3 | — | — | — | — | 26% / 5.2% | 35% / 5.0% | 40% / 10.9% | 42% / 5.0% | 56% / 4.2% | 56% / 1.7% | 52% / 4.2% | **44% / 5.2%** (n=834) |
| layout | 68% / 0.0% | 93% / 0.0% | 98% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | **98% / 0.0%** (n=1222) |
| layout+typo | 0% / 6.1% | 38% (+2%) / 7.9% | 40% (+6%) / 8.6% | 48% (+11%) / 8.1% | 70% (+9%) / 2.7% | 75% (+8%) / 3.4% | 78% (+10%) / 1.7% | 87% (+5%) / 0.9% | 86% (+8%) / 2.5% | 88% (+5%) / 0.8% | 91% (+3%) / 0.0% | **70% (+7%) / 3.5%** (n=1143) |

## En: recall / wrong

| kind | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | all |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| typo 1 | 0% / 2.9% | 45% / 7.2% | 60% / 2.2% | 68% / 2.7% | 73% / 3.4% | 84% / 0.9% | 83% / 1.7% | 87% / 0.8% | 91% / 0.0% | 95% / 0.0% | 96% / 0.8% | **77% / 1.8%** (n=1150) |
| typo 2 | — | — | 22% / 20.4% | 27% / 19.4% | 25% / 11.8% | 59% / 7.8% | 71% / 3.5% | 81% / 2.6% | 86% / 2.6% | 90% / 1.7% | 91% / 0.0% | **63% / 7.3%** (n=1010) |
| typo 3 | — | — | — | — | 19% / 16.1% | 32% / 10.2% | 38% / 10.0% | 44% / 6.8% | 51% / 1.7% | 55% / 1.7% | 53% / 1.7% | **42% / 6.8%** (n=834) |
| layout | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | **100% / 0.0%** (n=1235) |
| layout+typo | 0% / 0.0% | 32% (+12%) / 10.8% | 42% (+18%) / 5.6% | 49% (+15%) / 6.7% | 65% (+13%) / 7.6% | 71% (+12%) / 0.0% | 80% (+9%) / 0.0% | 82% (+9%) / 0.8% | 78% (+12%) / 0.0% | 87% (+8%) / 0.0% | 95% (+3%) / 0.0% | **69% (+11%) / 2.6%** (n=1113) |

## False positives: unknown-word proxy (changed %)

| set | 2-3 | 4 | 5-6 | 7-8 | 9-12 | all |
|---|---|---|---|---|---|---|
| Ru lower | 0.0% | 27.3% | 20.2% | 28.7% | 24.8% | **25.3%** (n=3000) |
| Ru Capitalised | 0.0% | 22.7% | 11.7% | 7.9% | 2.9% | **4.8%** (n=3000) |
| En lower | 0.0% | 17.4% | 26.5% | 18.7% | 15.3% | **17.2%** (n=3000) |
| En Capitalised | 0.0% | 13.0% | 20.4% | 9.4% | 3.8% | **6.9%** (n=3000) |

## Curated FP set (names, brands, code, URLs, slang): changed 12/297 (4.0%)

["Безос→Безо", "Tatyana→Tatiana", "rustc→rustic", "actix→arctic", "struct→strict", "const→cost", "РФ→HA", "ничо→ничто", "мерж→мердж", "Bizon→Bison", "Shmel→Shel", "Димон→Дион"]

## Known frequent words changed: 0/1320

[]

## Engine::decide latency (recall cases): mean 2.81 ms, p99 20.55 ms, max 40.72 ms (ghtcktltim)

## Wrong-fix examples

* En layout+typo: фпщщ→aigoo (ago), сгее→curt (cut), Ещщз→Troop (top), иккщ→burro (bro), щррн→щрн (ohh), икки→кики (brb), Сфии→Софии (cab), аффшк→ашк (fair), шшвс→idc (iirc), еешы→tis (tits), кйфлу→кайлу (rake), ешуут→teen (tien)
* En typo 1: bv→им (bc), ibsa→isa (ibs), ino→into (imo), rnj→кто (rnn), ssao→sao (sso), wgo→who (wto), thuy→ергн (thy), tist→twist (tits), glenm→glenn (glen), kellyy→лудднн (kelly), scalop→scallop (scalp), Lyrid→Lurid (lyric)
* En typo 2: kissse→kisses (kiss), jokew→jokes (joke), Wetd→Цуев (west), aipe→ape (apis), thyys→ернны (thus), tjte→еоеу (tits), Fna→Атф (fran), gklne→gone (glen), lafn→lan (lang), Ryys→Rhys (rays), libh→дшир (limb), Rwegf→Кцупа (reef)
* En typo 3: choeeo→choo (choose), strikjes→strikes (strike), fceukr→асуглк (fucker), easger→eager (easter), truuuss→truss (trusts), Ethidxx→Ethic (ethics), bhnc→иртс (brunch), rekmrkk→кульклл (remark), senssze→sense (sneeze), nooee→noose (nobles), bnred→bred (bribed), paanos→pianos (panels)
* FP unknown En Capitalised: Aamft→Aft, Abroma→Aroma, Abysms→Abyss, Acons→Acorns, Adjacency→Adjacent, Adjudgment→Adjustment, Adverted→Averted, Agued→Argued, Ahaz→Ayaz, Alightment→Alignment, Alleygating→Allegation, Aptos→Apos
* FP unknown En lower: aamft→aft, abdelnasir→abdulnasir, abhorrition→abhorration, ablutions→ablations, abroma→aroma, abysms→abyss, acanthus→acanthous, accolle→accole, acharnian→acharnean, acons→acorns, acrophobias→acarophobias, adamancies→adamances
* FP unknown Ru Capitalised: Аврикам→Африкам, Автоворов→Авторов, Агретине→Аргентине, Адресует→Арестует, Азоматом→Ароматом, Айраном→Райаном, Албберте→Альберте, Алды→Лады, Алисаг→Алиса, Алиу→Али, Альбитах→Альбертах, Альфоносах→Альфонсах
* FP unknown Ru lower: абазеровной→абузеровной, абдирайимами→абдираимами, абдрауфов→абдурауфов, абдрашитове→абрашитове, абдубасита→абдулбасита, абдувахита→абдуваита, абдугаффара→абдугафара, абдужелилами→абдужалилами, абдулажонам→абдулажанам, абдулкайюмов→абдулкаюмов, абдулхаера→абдулхаира, абдулхакимам→абдухакимам
* Ru layout+typo: b[[→их[ (их), cre→care (су), vffv→мама (мам), itw→its (шеф), vvvf→мама (ммм), juj'→ого' (огэ), cqt→cat (сей), yjuu→yuu (ног), Ybvk→Ним (нил), ;ltnn→;lynn (ждет), yt,jh→набор (небо), vyj.,→мною, (мною)
* Ru typo 1: тьт→тот (ть), ещ→to (еще), Руук→Руку (рук), даж→lf; (даю), тлли→талли (тли), Злог→Залог (зло), будуу→,else (буду), прис→пэрис (пирс), Кыин→Rsby (квин), взоле→воле (возле), Барнни→Банни (барни), Мэндди→Мэдди (мэнди)
* Ru typo 2: бву→,de (буду), пьр→пор (пора), полеен→полезен (поле), спм→сам (спал), бюнр→бнр (бюро), ткаеи→ткани (таки), Кввинн→Квинн (квин), ваеныу→dftyse (вены), пргд→прод (пред), Блав→Бла (бала), двно→давно (дань), бкок→бок (берк)
* Ru typo 3: пвермы→пермы (первым), санйт→сайт (станут), мнноогм→многом (многим), грж→uh; (гараже), геам→геем (гением), покыа→пока (подача), Вщзгянт→Dopuzyn (взгляни), поьзы→пользы (полезны), квцер→карцер (канцлер), явиьлс→явился (явилась), нвесен→нанесен (невесте), хущиан→хулиан (хулиган)

## Miss examples

* En layout+typo: Цыу (we), шпа (if), фыц (as), Ирн (by), нфф (ya), нз (yo), олл (jk), ечы (ts), нгщ (you), фуп (age), ышлн (sky), иеф (btw)
* En typo 1: hii (hi), gvg (gg), gzx (gz), jkk (jk), uxz (ux), olr (old), Hmk (hmm), sexs (sex), cuo (cup), kpj (kpi), rarf (rar), snss (sns)
* En typo 2: awyg (away), Kis (kids), sengg (sent), beatgf (beat), Dsuft (dust), brdr (brad), scazk (sack), iwt (iast), oqos (oaks), huryu (hurry), Angll (angel), molpty (molly)
* En typo 3: fiuffre (figure), priwlonn (prison), hidhen (hidden), puerec (pierce), shnhffs (sniffs), sctto (scotty), sytwelr (steele), Vrigk (virgil), poseivc (poetic), Weeredd (wedded), gnni (gianni), cunnnks (chunks)
* Ru layout: [v (хм), g; (пж), w, (цб), ;rn (жкт)
* Ru layout+typo: ;g (же), yg (ни), jqq (ой), rfe (как), jyt (она), ccv (сам), ve;; (муж), v;v (мэм), "gvb (эми), ifui (шаг), f[c (аэс), <ffy (бан)
* Ru typo 1: нен (не), ог (он), Зв (за), пто (по), сш (со), ож (ох), оф (ой), таа (та), Адд (ад), мбб (мб), цбю (цб), енн (ен)
* Ru typo 2: мааам (мама), далрр (дали), Роаж (рода), вииине (вине), наеус (несу), врдия (видя), аеннди (анди), мбаа (мала), блбж (блюд), жвмя (живя), Згаепт (знает), Пиьоть (пилот)
* Ru typo 3: Пршстоош (просто), зеааубдь (забудь), пговеелл (провел), гртлду (городу), псоутй (пустой), ммнуат (минута), Убеедддил (убедил), шертлогк (шерлок), сеацрум (сайрус), Дизе (дизайн), Сщыштип (слышим), неныжйй (нежный)
