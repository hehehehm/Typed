# Context eval: 0.3.72 n-gram context (Normal) (level Normal)

Cells: right (+ layout fixed, typo kept) / wrong (share of cases). `all` = every target, `mid` = targets after a word.

## ru: 3000 sentences, 2994 clean in both modes

| case | ctx on (all) | ctx off (all) | ctx on (mid) | ctx off (mid) | n (all/mid) |
|---|---|---|---|---|---|
| typo1 | 77.0% / 2.61% | 74.3% / 2.94% | 79.5% / 2.54% | 76.3% / 2.94% | 2683/2280 |
| typo2 | 45.7% / 9.13% | 43.3% / 10.03% | 47.6% / 9.04% | 44.8% / 10.09% | 2673/2279 |
| layout | 96.9% / 0.26% | 96.9% / 0.26% | 96.7% / 0.34% | 96.7% / 0.34% | 1910/1488 |
| layout+typo | 57.1% (+4.7%) / 5.49% | 56.1% (+5.5%) / 5.49% | 59.3% (+4.4%) / 5.60% | 58.1% (+5.4%) / 5.60% | 1675/1321 |

Unknown-word proxy in sentences (each word treated as missing from the lists; changed with / without context):
- rare words (rank ≥ 20000 or unranked, 761 words): 21.81% / 25.49%
- frequent words (5676 words, flattering: the corpus knows them): 15.70% / 29.07%

Rare words kept only thanks to context:
- `пустынях` (→ `пустоях` without context) after Some("в")
- `приему` (→ `приму` without context) after Some("по")
- `российский` (→ `росссийский` without context) after Some("москва")
- `удивлением` (→ `удалением` without context) after Some("с")
- `французски` (→ `франциски` without context) after Some("по")
- `огорчён` (→ `окончен` without context) after Some("очень")
- `плаваешь` (→ `плачешь` without context) after Some("хорошо")
- `секретным` (→ `скрытным` without context) after Some("с")
- `машет` (→ `мешает` without context) after Some("мэри")
- `приключениях` (→ `привлечениях` without context) after Some("о")
- `французски` (→ `франциски` without context) after Some("по")
- `запятой` (→ `занятой` without context) after Some("с")
- `частоте` (→ `чистоте` without context) after Some("этой")
- `французски` (→ `франциски` without context) after Some("по")
- `принятие` (→ `понятие` without context) after Some("на")

Clean sentences changed: ctx on 5 (0.17%), ctx off 6 (0.20%)

FP examples (ctx on):
- `От похвалы не возгордись, а от хулы не гневись.` → `От похвалы не возгордись, а от [eks не гневись.`
- `Сасай, лалка!` → `Спасай, лалка!`
- `Изаура — опытная женщина.` → `Из аура — опытная женщина.`
- `"Взлёт и падение Зигги Стардаста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.` → `"Взлёт и падение Зигги Староста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.`
- `Столица Танзании - Додома.` → `Столица Танзании - До дома.`

FP examples (ctx off):
- `От похвалы не возгордись, а от хулы не гневись.` → `От похвалы не возгордись, а от [eks не гневись.`
- `Если на руках ногти, то почему на ногах не рукти?` → `Если на руках ногти, то почему на ногах не руки?`
- `Сасай, лалка!` → `Спасай, лалка!`
- `Изаура — опытная женщина.` → `Из аура — опытная женщина.`
- `"Взлёт и падение Зигги Стардаста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.` → `"Взлёт и падение Зигги Староста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.`
- `Столица Танзании - Додома.` → `Столица Танзании - До дома.`

