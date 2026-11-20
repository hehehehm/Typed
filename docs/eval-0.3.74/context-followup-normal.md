# Context eval: 0.3.74 follow-up (level Normal)

Cells: right (+ layout fixed, typo kept) / wrong (share of cases). `all` = every target, `mid` = targets after a word.

## ru: 3000 sentences, 2995 clean in both modes

| case | ctx on (all) | ctx off (all) | ctx on (mid) | ctx off (mid) | n (all/mid) |
|---|---|---|---|---|---|
| typo1 | 75.9% / 2.92% | 73.2% / 3.07% | 78.9% / 2.89% | 75.7% / 3.06% | 2702/2317 |
| typo2 | 45.5% / 9.55% | 43.2% / 10.37% | 48.1% / 9.49% | 45.4% / 10.45% | 2680/2287 |
| layout | 98.1% / 0.26% | 98.1% / 0.26% | 98.1% / 0.19% | 98.1% / 0.19% | 1952/1542 |
| layout+typo | 57.0% (+5.2%) / 5.57% | 55.5% (+5.9%) / 5.57% | 59.9% (+4.7%) / 5.39% | 58.0% (+5.6%) / 5.39% | 1725/1372 |

Unknown-word proxy in sentences (each word treated as missing from the lists; changed with / without context):
- rare words (rank ≥ 20000 or unranked, 773 words): 23.42% / 27.30%
- frequent words (5793 words, flattering: the corpus knows them): 16.31% / 30.26%

Rare words kept only thanks to context:
- `пустынях` (→ `пустоях` without context) after Some("в")
- `российский` (→ `росссийский` without context) after Some("москва")
- `удивлением` (→ `удалением` without context) after Some("с")
- `литра` (→ `лира` without context) after Some("три")
- `французски` (→ `франциски` without context) after Some("по")
- `огорчён` (→ `окончен` without context) after Some("очень")
- `плаваешь` (→ `плачешь` without context) after Some("хорошо")
- `сложил` (→ `служил` without context) after Some("я")
- `столкнул` (→ `стукнул` without context) after Some("том")
- `машет` (→ `мешает` without context) after Some("мэри")
- `Томе` (→ `Том` without context) after Some("о")
- `французски` (→ `франциски` without context) after Some("по")
- `подождал` (→ `побеждал` without context) after Some("немного")
- `спрятались` (→ `спряталась` without context) after Some("они")
- `тихом` (→ `тихо` without context) after Some("в")

Clean sentences changed: ctx on 4 (0.13%), ctx off 5 (0.17%)

FP examples (ctx on):
- `Сасай, лалка!` → `Спасай, лалка!`
- `Изаура — опытная женщина.` → `Из аура — опытная женщина.`
- `"Взлёт и падение Зигги Стардаста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.` → `"Взлёт и падение Зигги Староста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.`
- `Столица Танзании - Додома.` → `Столица Танзании - До дома.`

FP examples (ctx off):
- `Если на руках ногти, то почему на ногах не рукти?` → `Если на руках ногти, то почему на ногах не руки?`
- `Сасай, лалка!` → `Спасай, лалка!`
- `Изаура — опытная женщина.` → `Из аура — опытная женщина.`
- `"Взлёт и падение Зигги Стардаста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.` → `"Взлёт и падение Зигги Староста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.`
- `Столица Танзании - Додома.` → `Столица Танзании - До дома.`

