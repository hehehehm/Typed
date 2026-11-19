# Correction eval: before (0.3.71)

Cells: recall (fixed right) / wrong fix, per word length; for layout+typo `(+N%)` = layout switched but the slip left. Typo 2 from 4 letters, typo 3 from 6.

## Ru: recall / wrong

| kind | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | all |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| typo 1 | 0% / 2.1% | 30% / 5.0% | 57% / 1.9% | 75% / 4.6% | 79% / 1.7% | 82% / 1.7% | 89% / 3.4% | 92% / 0.8% | 90% / 0.0% | 92% / 0.0% | 91% / 0.9% | **76% / 1.9%** (n=1185) |
| typo 2 | — | — | 9% / 13.9% | 12% / 13.3% | 7% / 8.7% | 47% / 6.9% | 61% / 4.3% | 68% / 2.6% | 75% / 5.1% | 88% / 0.8% | 82% / 2.5% | **51% / 6.3%** (n=1021) |
| typo 3 | — | — | — | — | 9% / 5.2% | 11% / 2.5% | 26% / 10.1% | 15% / 5.0% | 17% / 4.2% | 18% / 1.7% | 18% / 4.2% | **16% / 4.7%** (n=834) |
| layout | 68% / 0.0% | 93% / 0.0% | 98% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | **98% / 0.0%** (n=1221) |
| layout+typo | 0% / 2.9% | 0% (+33%) / 1.1% | 0% (+32%) / 6.5% | 0% (+43%) / 3.6% | 0% (+51%) / 0.9% | 0% (+50%) / 1.7% | 0% (+51%) / 0.0% | 0% (+50%) / 0.9% | 0% (+53%) / 0.0% | 0% (+48%) / 0.0% | 0% (+51%) / 0.0% | **0% (+45%) / 1.4%** (n=1143) |

## En: recall / wrong

| kind | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | all |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| typo 1 | 0% / 0.0% | 31% / 5.0% | 53% / 2.1% | 66% / 2.7% | 70% / 3.4% | 84% / 0.9% | 83% / 0.8% | 87% / 0.8% | 91% / 0.0% | 94% / 0.0% | 96% / 0.0% | **76% / 1.4%** (n=1147) |
| typo 2 | — | — | 9% / 20.4% | 10% / 16.5% | 5% / 11.8% | 50% / 7.8% | 64% / 3.5% | 72% / 1.7% | 83% / 3.4% | 88% / 1.7% | 88% / 0.0% | **54% / 7.0%** (n=1010) |
| typo 3 | — | — | — | — | 5% / 11.9% | 13% / 6.8% | 18% / 8.3% | 18% / 6.8% | 11% / 2.5% | 16% / 0.8% | 16% / 1.7% | **14% / 5.5%** (n=834) |
| layout | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | 100% / 0.0% | **100% / 0.0%** (n=1238) |
| layout+typo | 0% / 0.0% | 0% (+42%) / 1.5% | 0% (+53%) / 0.0% | 0% (+62%) / 1.9% | 0% (+76%) / 0.0% | 0% (+62%) / 0.0% | 0% (+75%) / 0.0% | 0% (+79%) / 0.0% | 0% (+68%) / 0.0% | 0% (+79%) / 0.0% | 0% (+72%) / 0.0% | **0% (+67%) / 0.3%** (n=1114) |

## False positives: unknown-word proxy (changed %)

| set | 2-3 | 4 | 5-6 | 7-8 | 9-12 | all |
|---|---|---|---|---|---|---|
| Ru lower | 0.0% | 22.7% | 16.1% | 28.0% | 24.6% | **24.7%** (n=3000) |
| Ru Capitalised | 0.0% | 22.7% | 10.8% | 7.8% | 2.9% | **4.7%** (n=3000) |
| En lower | 0.0% | 13.0% | 21.9% | 18.2% | 14.8% | **16.3%** (n=3000) |
| En Capitalised | 0.0% | 13.0% | 18.3% | 9.3% | 3.6% | **6.6%** (n=3000) |

## Curated FP set (names, brands, code, URLs, slang): changed 9/297 (3.0%)

["Безос→Безо", "Tatyana→Tatiana", "rustc→rustic", "const→cost", "РФ→HA", "ничо→ничто", "Bizon→Bison", "Shmel→Shel", "Димон→Дион"]

## Known frequent words changed: 0/1320

[]

## Engine::decide latency (recall cases): mean 2.22 ms, p99 19.23 ms, max 33.23 ms (пгсморишь)

## Wrong-fix examples