Wrong fixes with context (sample):
- typo1: `мигали`→`мирали`: `Вдали морали огоньки.`
- typo1: `приему`→`принему`: `К стоматологу только по при нему.`
- typo2: `сделал`→`сдлеаал`: `Ты сделала это!`
- typo2: `мои`→`моиа`: `Результаты не оправдали моим ожидания.`
- typo1: `Томом`→`Тоом`: `Я очень горжусь Том.`
- typo2: `дурак`→`двура`: `Он, должно быть, дура, если верит в такое.`
- typo1: `дверь`→`двер`: `Кто-то стучал в довер.`
- typo1: `Мне`→`Мнее`: `Менее не нужны твои деньги.`
- typo1: `рассердятся`→`равсердятся`: `А на вас не рассердится, что вы дома не ночевали и никого не предупредили?`
- typo1: `Мне`→`Мнее`: `Менее нужно, чтобы ты верила в меня.`
- typo2: `говорит`→`гоывори`: `Он говори на русском языке.`
- typo2: `Большие`→`Больлшеи`: `Большеи запросы усложняют существование. Счастливая жизнь начинается, когда от них отказываешься.`
- typo1: `пополнять`→`поплолнять`: `Каждый может пополнить базу данных, чтобы приводить примеры новой лексики.`
- layout+typo: `пополнять`→`gjgjkyzn,`: `Каждый может пополнят, базу данных, чтобы приводить примеры новой лексики.`
- typo2: `дома`→`доаам`: `Сегодня мы будем домам.`
- typo2: `думал`→`дамл`: `Он дал, что я глуп.`
- typo2: `более`→`болле`: `Не объяснишь ли ты болел подробно?`
- layout+typo: `Будь`→`<delm`: `<dem серьёзна!`
- typo2: `Свет`→`Свкер`: `Сквер сам погас.`
- typo2: `всё`→`всс`: `Я вас ещё не верю в это.`
- typo1: `Просто`→`Простоо`: `Все в порядке. Простого расслабься. Будь собой.`
- typo1: `воду`→`водуу`: `Том столкнул Мэри в водку.`
- typo2: `рано`→`рпено`: `Встань завтра утром рено.`
- typo2: `Войну`→`Вофу`: `Я тоже никогда не читал "Воу и мир".`
- typo2: `Том`→`Тьим`: `Тим отдал Мэри свою почку.`
- layout+typo: `священнее`→`cdzotyyttt`: `Для индейцев звери были священнике людей.`
- typo2: `сошьёшь`→`соьшёш`: `Из спасиба шубу не сошел.`
- layout+typo: `Том`→`Njbv`: `Твоим делает всё, что мы ни захотим.`
- typo2: `дома`→`дорома`: `Мы поговорили с хозяином до рома.`
- typo1: `день`→`деньь`: `Принимайте это лекарство два раза в lymm/`
- layout+typo: `играла`→`buhff`: `Вчера я не buff в теннис.`
- layout+typo: `часа`→`xfcff`: `Ехать полтора часам на работу — тягомотина.`
- typo1: `поэтом`→`пожтом`: `Я хотел бы стать потом.`
- typo2: `если`→`елми`: `А ели что-нибудь сломается?`
- typo2: `бизона`→`бизоон`: `Почему ты ищешь белого бизон?`
- typo1: `шлюпками`→`шлжюпками`: `Большие суда все оснащены спасательными шляпками.`
- typo2: `том`→`тоип`: `Вы уверены, что мы говорим об одном и тип же человеке?`
- typo2: `прежде`→`преед`: `Подумай перед тем, как отдать. Подумай дважды прежде, чем взять, и подумай тысячу раз перед, чем требовать.`
- typo2: `Мне`→`Меен`: `Мен надо поторапливаться.`
- layout+typo: `такой`→`nfdjq`: `Это твой тонкий намёк на толстые обстоятельства.`
- typo2: `уснул`→`усно`: `То-о-ом! Ты что, умно? Я к тебе обращаюсь.`
- layout+typo: `Когда`→`Rujlf`: `Кода я пришёл к ней, её не было дома.`
- typo2: `задрожал`→`зсдржал`: `Наш дом задержал.`
- typo2: `Берлина`→`Беерлин`: `Я житель Берлин.`
- typo2: `самом`→`сммо`: `Каждая серия заканчивается на само интересном месте.`
- typo1: `пешочком`→`пешочкошм`: `Спасибо, я пешком.`
- typo2: `что`→`чьио`: `Я знаю, чьи Тома и Мэри арестовали.`
- typo2: `была`→`бычл`: `Я был дурой, что поверила ему.`
- typo2: `Мне`→`Мпме`: `Маме не идёт чёлка.`
- typo2: `контакта`→`корнтакте`: `Робкие люди избегают зрительного контакте и легко краснеют.`
- typo1: `ещё`→`ещёё`: `Мама to`` не вернулась?`
- typo2: `будет`→`буетт`: `Его жизнь бетт другой.`
- typo2: `будет`→`будтее`: `Не думаю, что Тому будете холодно.`
- typo2: `меня`→`меанч`: `Ты на манч ещё злишься?`
- typo2: `Кто`→`Ктт`: `@Rnn вы по национальности?" - "Аргентинцы".`
- layout+typo: `здесь`→`pltc`: `Они всегда играли plc в теннис.`
- typo2: `виноват`→`винове`: `Почему у тебя чуть что, так сразу я виновен?`
- typo1: `Джон`→`Ддон`: `Дон уверен в своих способностях.`
- typo2: `чтобы`→`чттоб`: `Скажите Мэри, чтоб она не делала этого одна.`
- typo2: `Врач`→`Врамя`: `Время сказал, что его болезнь неизлечима.`