Wrong fixes with context (sample):
- layout+typo: `может`→`v;jtn`: `Странно, что у него моет быть такая вещь.`
- typo2: `сделал`→`седлла`: `Ты седла это!`
- layout+typo: `года`→`ujnlf`: `Я уверен, что встречал ее два ulf назад.`
- layout+typo: `вещь`→`dtoom`: `Эта doom твоя?`
- layout: `чемоданах`→`xtvjlfyf[`: `И давно ты так на чемодана[ живёшь?`
- typo2: `верит`→`верт`: `Он, должно быть, дурак, если черт в такое.`
- typo2: `моё`→`моёо`: `Мне нравится, как вы произносите моего имя.`
- typo1: `умел`→`упел`: `Я забыла, что Том успел читать по-французски.`
- typo2: `целый`→`цеьы`: `Дождь шёл целы день.`
- layout+typo: `Едва`→`Tldt`: `Tldr он закончил завтракать, как зазвонил телефон.`
- layout+typo: `Тому`→`Njev`: `Nev это понравилось.`
- layout+typo: `Поедем`→`Gjtlt`: `Победе со мной в Канаду.`
- typo2: `тоже`→`теож`: `Богатые тео плачут.`
- typo1: `жирна`→`жирн`: `Она стара, уродлива и жир.`
- layout+typo: `случаются`→`ckexfn.cz`: `Такие вещи случаюся только в Швеции.`
- typo1: `Подумайте`→`Подумаайте`: `Подумаете о вашем брате.`
- typo2: `обижена`→`обижаен`: `Она была чем-то обижен.`
- typo1: `Том`→`Тоим`: `Получив "Оскар", Твоим поблагодарил свою замечательную жену Мэри, а также своих детей и родителей.`
- typo2: `Тома`→`Тосм`: `Я не хотел оскорблять Том.`
- typo2: `клумба`→`клубиа`: `Посреди сада была клуба.`
- typo1: `Пойди`→`Пйоди`: `Поди скажи ей, чтобы на Вадима не готовила, его не будет — аврал.`
- typo1: `облачное`→`обалчное`: `Небо сегодня обычное.`
- layout+typo: `Таково`→`Nfrjj`: `Такого назначение этой вещи.`
- typo1: `выберет`→`выбереет`: `Какую он выбирает?`
- layout+typo: `Как`→`Rfrr`: `Rrr ты думаешь, кто Тому сказал?`
- layout+typo: `старше`→`cfhit`: `Он chit меня.`
- layout+typo: `вил`→`dbbk`: `Черенок винил был сломан.`
- typo2: `работу`→`раоты`: `Ехать полтора часа на работы — тягомотина.`
- typo2: `могу`→`моукг`: `Я не мог забыть их.`
- typo2: `Том`→`Тоомм`: `Томом счастлив?`
- typo1: `стесняется`→`стесняеться`: `Он немного стесняться.`
- typo2: `всё`→`всс`: `Он всегда вас забывает.`
- typo1: `поляк`→`полчк`: `Я получу.`
- typo2: `помочь`→`поачь`: `Том согласился плачь Мэри найти работу.`
- typo2: `умеет`→`уметт`: `Твоя жена уметь петь?`
- typo2: `подарок`→`поадрк`: `Я купила эту книгу в подарк Тому.`
- typo2: `Томом`→`Тммом`: `Мы с Тимом друганы.`
- typo2: `Мэри`→`Мхрии`: `Том знает, что Мэрии нет дома.`
- typo1: `качается`→`качаается`: `Том касается на качелях.`
- typo2: `Том`→`Тооми`: `Томи отказался принять от Мэри подарок.`
- typo2: `нас`→`наав`: `У начав есть вакцина.`
- layout+typo: `моюсь`→`vj.dm`: `Я молюсь каждый день.`
- typo2: `есть`→`есст`: `Не думаю, что у меня такой ест.`
- typo2: `тебе`→`тебб`: `Том вообще теб ответил?`
- layout+typo: `отказалась`→`jnrfpffkcm`: `Я отказалсь отвечать.`
- typo2: `Как`→`Кекк`: `Кек мне за неделю похудеть на три килограмма?`
- typo2: `выключила`→`включлиа`: `Я включила лампу на столе.`
- typo1: `ещё`→`ещ`: `Мэри to студентка?`
- typo2: `Ученик`→`Учеинка`: `Ученика ничему не учится.`
- typo2: `Такой`→`Таок`: `Так подойдёт?`
- typo1: `Том`→`Тлом`: `Телом нечасто улыбался.`
- typo2: `меня`→`мееч`: `У меч были свои дела.`
- typo2: `нет`→`негот`: `Том говорит, что у него него проблем с засыпанием.`
- typo2: `роман`→`рмоа`: `Это просто рома.`
- layout+typo: `если`→`tfkb`: `Что бы Том стал делать, ехали бы узнал?`
- typo1: `планы`→`планыу`: `У неё есть плану.`
- layout+typo: `ему`→`tge`: `Том говорит, что the надоело быть тем, кому всегда приходится принимать решения.`
- typo1: `Ужасно`→`Ужасноо`: `Ужасного выглядишь, Том.`
- typo2: `что`→`чрт`: `Я ответила, черт не знаю.`
- typo1: `Мне`→`Мнее`: `Менее надо идти, но я не пойду.`