* En layout+typo: Сфии→Софии (cab), екнт→кент (tryin), руене→рене (hetty)
* En typo 1: sdki→ывлш (sdk), gptt→gott (gpt), heuh→ругр (heh), thuy→ергн (thy), tist→twist (tits), glenm→glenn (glen), kellyy→лудднн (kelly), scalop→scallop (scalp), Lyrid→Lurid (lyric), faser→fraser (faster), Brnady→Brady (brandy), Manial→Menial (manual)
* En typo 2: kissse→kisses (kiss), jokew→jokes (joke), Wetd→Цуев (west), wfik→цашл (wifi), thyys→ернны (thus), tjte→еоеу (tits), rjubhy→когирн (ruby), Fna→Атф (fran), gklne→плдту (glen), lafn→lan (lang), Ryys→Rhys (rays), libh→дшир (limb)
* En typo 3: strikjes→strikes (strike), effcctt→уаассее (effect), fceukr→асуглк (fucker), Bfahdy→Иафрвн (brandy), easger→eager (easter), bhnc→иртс (brunch), rekmrkk→кульклл (remark), nooee→noose (nobles), bnred→bred (bribed), paanos→pianos (panels), Seede→Swede (seeker), Stirter→Starter (sitter)
* FP unknown En Capitalised: Abroma→Aroma, Abysms→Abyss, Acons→Acorns, Adjudgment→Adjustment, Adverted→Averted, Agued→Argued, Ahaz→Ayaz, Alightment→Alignment, Aptos→Apos, Arago→Argo, Arcked→Racked, Aricas→Arias
* FP unknown En lower: abdelnasir→abdulnasir, abhorrition→abhorration, ablutions→ablations, abroma→aroma, abysms→abyss, acanthus→acanthous, accolle→accole, acharnian→acharnean, acons→acorns, acrophobias→acarophobias, adamancies→adamances, adjudgment→adjustment
* FP unknown Ru Capitalised: Аврикам→Африкам, Автоворов→Авторов, Агретине→Аргентине, Адресует→Арестует, Азоматом→Ароматом, Айраном→Райаном, Албберте→Альберте, Алды→Лады, Алисаг→Алиса, Алиу→Али, Альбитах→Альбертах, Альфоносах→Альфонсах
* FP unknown Ru lower: абазеровной→абузеровной, абдирайимами→абдираимами, абдрауфов→абдурауфов, абдрашитове→абрашитове, абдубасита→абдулбасита, абдувахита→абдуваита, абдугаффара→абдугафара, абдужелилами→абдужалилами, абдулажонам→абдулажанам, абдулкайюмов→абдулкаюмов, абдулхаера→абдулхаира, абдулхакимам→абдухакимам
* Ru layout+typo: b[[→их[ (их), yjuu→yuu (ног), ;ltnn→;lynn (ждет), vyj.,→мною, (мною), cfbs→cbs (сары), {erfy→{effy (хуан), ;fhee→;fee (жару), lpel→lapel (джуд), uhlm→uhm (грудь), lfab→lab (дафни), ierne→irene (шутку), ;ltbt→;lgbt (ждете)
* Ru typo 1: ждд→;ll (жд), ещ→to (еще), Руук→Руку (рук), даж→lf; (даю), тлли→талли (тли), Злог→Залог (зло), прис→пэрис (пирс), Кыин→Rsby (квин), взоле→воле (возле), селф→сел (селфи), Барнни→Банни (барни), Мэндди→Мэдди (мэнди)
* Ru typo 2: бву→,de (буду), полеен→полезен (поле), бюнр→бнр (бюро), ткаеи→ткани (таки), Кввинн→Квинн (квин), уьтур→emneh (утру), ваеныу→dftyse (вены), пргд→прод (пред), Блав→Бла (бала), двно→давно (дань), бкок→бок (берк), дувд→двд (джуд)
* Ru typo 3: пвермы→пермы (первым), нкшиии→yribbb (нашими), санйт→сайт (станут), грж→uh; (гараже), геам→геем (гением), покыа→пока (подача), Вщзгянт→Dopuzyn (взгляни), поьзы→пользы (полезны), хущиан→хулиан (хулиган), сттрана→страна (странная), Доверряяте→Доверяете (доверяет), поуоончи→покончи (полуночи)

## Miss examples

* En layout+typo: Цыу (we), шпа (if), фыц (as), Ирн (by), нфф (ya), нз (yo), еп (ty), нуек (yet), щщо (ooh), щшк (okr), иыса (bsc), фшз (api)
* En typo 1: hii (hi), uii (ui), tgy (ty), ywe (yw), npo (np), jok (jk), shee (she), hadd (had), mmo (mom), saaw (saw), Tpo (top), gqy (gay)
* En typo 2: Ovvdr (over), mivee (move), wiugfe (wife), Kis (kids), Feesd (feed), hoookk (hook), boknr (bond), miaq (mina), gfarm (gram), howss (howe), mionoo (mono), ystti (yeti)
* En typo 3: fiuffre (figure), priwlonn (prison), Memkort (memory), sxsveer (severe), coodton (cotton), alxeiiss (alexis), Bafetr (baxter), sytwelr (steele), truuuss (trusts), ndwetsr (newest), Tenr (tenant), stylplew (styles)
* Ru layout: [v (хм), [p (хз), h, (рб), ,ju (бог), nt, (теб), ik.[ (шлюх), ,thr (берк)
* Ru layout+typo: ;g (же), yg (ни), jqq (ой), ;d (жк), g[ (пж), nmm (ть), rfe (как), Uly (где), "ngb (эти), C]h (сэр), nhv (три), dti (ваш)
* Ru typo 1: нен (не), ог (он), Зв (за), пто (по), сш (со), ож (ох), оф (ой), таа (та), Адд (ад), рлб (рб), суу (су), нав (нас)
* Ru typo 2: туддпа (туда), лицтоо (лицо), мссьее (мсье), чтрожд (чтож), Ушл (ушло), Моюдяы (моды), Хцарн (хуан), врдия (видя), блбж (блюд), жвмя (живя), пойцтми (пойти), сттвет (совет)
* Ru typo 3: тррдугно (трудно), ситтгпнал (сигнал), пемде (поедем), пговеелл (провел), суипрть (судить), урмешлль (умрешь), меясццыы (месяцы), джаант (джанет), Ннидч (ниндзя), бгееет (бегает), зкнойй (зубной), врангьеап (вранье)