Cases where context changed the outcome (✓ right, ✗ wrong, · unchanged):
- typo1: `говорил`→`говориил` in `Джон говорил так громко, что я слышал его на втором этаже.`: ctx · `Джон говориил так громко, что я слышал его на втором этаже.`, no ctx ✓ `Джон говорил так громко, что я слышал его на втором этаже.`
- typo2: `студент`→`студт` in `Кажется, он студент.`: ctx ✓ `Кажется, он студент.`, no ctx · `Кажется, он студт.`
- typo1: `поднялись`→`подрялись` in `Мы поднялись на борт в десять.`: ctx ✓ `Мы поднялись на борт в десять.`, no ctx · `Мы подрялись на борт в десять.`
- layout+typo: `знал`→`pyffk` in `Я ещё никогда не бывал в этом городе, и, что ещё более важно, я не знал ни слова на том языке, на котором там говорили.`: ctx ✓ `Я ещё никогда не бывал в этом городе, и, что ещё более важно, я не знал ни слова на том языке, на котором там говорили.`, no ctx ✗ `Я ещё никогда не бывал в этом городе, и, что ещё более важно, я не знаал ни слова на том языке, на котором там говорили.`
- typo1: `своё`→`свёо` in `Он сделал своё замечание очень тактично.`: ctx ✗ `Он сделал свое замечание очень тактично.`, no ctx · `Он сделал свёо замечание очень тактично.`
- typo2: `такой`→`таккй` in `Извините за такой ранний звонок.`: ctx ✓ `Извините за такой ранний звонок.`, no ctx · `Извините за таккй ранний звонок.`
- typo2: `диете`→`дмтее` in `Моя мать на диете.`: ctx ✓ `Моя мать на диете.`, no ctx · `Моя мать на дмтее.`
- typo2: `врач`→`верч` in `Я не врач.`: ctx · `Я не верч.`, no ctx ✗ `Я не веры.`
- typo1: `хотел`→`хотл` in `Я бы хотел ещё масла.`: ctx ✓ `Я бы хотел ещё масла.`, no ctx · `Я бы хотл ещё масла.`
- typo2: `моём`→`моёммм` in `На моём компьютере установлен Линукс.`: ctx ✗ `На моем компьютере установлен Линукс.`, no ctx · `На моёммм компьютере установлен Линукс.`
- typo1: `улыбнулся`→`улыбулся` in `Младенец увидел меня и улыбнулся.`: ctx ✓ `Младенец увидел меня и улыбнулся.`, no ctx · `Младенец увидел меня и улыбулся.`
- typo1: `нам`→`наа` in `Я думаю, нам следует нанять Тома.`: ctx ✓ `Я думаю, нам следует нанять Тома.`, no ctx · `Я думаю, наа следует нанять Тома.`
- typo2: `должна`→`должжн` in `Она должна туда пойти.`: ctx ✓ `Она должна туда пойти.`, no ctx · `Она должжн туда пойти.`
- typo1: `дешёвке`→`дешвке` in `Эта книга была куплена по дешёвке.`: ctx · `Эта книга была куплена по дешвке.`, no ctx ✗ `Эта книга была куплена по ltidrt/`
- typo2: `каждый`→`каждц` in `Я учу французский каждый день.`: ctx ✓ `Я учу французский каждый день.`, no ctx · `Я учу французский каждц день.`
- layout+typo: `дом`→`ljvv` in `Вы не зайдёте в дом в таком виде.`: ctx ✓ `Вы не зайдёте в дом в таком виде.`, no ctx ✗ `Вы не зайдёте в домм в таком виде.`
- layout+typo: `могли`→`vjukv` in `Будьте добры, Вы не могли бы перезвонить мне завтра?`: ctx ✓ `Будьте добры, Вы не могли бы перезвонить мне завтра?`, no ctx ✗ `Будьте добры, Вы не моглм бы перезвонить мне завтра?`
- layout+typo: `показать`→`gjrfpfm` in `Том хотел показать Мэри фотографию Джона.`: ctx ✓ `Том хотел показать Мэри фотографию Джона.`, no ctx · `Том хотел gjrfpfm Мэри фотографию Джона.`
- typo2: `день`→`днеьо` in `Дождь шёл целый день.`: ctx ✓ `Дождь шёл целый день.`, no ctx · `Дождь шёл целый днеьо.`
- typo1: `сказал`→`сказаал` in `Я бы сказал, что вы ревнуете.`: ctx ✓ `Я бы сказал, что вы ревнуете.`, no ctx · `Я бы сказаал, что вы ревнуете.`
- typo2: `кажешься`→`каеься` in `Сегодня ты кажешься рассеянным.`: ctx · `Сегодня ты каеься рассеянным.`, no ctx ✗ `Сегодня ты кажется рассеянным.`
- typo1: `носила`→`нсила` in `Она носила симпатичную шляпку.`: ctx ✓ `Она носила симпатичную шляпку.`, no ctx · `Она нсила симпатичную шляпку.`
- typo2: `сказали`→`сааали` in `Вы мне так и не сказали, зачем вы здесь.`: ctx ✓ `Вы мне так и не сказали, зачем вы здесь.`, no ctx · `Вы мне так и не сааали, зачем вы здесь.`
- typo1: `мной`→`мпой` in `Намедни странная история со мной приключилась.`: ctx ✓ `Намедни странная история со мной приключилась.`, no ctx · `Намедни странная история со мпой приключилась.`
- typo2: `ещё`→`езё` in `Почему в дверях стоят кровавый и розовый стейки? Потому что они оба ещё не готовы.`: ctx · `Почему в дверях стоят кровавый и розовый стейки? Потому что они оба езё не готовы.`, no ctx ✗ `Почему в дверях стоят кровавый и розовый стейки? Потому что они оба еще не готовы.`
- typo2: `говорят`→`гооврт` in `Если никто не знает, что должно произойти, то все говорят: что-то должно произойти.`: ctx ✓ `Если никто не знает, что должно произойти, то все говорят: что-то должно произойти.`, no ctx · `Если никто не знает, что должно произойти, то все гооврт: что-то должно произойти.`
- typo2: `богат`→`боьгато` in `Том говорит, что он богат.`: ctx ✓ `Том говорит, что он богат.`, no ctx · `Том говорит, что он боьгато.`
- typo2: `подозревал`→`подоьзрева` in `Я и не подозревал, что Том такой старый.`: ctx ✓ `Я и не подозревал, что Том такой старый.`, no ctx · `Я и не подоьзрева, что Том такой старый.`
- typo2: `костей`→`ктстй` in `Я продрогла до костей.`: ctx ✓ `Я продрогла до костей.`, no ctx · `Я продрогла до ктстй.`
- typo2: `тоже`→`теж` in `Богатые тоже плачут.`: ctx · `Богатые теж плачут.`, no ctx ✗ `Богатые тех плачут.`
- typo1: `более`→`боьлее` in `Не объяснишь ли ты более подробно?`: ctx ✗ `Не объяснишь ли ты болен подробно?`, no ctx ✓ `Не объяснишь ли ты более подробно?`
- layout+typo: `более`→`,ktt` in `Не объяснишь ли ты более подробно?`: ctx · `Не объяснишь ли ты ,ktt подробно?`, no ctx ✓ `Не объяснишь ли ты более подробно?`
- layout+typo: `стране`→`cnhft` in `Многие молодые люди в этой стране не имеют работы.`: ctx ✓ `Многие молодые люди в этой стране не имеют работы.`, no ctx ✗ `Многие молодые люди в этой страе не имеют работы.`
- typo2: `покинул`→`пкоинл` in `Том их покинул.`: ctx · `Том их пкоинл.`, no ctx ✓ `Том их покинул.`
- typo2: `бассейн`→`бссейнп` in `Я не знал, что здесь есть плавательный бассейн.`: ctx ✓ `Я не знал, что здесь есть плавательный бассейн.`, no ctx · `Я не знал, что здесь есть плавательный бссейнп.`
- typo1: `понять`→`поняь` in `Я не мог понять, о чём он говорит.`: ctx ✓ `Я не мог понять, о чём он говорит.`, no ctx · `Я не мог поняь, о чём он говорит.`
- typo2: `раза`→`рза` in `Раньше горячую воду отключали два раза за лето, а нынче - только один.`: ctx ✓ `Раньше горячую воду отключали два раза за лето, а нынче - только один.`, no ctx · `Раньше горячую воду отключали два рза за лето, а нынче - только один.`
- typo2: `знает`→`знаееьт` in `Я знаю что-то, чего не знает Том.`: ctx ✓ `Я знаю что-то, чего не знает Том.`, no ctx · `Я знаю что-то, чего не знаееьт Том.`
- typo1: `делает`→`длает` in `Что делает этот полицейский?`: ctx ✓ `Что делает этот полицейский?`, no ctx · `Что длает этот полицейский?`
- typo1: `мире`→`мирн` in `Гора Эверест является высочайшей вершиной в мире.`: ctx ✓ `Гора Эверест является высочайшей вершиной в мире.`, no ctx · `Гора Эверест является высочайшей вершиной в мирн.`