Cases where context changed the outcome (✓ right, ✗ wrong, · unchanged):
- layout+typo: `весьма`→`dtcmf` in `Это весьма перспективный чешский писатель.`: ctx ✓ `Это весьма перспективный чешский писатель.`, no ctx · `Это dtcmf перспективный чешский писатель.`
- layout+typo: `знал`→`pyfm` in `Я ещё никогда не бывал в этом городе, и, что ещё более важно, я не знал ни слова на том языке, на котором там говорили.`: ctx ✓ `Я ещё никогда не бывал в этом городе, и, что ещё более важно, я не знал ни слова на том языке, на котором там говорили.`, no ctx · `Я ещё никогда не бывал в этом городе, и, что ещё более важно, я не pyfm ни слова на том языке, на котором там говорили.`
- typo2: `книга`→`кртга` in `Любая интересная книга подойдёт.`: ctx ✓ `Любая интересная книга подойдёт.`, no ctx ✗ `Любая интересная rhnuf подойдёт.`
- typo2: `траве`→`травае` in `По траве не ходить.`: ctx ✓ `По траве не ходить.`, no ctx · `По травае не ходить.`
- layout+typo: `мной`→`vjyq` in `Ты сидишь между мной и Мег.`: ctx ✓ `Ты сидишь между мной и Мег.`, no ctx · `Ты сидишь между vjyq и Мег.`
- typo2: `сделал`→`седлла` in `Ты сделал это!`: ctx ✗ `Ты седла это!`, no ctx · `Ты седлла это!`
- typo1: `неё`→`ннё` in `У неё сейчас очень плохое настроение.`: ctx ✗ `У нее сейчас очень плохое настроение.`, no ctx · `У ннё сейчас очень плохое настроение.`
- typo2: `играй`→`иггрйа` in `Не играй на улице.`: ctx ✓ `Не играй на улице.`, no ctx · `Не иггрйа на улице.`
- typo1: `врач`→`врча` in `Я не врач.`: ctx ✓ `Я не врач.`, no ctx · `Я не врча.`
- layout+typo: `дом`→`ljvv` in `В прошлом году они купили дом в Коннемаре.`: ctx ✓ `В прошлом году они купили дом в Коннемаре.`, no ctx ✗ `В прошлом году они купили домм в Коннемаре.`
- typo1: `искали`→`искалм` in `Мы искали её.`: ctx ✓ `Мы искали её.`, no ctx · `Мы искалм её.`
- layout+typo: `искали`→`bcrfkg` in `Мы искали её.`: ctx ✓ `Мы искали её.`, no ctx · `Мы bcrfkg её.`
- typo2: `любовь`→`любоьвы` in `Мне просто нужна твоя любовь.`: ctx ✓ `Мне просто нужна твоя любовь.`, no ctx · `Мне просто нужна твоя любоьвы.`
- typo2: `носят`→`нояят` in `Подростки часто носят странную одежду.`: ctx ✓ `Подростки часто носят странную одежду.`, no ctx · `Подростки часто нояят странную одежду.`
- typo2: `вам`→`вккм` in `Думаю, вам может понадобиться моя помощь.`: ctx · `Думаю, вккм может понадобиться моя помощь.`, no ctx ✗ `Думаю, drrv может понадобиться моя помощь.`
- typo1: `вечере`→`вечереа` in `Утро вечера мудренее, но и в вечере что-то есть.`: ctx · `Утро вечера мудренее, но и в вечереа что-то есть.`, no ctx ✗ `Утро вечера мудренее, но и в вечера что-то есть.`
- layout+typo: `это`→`'njj` in `Сделай это тактично.`: ctx ✓ `Сделай это тактично.`, no ctx · `Сделай 'njj тактично.`
- typo1: `сдал`→`садл` in `Он сдал экзамен кое-как.`: ctx ✓ `Он сдал экзамен кое-как.`, no ctx · `Он садл экзамен кое-как.`
- typo2: `языке`→`языычке` in `Он говорит на русском языке.`: ctx ✓ `Он говорит на русском языке.`, no ctx · `Он говорит на русском языычке.`
- typo1: `сердцем`→`сердцес` in `Я люблю её всем сердцем.`: ctx ✓ `Я люблю её всем сердцем.`, no ctx · `Я люблю её всем сердцес.`
- typo2: `пора`→`пооар` in `Тебе не кажется, что тебе пора?`: ctx ✓ `Тебе не кажется, что тебе пора?`, no ctx · `Тебе не кажется, что тебе пооар?`
- typo2: `родился`→`рился` in `Том родился в Бостоне.`: ctx ✓ `Том родился в Бостоне.`, no ctx · `Том рился в Бостоне.`
- typo1: `было`→`быо` in `Решение было единодушным.`: ctx ✓ `Решение было единодушным.`, no ctx · `Решение быо единодушным.`
- typo2: `Тому`→`Томууу` in `Расскажи всё Тому.`: ctx ✓ `Расскажи всё Тому.`, no ctx · `Расскажи всё Томууу.`
- typo2: `использовать`→`использооваь` in `Я могу использовать это?`: ctx ✓ `Я могу использовать это?`, no ctx · `Я могу использооваь это?`
- layout+typo: `ответ`→`jnstn` in `Большое спасибо! На столь быстрый ответ я не рассчитывал.`: ctx ✓ `Большое спасибо! На столь быстрый ответ я не рассчитывал.`, no ctx ✗ `Большое спасибо! На столь быстрый отыет я не рассчитывал.`
- typo1: `задание`→`заание` in `Учительница дала нам домашнее задание.`: ctx ✓ `Учительница дала нам домашнее задание.`, no ctx · `Учительница дала нам домашнее заание.`
- typo1: `думал`→`думла` in `Он думал, что я глуп.`: ctx ✓ `Он думал, что я глуп.`, no ctx · `Он думла, что я глуп.`
- layout+typo: `стране`→`cnhfyr` in `Многие молодые люди в этой стране не имеют работы.`: ctx ✓ `Многие молодые люди в этой стране не имеют работы.`, no ctx ✗ `Многие молодые люди в этой странк не имеют работы.`
- typo2: `слова`→`сдоав` in `Сотри ластиком эти слова.`: ctx ✓ `Сотри ластиком эти слова.`, no ctx · `Сотри ластиком эти сдоав.`
- typo1: `дам`→`даам` in `Я не дам тебе умереть.`: ctx ✓ `Я не дам тебе умереть.`, no ctx · `Я не даам тебе умереть.`
- typo2: `этом`→`эть` in `Она не хочет, чтобы ты об этом знал.`: ctx · `Она не хочет, чтобы ты об эть знал.`, no ctx ✗ `Она не хочет, чтобы ты об это знал.`
- typo1: `вас`→`вач` in `Мы вас знаем.`: ctx ✓ `Мы вас знаем.`, no ctx · `Мы вач знаем.`
- typo1: `города`→`гороад` in `Как далеко твой дом от города?`: ctx ✓ `Как далеко твой дом от города?`, no ctx · `Как далеко твой дом от гороад?`
- typo1: `этим`→`этимм` in `Любой пример с этим словом полезен для меня.`: ctx ✓ `Любой пример с этим словом полезен для меня.`, no ctx · `Любой пример с этимм словом полезен для меня.`
- typo2: `меня`→`мняе` in `Том попросил у меня тридцать долларов взаймы.`: ctx · `Том попросил у мняе тридцать долларов взаймы.`, no ctx ✗ `Том попросил у мне тридцать долларов взаймы.`
- typo2: `этому`→`этоп` in `Том не был счастлив по этому поводу.`: ctx · `Том не был счастлив по этоп поводу.`, no ctx ✗ `Том не был счастлив по этом поводу.`
- typo1: `это`→`эот` in `Ты сказал, что это важно.`: ctx ✓ `Ты сказал, что это важно.`, no ctx · `Ты сказал, что эот важно.`
- layout+typo: `чего`→`xthj` in `До чего же этот цветок красив!`: ctx ✓ `До чего же этот цветок красив!`, no ctx ✗ `До черо же этот цветок красив!`
- typo2: `мне`→`мнн` in `Ты не пойдёшь мне навстречу?`: ctx · `Ты не пойдёшь мнн навстречу?`, no ctx ✓ `Ты не пойдёшь мне навстречу?`