## en: 3000 sentences, 2995 clean in both modes

| case | ctx on (all) | ctx off (all) | ctx on (mid) | ctx off (mid) | n (all/mid) |
|---|---|---|---|---|---|
| typo1 | 73.5% / 3.20% | 71.3% / 3.80% | 75.8% / 3.32% | 73.4% / 3.99% | 2343/2107 |
| typo2 | 39.6% / 10.88% | 37.4% / 13.59% | 42.0% / 10.94% | 39.6% / 13.94% | 2398/2166 |
| layout | 99.6% / 0.15% | 99.6% / 0.15% | 99.5% / 0.17% | 99.5% / 0.17% | 2068/1787 |
| layout+typo | 49.5% (+11.9%) / 8.18% | 48.4% (+12.6%) / 8.30% | 50.9% (+11.3%) / 8.49% | 49.6% (+12.1%) / 8.62% | 1650/1461 |

Unknown-word proxy in sentences (each word treated as missing from the lists; changed with / without context):
- rare words (rank ≥ 20000 or unranked, 135 words): 22.96% / 25.19%
- frequent words (8564 words, flattering: the corpus knows them): 13.12% / 28.29%

Rare words kept only thanks to context:
- `snows` (→ `shows` without context) after Some("it")
- `parolee` (→ `parole` without context) after Some("the")
- `displeased` (→ `diseased` without context) after Some("was")