## en: 3000 sentences, 2995 clean in both modes

| case | ctx on (all) | ctx off (all) | ctx on (mid) | ctx off (mid) | n (all/mid) |
|---|---|---|---|---|---|
| typo1 | 72.9% / 2.85% | 70.3% / 3.20% | 76.2% / 2.45% | 73.2% / 2.84% | 2347/2081 |
| typo2 | 35.5% / 14.24% | 34.0% / 16.23% | 37.6% / 13.57% | 35.9% / 15.83% | 2409/2122 |
| layout | 99.6% / 0.05% | 99.6% / 0.05% | 99.5% / 0.00% | 99.5% / 0.00% | 2083/1762 |
| layout+typo | 48.9% (+11.3%) / 7.56% | 47.1% (+12.5%) / 7.86% | 50.9% (+10.1%) / 7.63% | 48.9% (+11.5%) / 7.98% | 1666/1441 |

Unknown-word proxy in sentences (each word treated as missing from the lists; changed with / without context):
- rare words (rank ≥ 20000 or unranked, 139 words): 22.30% / 25.18%
- frequent words (8562 words, flattering: the corpus knows them): 13.19% / 27.93%

Rare words kept only thanks to context:
- `snows` (→ `shows` without context) after Some("it")
- `untied` (→ `united` without context) after Some("tom")
- `Georgian` (→ `Georgina` without context) after Some("the")
- `Berber` (→ `Barber` without context) after Some("learning")
- `Berber` (→ `Barber` without context) after Some("learn")

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
- typo2: `game`→`gqam`: `If it rains, the gam will be canceled.`
- typo2: `was`→`wqax`: `There are good grounds for the view that Paris wax the scene of frequent riots even before the revolution of 1789.`
- layout+typo: `were`→`цууку`: `I wondered why people цуку staring at me.`
- typo2: `came`→`czem`: `The accident cem about through my carelessness.`
- typo2: `The`→`Thuee`: `Thee astronaut was slow to get used to the condition of weightlessness.`
- typo2: `seeing`→`seeib`: `You mustn't miss seein this wonderful film.`
- layout+typo: `have`→`рфмму`: `I don't рифму any family to support.`
- typo2: `hole`→`hpope`: `We dug a hope in the ground.`
- typo2: `health`→`healrth`: `He wanted to succeed, even at the cost of his hearth.`
- typo2: `heart`→`hret`: `He seems nice, but he's wicked at het.`
- typo1: `known`→`knwon`: `He is kwon to everyone.`
- layout+typo: `known`→`лтцщт`: `He is kwon to everyone.`
- layout+typo: `that`→`ерфен`: `I believe beyond doubt ерен she is innocent.`
- typo2: `week`→`weje`: `I'd like to visit you next week. What day of the wee would it be convenient for you?`
- typo1: `novel`→`novbel`: `I think his nobel is boring.`
- typo1: `country`→`countr`: `My counter is the world, and my religion is to do good.`
- typo2: `She`→`Sehh`: `Shh isn't picky.`
- typo2: `very`→`veeyr`: `Tom doesn't take veer good care of his dog.`
- typo2: `would`→`wule`: `I doubt that Tom wile remember me.`
- typo2: `Could`→`Coypd`: `Copd you help?`
- typo1: `boats`→`baots`: `It was one of the quietest bots I've ever been on.`
- typo2: `His`→`Hsxi`: `Рычш life has completely changed when he married her.`
- typo2: `Tom`→`Toomk`: `Took is in the hospital.`
- typo1: `what`→`whatf`: `You can't always do wharf you want.`
- typo2: `watching`→`watchimn`: `Tom's watchin.`
- typo1: `from`→`froom`: `I think he's hiding something groom me.`
- layout+typo: `that`→`ерфее`: `You look gorgeous in ерофее dress.`
- typo2: `Tom`→`Tkkm`: `I've located Ель.`
- typo2: `cracked`→`crackdr`: `I cracker the code.`
- typo2: `here`→`hrer`: `I wonder why Tom wanted us to be her.`
- typo2: `Tom`→`Tklm`: `Елдь checked the rearview mirror.`
- typo2: `man`→`mqnn`: `You're a brave mann.`
- typo2: `The`→`Theew`: `Threw coins are made of metal.`
- typo2: `Tom`→`Tmoo`: `Too took off his wig.`
- typo2: `like`→`lkwe`: `Don't act lowe a bull in a china shop.`
- layout+typo: `have`→`рфау`: `We're not supposed to фрау these.`
- typo1: `there`→`therre`: `Is terre a connection?`
- typo2: `were`→`wsee`: `We wee almost run over by a truck.`
- typo2: `child`→`cnid`: `Tom is still a cid.`
- typo2: `Tom`→`Tmpo`: `Tempo says he'll go with us.`
- typo2: `movies`→`mives`: `Do you like horror moves?`
- layout+typo: `might`→`ьшпрее`: `To think I mighty have been married to that woman! It was a close thing once.`
- layout+typo: `sure`→`ыгкуу`: `I'm not siree Tom is going to survive.`
- typo2: `today`→`tloady`: `Tom is going to stop by toady.`
- typo2: `raised`→`raoses`: `He raises his hand and so did Tomer.`
- layout+typo: `you`→`нщгш`: `That blouse fits yui well.`
- layout+typo: `with`→`цшпр`: `Are you going to go weigh Tom?`
- typo2: `should`→`shour`: `Where sour we start?`
- typo1: `are`→`adre`: `Hey, what andre you guys looking at?`
- typo2: `point`→`ponot`: `I fully agree with your pinot of view.`
- layout+typo: `you`→`нщог`: `I don't always understand everything ног write.`
- layout+typo: `movie`→`ьщмш`: `Tom's favorite moi is "Planet of the Squirrels."`
- typo2: `plan`→`plla`: `Tom didn't pla to go to Boston.`
- typo2: `her`→`hrrr`: `Can I talk to herr for a minute?`
- layout+typo: `her`→`руук`: `Tell руку you're sick.`
- typo2: `protect`→`proet`: `I can't poet them.`
- layout+typo: `upset`→`гзыу`: `I'm not use about him.`
- typo2: `like`→`lkew`: `Would you lew to watch TV?`
- typo1: `Boy`→`Boyy`: `Инн was I wrong.`
- typo2: `might`→`mognt`: `Tomorrow mont be better for me.`