Clean sentences changed: ctx on 5 (0.17%), ctx off 5 (0.17%)

FP examples (ctx on):
- `Fadil and Layla hitched a ride to the motel.` → `Fail and Layla hitched a ride to the motel.`
- `I'll help you find Fadil.` → `I'll help you find Fail.`
- `Fadil tried to focus on his family.` → `Fail tried to focus on his family.`
- `Go help Fadil.` → `Go help Fail.`
- `Firmus is a Berber king from Kabylia.` → `Firms is a Berber king from Kabylia.`

FP examples (ctx off):
- `Fadil and Layla hitched a ride to the motel.` → `Fail and Layla hitched a ride to the motel.`
- `I'll help you find Fadil.` → `I'll help you find Fail.`
- `Fadil tried to focus on his family.` → `Fail tried to focus on his family.`
- `Go help Fadil.` → `Go help Fail.`
- `Firmus is a Berber king from Kabylia.` → `Firms is a Berber king from Kabylia.`

Wrong fixes with context (sample):
- typo2: `back`→`bdk`: `I have to go shopping. I'll be ивл in an hour.`
- typo2: `what`→`whart`: `Not knowing wart to say, she remained silent.`
- typo2: `was`→`waaa`: `There are good grounds for the view that Paris waa the scene of frequent riots even before the revolution of 1789.`
- layout+typo: `fine`→`ашьу`: `It was such a time day that we decided to have a picnic.`
- typo1: `leave`→`leavee`: `The boat is to levee for England this evening.`
- layout+typo: `heavy`→`руфм`: `He put up a brave and lone struggle, but up against such hav odds he couldn't get his business plan accepted.`
- typo2: `ran`→`raam`: `The car ram into a tree.`
- layout+typo: `the`→`епу`: `Hold on to ему rail.`
- layout+typo: `have`→`рфмму`: `I рифму just remembered his name, which I couldn't for a long time.`
- layout+typo: `got`→`пщще`: `He groot a position as a clerk.`
- typo2: `her`→`heee`: `She flung here clothes on.`
- layout+typo: `her`→`рвук`: `She flung рук clothes on.`
- typo2: `very`→`vfry`: `I'm макн glad to see you again.`
- layout+typo: `felt`→`адуе`: `They no longer аду a need to protest.`
- typo2: `coffee`→`coffver`: `You love cover.`
- typo2: `Tom`→`Tmoo`: `Too isn't at home.`
- typo1: `drove`→`droive`: `Tom drive his car off a bridge.`
- typo1: `guaranties`→`guarantiea`: `Now that I got guarantees, I am no longer afraid.`
- layout+typo: `were`→`цкуку`: `He just gave us a map and sprinted ahead. We who цуку left behind had to go on with just the map to rely on.`
- typo2: `wood`→`wodok`: `Their job is to chop the wook.`
- typo2: `want`→`wafn`: `I didn't warn to come here at all.`
- typo2: `your`→`yiou`: `You want me to do you dirty work for you, don't you?`
- layout+typo: `not`→`тщеа`: `Hey guys! That's теа funny!`
- typo1: `Bring`→`Briung`: `Brung Tom in.`
- typo2: `sitting`→`sitgin`: `I'm sittin here.`
- typo2: `invite`→`innvitse`: `Who'd you invites?`
- layout+typo: `right`→`кшре`: `I have a коре to defend myself.`
- layout: `come`→`сщьу`: `The father had an outburst and screamed in the telephone, @come to the house immediately! Immediately! Immediately! Immediately!"`
- layout+typo: `married`→`ьфкшкув`: `I should never have marred Mary.`
- layout+typo: `never`→`тмук`: `She тук listens to the teacher.`
- typo1: `You`→`Youi`: `Yui added nothing.`
- typo2: `the`→`trh`: `She saved try puppy.`
- typo2: `camera`→`cammeraa`: `She installed a security cameras.`
- typo2: `This`→`Tijhs`: `Ешоры is so romantic.`
- typo1: `door`→`doior`: `They were shown the dior.`
- layout+typo: `day`→`вфын`: `It's a beautiful easy here in Boston.`
- typo2: `talked`→`tlake`: `Has he take to you?`
- typo1: `fault`→`faultt`: `Whose faulty was it?`
- layout+typo: `the`→`епру`: `Tom ran back into перу cave.`
- typo2: `racking`→`racning`: `I'm racing my brain.`
- typo2: `makes`→`mkess`: `We have to support whatever decision Tom mess.`
- layout+typo: `plan`→`ззфт`: `Tom didn't pan to go to Boston.`
- typo2: `You`→`Yj`: `Но really hurt us.`
- layout+typo: `These`→`Ерыу`: `The days Mary is never sure to whom grammar questions should be addressed to ensure an authoritative answer.`
- typo2: `There`→`Thereee`: `Therese are a couple of options.`
- typo2: `wrong`→`wonv`: `Boy was I wong.`
- typo2: `Tom`→`Tkon`: `Ton made a film about his childhood.`
- layout+typo: `the`→`ерву`: `I don't know how Paris is the most visited city in еву world.`
- typo2: `can`→`cxsn`: `Where сыт I get one of those?`
- typo2: `think`→`tinbk`: `I tink it's a shame.`
- typo2: `will`→`wiool`: `Who wool swim the fastest?`
- typo1: `got`→`goot`: `Tom almost groot hit by a car.`
- typo2: `phone`→`phooned`: `What did you do to my phoned?`
- typo1: `poor`→`ppor`: `That por slob began and ended his career as a district policeman.`
- typo2: `transfusions`→`tramnsfusion`: `Tom's treatment may require transfusion.`
- layout+typo: `long`→`дщдтп`: `How дтп is it going to be before the heat comes back on?`
- typo2: `you`→`youyh`: `How can youth be so calm about everything?`
- typo2: `put`→`phutt`: `Tom putt the cake on a platter.`
- layout+typo: `left`→`дуаме`: `Tom was даме alone in the room.`
- typo2: `sensors`→`sennsor`: `My sensor detect an intruder in the area.`

Cases where context changed the outcome (✓ right, ✗ wrong, · unchanged):
- typo2: `member`→`memhert` in `Each member was called upon.`: ctx ✓ `Each member was called upon.`, no ctx · `Each memhert was called upon.`
- typo2: `left`→`lft` in `We left a margin for error in our estimates.`: ctx ✓ `We left a margin for error in our estimates.`, no ctx · `We lft a margin for error in our estimates.`
- typo1: `follows`→`folloes` in `The reasons for our failure are as follows.`: ctx ✓ `The reasons for our failure are as follows.`, no ctx · `The reasons for our failure are as folloes.`
- typo2: `follows`→`folloow` in `The reasons for our failure are as follows.`: ctx · `The reasons for our failure are as folloow.`, no ctx ✗ `The reasons for our failure are as follow.`
- typo2: `going`→`gkin` in `Instead of going myself, I sent a messenger.`: ctx · `Instead of gkin myself, I sent a messenger.`, no ctx ✗ `Instead of плшт myself, I sent a messenger.`
- typo1: `horse`→`horase` in `Mark was a work horse on his construction crew.`: ctx · `Mark was a work horase on his construction crew.`, no ctx ✓ `Mark was a work horse on his construction crew.`
- layout+typo: `been`→`иуцт` in `I've been wandering around.`: ctx ✓ `I've been wandering around.`, no ctx ✗ `I've bewn wandering around.`
- typo2: `times`→`tomds` in `The pyramids were built in ancient times.`: ctx ✓ `The pyramids were built in ancient times.`, no ctx · `The pyramids were built in ancient tomds.`
- typo1: `price`→`prie` in `We got a good price for that.`: ctx ✓ `We got a good price for that.`, no ctx · `We got a good prie for that.`
- typo2: `price`→`proe` in `We got a good price for that.`: ctx · `We got a good proe for that.`, no ctx ✗ `We got a good prove for that.`
- typo1: `many`→`mnay` in `The old man is wise and knows many things about life.`: ctx ✓ `The old man is wise and knows many things about life.`, no ctx · `The old man is wise and knows mnay things about life.`
- typo2: `leave`→`leazves` in `The boat is to leave for England this evening.`: ctx ✓ `The boat is to leave for England this evening.`, no ctx ✗ `The boat is to leaves for England this evening.`
- typo2: `lift`→`lifttg` in `This desk was too heavy for Patty to lift.`: ctx · `This desk was too heavy for Patty to lifttg.`, no ctx ✗ `This desk was too heavy for Patty to дшаеепю`
- typo1: `kinds`→`kindsx` in `There were various kinds of sweets.`: ctx · `There were various kindsx of sweets.`, no ctx ✗ `There were various лштвыч of sweets.`
- typo2: `kinds`→`kiindss` in `There were various kinds of sweets.`: ctx ✓ `There were various kinds of sweets.`, no ctx · `There were various kiindss of sweets.`
- typo2: `sea`→`seae` in `Do you travel by sea or by air?`: ctx · `Do you travel by seae or by air?`, no ctx ✗ `Do you travel by see or by air?`
- typo1: `the`→`thre` in `Where is the police station?`: ctx ✓ `Where is the police station?`, no ctx · `Where is thre police station?`
- typo1: `for`→`flr` in `If you try for too much, you'll end up with nothing at all.`: ctx · `If you try flr too much, you'll end up with nothing at all.`, no ctx ✓ `If you try for too much, you'll end up with nothing at all.`
- typo2: `came`→`cwamer` in `We waited at the scene of the accident till the police came.`: ctx ✓ `We waited at the scene of the accident till the police came.`, no ctx · `We waited at the scene of the accident till the police cwamer.`
- typo1: `new`→`nea` in `May we draw your attention to our new products?`: ctx ✓ `May we draw your attention to our new products?`, no ctx · `May we draw your attention to our nea products?`
- typo2: `introduced`→`intropducd` in `I know that he will be introduced to a lady by his uncle today.`: ctx ✓ `I know that he will be introduced to a lady by his uncle today.`, no ctx · `I know that he will be intropducd to a lady by his uncle today.`
- typo1: `doll`→`dokl` in `He presented her with a doll.`: ctx ✓ `He presented her with a doll.`, no ctx · `He presented her with a dokl.`
- typo1: `seven`→`seevn` in `I have my supper at a quarter past seven.`: ctx ✓ `I have my supper at a quarter past seven.`, no ctx · `I have my supper at a quarter past seevn.`
- typo1: `but`→`byt` in `Excuse me, but aren't you Dr. White?`: ctx ✓ `Excuse me, but aren't you Dr. White?`, no ctx · `Excuse me, byt aren't you Dr. White?`
- typo1: `now`→`noow` in `The exact time is three now.`: ctx · `The exact time is three noow.`, no ctx ✓ `The exact time is three now.`
- typo2: `now`→`nokkw` in `The exact time is three now.`: ctx · `The exact time is three nokkw.`, no ctx ✓ `The exact time is three now.`
- typo1: `stood`→`stoo` in `She stood up when her teacher came in.`: ctx · `She stoo up when her teacher came in.`, no ctx ✗ `She stop up when her teacher came in.`
- typo2: `stood`→`stookc` in `She stood up when her teacher came in.`: ctx ✓ `She stood up when her teacher came in.`, no ctx · `She stookc up when her teacher came in.`
- typo2: `loves`→`lovrees` in `Who loves war?`: ctx ✓ `Who loves war?`, no ctx · `Who lovrees war?`
- typo2: `made`→`mzd` in `Careful as he was, he made an unexpected mistake.`: ctx · `Careful as he was, he mzd an unexpected mistake.`, no ctx ✗ `Careful as he was, he mad an unexpected mistake.`
- typo2: `goes`→`gess` in `It goes without saying that he's the best person for the job.`: ctx · `It gess without saying that he's the best person for the job.`, no ctx ✗ `It guess without saying that he's the best person for the job.`
- typo2: `him`→`hik` in `I caught him by the arm before he could fall.`: ctx ✓ `I caught him by the arm before he could fall.`, no ctx · `I caught hik by the arm before he could fall.`
- typo1: `him`→`hkm` in `Give him plenty of work to do.`: ctx ✓ `Give him plenty of work to do.`, no ctx · `Give hkm plenty of work to do.`
- typo1: `his`→`hiz` in `Let him go his own way.`: ctx · `Let him go hiz own way.`, no ctx ✓ `Let him go his own way.`
- typo2: `his`→`hjis` in `Let him go his own way.`: ctx · `Let him go hjis own way.`, no ctx ✓ `Let him go his own way.`
- layout+typo: `his`→`ршщы` in `Let him go his own way.`: ctx ✗ `Let him go hios own way.`, no ctx ✓ `Let him go his own way.`
- typo2: `lost`→`lkkst` in `He lost his temper and began calling me names.`: ctx ✓ `He lost his temper and began calling me names.`, no ctx ✗ `He дллые his temper and began calling me names.`
- typo1: `best`→`brst` in `He is by far the best student.`: ctx ✓ `He is by far the best student.`, no ctx ✗ `He is by far the burst student.`
- typo2: `player`→`plagwr` in `He is as good as any player on our team.`: ctx ✓ `He is as good as any player on our team.`, no ctx · `He is as good as any plagwr on our team.`
- layout+typo: `player`→`здфнуа` in `He is as good as any player on our team.`: ctx ✓ `He is as good as any player on our team.`, no ctx ✗ `He is as good as any playef on our team.`