Cases where context changed the outcome (✓ right, ✗ wrong, · unchanged):
- typo1: `him`→`hkm` in `I did write to him.`: ctx ✓ `I did write to him.`, no ctx ✗ `I did write to ролью`
- typo1: `him`→`hik` in `I had my radio repaired by him.`: ctx ✓ `I had my radio repaired by him.`, no ctx · `I had my radio repaired by hik.`
- typo2: `want`→`wnay` in `The more popcorn you eat, the more you want.`: ctx · `The more popcorn you eat, the more you wnay.`, no ctx ✗ `The more popcorn you eat, the more you way.`
- typo2: `her`→`hrr` in `At last, her dream to be a doctor came true.`: ctx · `At last, hrr dream to be a doctor came true.`, no ctx ✓ `At last, her dream to be a doctor came true.`
- typo2: `matter`→`matet` in `You should look into the matter at once.`: ctx ✓ `You should look into the matter at once.`, no ctx · `You should look into the matet at once.`
- typo1: `for`→`fof` in `Jim is short for James.`: ctx ✓ `Jim is short for James.`, no ctx · `Jim is short fof James.`
- typo2: `that`→`tgzt` in `Jane insisted that she was right.`: ctx · `Jane insisted tgzt she was right.`, no ctx ✗ `Jane insisted епяе she was right.`
- typo2: `been`→`bsen` in `The optimal formula which would encompass all these factors has not been proposed.`: ctx ✓ `The optimal formula which would encompass all these factors has not been proposed.`, no ctx · `The optimal formula which would encompass all these factors has not bsen proposed.`
- typo1: `where`→`wehre` in `This is the town where he was born.`: ctx ✓ `This is the town where he was born.`, no ctx · `This is the town wehre he was born.`
- layout+typo: `weekly`→`цуулзн` in `Here is a weekly magazine.`: ctx ✓ `Here is a weekly magazine.`, no ctx · `Here is a цуулзн magazine.`
- typo1: `woman`→`womn` in `The history of every country begins in the heart of a man or a woman.`: ctx ✓ `The history of every country begins in the heart of a man or a woman.`, no ctx · `The history of every country begins in the heart of a man or a womn.`
- typo1: `sea`→`swa` in `Do you travel by sea or by air?`: ctx ✓ `Do you travel by sea or by air?`, no ctx · `Do you travel by swa or by air?`
- layout+typo: `give`→`пшмуц` in `I give up.`: ctx ✗ `I givew up.`, no ctx ✗ `I gives up.`
- typo1: `eaten`→`eaen` in `I've eaten a great deal of food this week.`: ctx ✓ `I've eaten a great deal of food this week.`, no ctx · `I've eaen a great deal of food this week.`
- typo1: `late`→`latee` in `I worked hard till late last night, so I'll have to catch some z's after lunch if it's possible.`: ctx ✓ `I worked hard till late last night, so I'll have to catch some z's after lunch if it's possible.`, no ctx · `I worked hard till latee last night, so I'll have to catch some z's after lunch if it's possible.`
- typo1: `ago`→`agk` in `My father died three years ago, that is to say in 1977.`: ctx ✓ `My father died three years ago, that is to say in 1977.`, no ctx · `My father died three years agk, that is to say in 1977.`
- typo1: `the`→`thre` in `I refuse to answer the question.`: ctx ✓ `I refuse to answer the question.`, no ctx · `I refuse to answer thre question.`
- typo1: `doll`→`dlol` in `He presented her with a doll.`: ctx ✓ `He presented her with a doll.`, no ctx · `He presented her with a dlol.`
- typo2: `doll`→`doolll` in `He presented her with a doll.`: ctx ✓ `He presented her with a doll.`, no ctx · `He presented her with a doolll.`
- typo2: `world`→`woorpd` in `The whole world is off its rocker.`: ctx ✓ `The whole world is off its rocker.`, no ctx · `The whole woorpd is off its rocker.`
- typo1: `the`→`teh` in `Bill took the blame for the loss.`: ctx · `Bill took teh blame for the loss.`, no ctx ✓ `Bill took the blame for the loss.`
- typo2: `home`→`hoen` in `I have a lot of baggage, so I can't walk home.`: ctx · `I have a lot of baggage, so I can't walk hoen.`, no ctx ✗ `I have a lot of baggage, so I can't walk horn.`
- typo1: `him`→`hij` in `We congratulated him on winning a gold medal.`: ctx ✓ `We congratulated him on winning a gold medal.`, no ctx · `We congratulated hij on winning a gold medal.`
- typo1: `his`→`hiis` in `Let him go his own way.`: ctx · `Let him go hiis own way.`, no ctx ✓ `Let him go his own way.`
- layout+typo: `his`→`ршшы` in `Let him go his own way.`: ctx ✗ `Let him go hiis own way.`, no ctx ✓ `Let him go his own way.`
- typo2: `late`→`latgee` in `He is always late for school.`: ctx ✓ `He is always late for school.`, no ctx · `He is always latgee for school.`
- typo2: `made`→`maad` in `He made up his mind to go there.`: ctx · `He maad up his mind to go there.`, no ctx ✗ `He mad up his mind to go there.`
- typo2: `opened`→`oenecd` in `He opened the drawer, and took out a letter.`: ctx · `He oenecd the drawer, and took out a letter.`, no ctx ✗ `He щутусв the drawer, and took out a letter.`
- typo2: `for`→`fofc` in `He left for America by air.`: ctx · `He left fofc America by air.`, no ctx ✗ `He left ащас America by air.`
- typo2: `feels`→`feeel` in `He feels a lot better today.`: ctx · `He feeel a lot better today.`, no ctx ✗ `He feel a lot better today.`
- typo2: `committee`→`commjittes` in `She has been asked to sit on the committee.`: ctx ✓ `She has been asked to sit on the committee.`, no ctx · `She has been asked to sit on the commjittes.`
- typo2: `left`→`levf` in `This is one thing my father left to me.`: ctx · `This is one thing my father levf to me.`, no ctx ✗ `This is one thing my father дума to me.`
- typo1: `from`→`fom` in `It looks like you are from India.`: ctx · `It looks like you are fom India.`, no ctx ✓ `It looks like you are from India.`
- typo2: `from`→`froh` in `It looks like you are from India.`: ctx · `It looks like you are froh India.`, no ctx ✗ `It looks like you are frog India.`
- typo1: `soon`→`sooon` in `It rained as soon as he got home.`: ctx ✓ `It rained as soon as he got home.`, no ctx · `It rained as sooon as he got home.`
- layout+typo: `seems`→`ыууыь` in `It seems that Mr Hatoyama is living in company housing.`: ctx ✓ `It seems that Mr Hatoyama is living in company housing.`, no ctx ✗ `It seesm that Mr Hatoyama is living in company housing.`
- typo2: `soccer`→`socfr` in `The man likes to play soccer.`: ctx ✓ `The man likes to play soccer.`, no ctx · `The man likes to play socfr.`
- typo2: `working`→`wrming` in `I don't feel like working today, so let's go play soccer.`: ctx ✓ `I don't feel like working today, so let's go play soccer.`, no ctx · `I don't feel like wrming today, so let's go play soccer.`
- typo1: `tell`→`tll` in `I'm going to tell you everything.`: ctx ✓ `I'm going to tell you everything.`, no ctx · `I'm going to tll you everything.`
- typo2: `hurts`→`hurfws` in `What hurts you?`: ctx · `What hurfws you?`, no ctx ✓ `What hurts you?`

