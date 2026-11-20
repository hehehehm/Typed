# Context eval: 0.3.73 (level Aggressive)

Cells: right (+ layout fixed, typo kept) / wrong (share of cases). `all` = every target, `mid` = targets after a word.

## ru: 3000 sentences, 2992 clean in both modes

| case | ctx on (all) | ctx off (all) | ctx on (mid) | ctx off (mid) | n (all/mid) |
|---|---|---|---|---|---|
| typo1 | 76.4% / 3.93% | 74.0% / 4.04% | 79.4% / 4.06% | 76.6% / 4.19% | 2699/2315 |
| typo2 | 57.0% / 11.47% | 53.1% / 12.78% | 61.5% / 11.64% | 57.0% / 13.17% | 2677/2285 |
| layout | 96.5% / 0.36% | 96.5% / 0.36% | 96.6% / 0.32% | 96.6% / 0.32% | 1951/1541 |
| layout+typo | 58.6% (+4.5%) / 7.60% | 57.2% (+5.1%) / 7.60% | 61.9% (+3.8%) / 7.73% | 60.2% (+4.6%) / 7.73% | 1724/1371 |

Unknown-word proxy in sentences (each word treated as missing from the lists; changed with / without context):
- rare words (rank ≥ 20000 or unranked, 770 words): 32.21% / 36.49%
- frequent words (5774 words, flattering: the corpus knows them): 20.02% / 33.88%

Rare words kept only thanks to context:
- `пустынях` (→ `пустоях` without context) after Some("в")
- `ширину` (→ `шину` without context) after Some("в")
- `пауков` (→ `парков` without context) after Some("может")
- `российский` (→ `росссийский` without context) after Some("москва")
- `удивлением` (→ `удалением` without context) after Some("с")
- `литра` (→ `лира` without context) after Some("три")
- `французски` (→ `франциски` without context) after Some("по")
- `огорчён` (→ `окончен` without context) after Some("очень")
- `плаваешь` (→ `плачешь` without context) after Some("хорошо")
- `сложил` (→ `служил` without context) after Some("я")
- `столкнул` (→ `стукнул` without context) after Some("том")
- `иврите` (→ `вините` without context) after Some("на")
- `Томе` (→ `Том` without context) after Some("о")
- `французски` (→ `франциски` without context) after Some("по")
- `подождал` (→ `побеждал` without context) after Some("немного")

Clean sentences changed: ctx on 8 (0.27%), ctx off 8 (0.27%)

FP examples (ctx on):
- `От похвалы не возгордись, а от хулы не гневись.` → `От похвалы не возгордись, а от [eks не гневись.`
- `Маленькие детки - маленькие бедки, большие детки - большие бедки.` → `Маленькие детки - маленькие белки, большие детки - большие белки.`
- `Если на руках ногти, то почему на ногах не рукти?` → `Если на руках ногти, то почему на ногах не рути?`
- `Сасай, лалка!` → `Спасай, лалка!`
- `Изаура — опытная женщина.` → `Из аура — опытная женщина.`
- `«Ух ты! Такое красивое утро у нас здесь сегодня! Мне в самом деле не терпится – а? Что такое?» – «Как дела, Линк?» – «Моршу, в чём дело?» – «Ха, всё просто. Благодаря денежному успеху моих предприятий и карьеры в битбоксе я разбогател!» – «Ого, и ты решил устроить нам вечеринку?» – «Ха, нет. Охрана, вышвырните этих гондонов».` → `«Ух ты! Такое красивое утро у нас здесь сегодня! Мне в самом деле не терпится – а? Что такое?» – «Как дела, Линк?» – «Моршу, в чём дело?» – «Ха, всё просто. Благодаря денежному успеху моих предприятий и карьеры в битбоксе я разбогател!» – «Ого, и ты решил устроить нам вечеринку?» – «Ха, нет. Охрана, вышвырните этих городов».`
- `"Взлёт и падение Зигги Стардаста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.` → `"Взлёт и падение Зигги Староста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.`
- `Столица Танзании - Додома.` → `Столица Танзании - До дома.`

FP examples (ctx off):
- `От похвалы не возгордись, а от хулы не гневись.` → `От похвалы не возгордись, а от [eks не гневись.`
- `Маленькие детки - маленькие бедки, большие детки - большие бедки.` → `Маленькие детки - маленькие белки, большие детки - большие белки.`
- `Если на руках ногти, то почему на ногах не рукти?` → `Если на руках ногти, то почему на ногах не руки?`
- `Сасай, лалка!` → `Спасай, лалка!`
- `Изаура — опытная женщина.` → `Из аура — опытная женщина.`
- `«Ух ты! Такое красивое утро у нас здесь сегодня! Мне в самом деле не терпится – а? Что такое?» – «Как дела, Линк?» – «Моршу, в чём дело?» – «Ха, всё просто. Благодаря денежному успеху моих предприятий и карьеры в битбоксе я разбогател!» – «Ого, и ты решил устроить нам вечеринку?» – «Ха, нет. Охрана, вышвырните этих гондонов».` → `«Ух ты! Такое красивое утро у нас здесь сегодня! Мне в самом деле не терпится – а? Что такое?» – «Как дела, Линк?» – «Моршу, в чём дело?» – «Ха, всё просто. Благодаря денежному успеху моих предприятий и карьеры в битбоксе я разбогател!» – «Ого, и ты решил устроить нам вечеринку?» – «Ха, нет. Охрана, вышвырните этих городов».`
- `"Взлёт и падение Зигги Стардаста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.` → `"Взлёт и падение Зигги Староста и Пауков с Марса", пятый альбом Дэвида Боуи, был выпущен в 1972 году.`
- `Столица Танзании - Додома.` → `Столица Танзании - До дома.`

Wrong fixes with context (sample):
- layout+typo: `может`→`v;jtn`: `Странно, что у него моет быть такая вещь.`
- layout+typo: `креститься`→`rhtncbnmcz`: `Когда кажется, крутиться надо.`
- typo2: `как`→`ккм`: `Я не такой, кем ты.`
- layout+typo: `смыслах`→`cvskf[`: `Было жарко во всех смыла[ слова.`
- typo1: `посягай`→`посяшай`: `Не послушай на право другого!`
- typo2: `пошла`→`пошао`: `"Не прокатило", - подумала Мэри и пошел звонить Джону.`
- typo2: `Врун`→`Врку`: `Вру несчастный.`
- typo1: `постельном`→`посельном`: `Доктор настоял на посыльном режиме для него.`
- typo2: `верит`→`верт`: `Он, должно быть, дурак, если черт в такое.`
- typo2: `Том`→`Томмм`: `Томми надеется увидеть тебя в октябре.`
- typo1: `это`→`эот`: `Если бы по каким-либо причинам этот произошло, что бы ты сделал?`
- typo2: `было`→`былщ`: `В комнате был слишком многолюдно.`
- typo2: `самой`→`ссамо`: `Перестаньте лгать само себе!`
- typo2: `Тома`→`Тосм`: `Это Том?`
- typo2: `тоже`→`теож`: `Богатые тео плачут.`
- typo2: `стране`→`странеее`: `Многие молодые люди в этой страннее не имеют работы.`
- layout+typo: `для`→`lkkz`: `Это может быть полезно liz тебя.`
- typo2: `это`→`эоот`: `Я всё ещё не верю в этот.`
- layout+typo: `так`→`nfrr`: `Не nft уж и холодно.`
- typo2: `этом`→`эомм`: `Из-за твоих взбалмошных действий весь кропотливо составленный мной план пошёл насмарку! Об эмм ты не подумал?`
- typo1: `плодов`→`плодлов`: `Ветки дерева сгибаются под тяжестью полов.`
- typo1: `хочется`→`хочеся`: `Мне хосеся написать письмо.`
- typo2: `жена`→`жпан`: `Кстати о приключениях, вот и твоя жан идёт.`
- typo2: `Тома`→`Тосм`: `Я не хотел оскорблять Том.`
- typo1: `священнее`→`священнпе`: `Для индейцев звери были священник людей.`
- layout+typo: `окно`→`jryjj`: `Это Мэри вчера jury разбила.`
- typo2: `собираясь`→`собигаясо`: `Том помолчал, собираюсь с мыслями.`
- typo2: `захотим`→`захотьи`: `Том делает всё, что мы ни заходи.`
- typo2: `фотографирую`→`фоторафриую`: `Я впервые фотографию королеву.`
- typo2: `виляет`→`вилянрет`: `Здесь хвост влияет собакой.`
- typo2: `клумба`→`клубиа`: `Посреди сада была клуба.`
- typo1: `Пойди`→`Пйоди`: `Поди скажи ей, чтобы на Вадима не готовила, его не будет — аврал.`
- typo1: `сочная`→`сочнафя`: `Летом здесь колышется срочная высокая трава.`
- typo2: `сочная`→`сшогчная`: `Летом здесь колышется срочная высокая трава.`
- typo2: `словами`→`слтовам`: `Мне трудно выразить словам то, что я хочу сказать.`
- layout+typo: `Как`→`Rfrr`: `Rrr ты думаешь, кто Тому сказал?`
- typo2: `живу`→`жтв`: `Я не жив в Бостоне.`
- layout+typo: `могу`→`vjie`: `Я не vie забыть их.`
- typo1: `стать`→`стато`: `Я хотел бы сато поэтом.`
- layout+typo: `сидят`→`cbblzn`: `Том и Мэри clan за столиком в тихом уголке.`
- typo1: `ещё`→`ещ`: `Я to работаю.`
- typo1: `стесняется`→`стесняеться`: `Он немного стесняться.`
- typo2: `убирался`→`убрипался`: `Ты убрался в доме.`
- layout+typo: `Когда`→`Rujlf`: `Кода было принято это решение?`
- typo2: `Мэри`→`Мэпр`: `Том на год старше Мэр.`
- typo2: `умеет`→`уметт`: `Твоя жена уметь петь?`
- typo2: `собственном`→`стбственнмо`: `Рассказ основан на его собственно опыте.`
- layout+typo: `друг`→`lheh`: `Я думаю, мы можем помочь leh другу.`
- typo2: `идеален`→`идаеле`: `Ты идеале.`
- typo2: `оторвалась`→`оооррвалась`: `Плюшевого мишку уронили на пол, и у него оторвались лапа.`
- layout+typo: `оторвалась`→`jnjhdfokfcm`: `Плюшевого мишку уронили на пол, и у него оторвались лапа.`
- typo2: `Том`→`Тооми`: `Томи отказался принять от Мэри подарок.`
- typo2: `нас`→`наав`: `У начав есть вакцина.`
- layout+typo: `здесь`→`pltc`: `Ничего plc не трогайте.`
- typo2: `молод`→`моллдо`: `Я был когда-то молодо.`
- typo2: `ведь`→`вежь`: `Черепахи врежь не кусаются, правда?`
- typo1: `тубе`→`туббе`: `Я никогда не играл на трубе.`
- typo2: `Это`→`Эоот`: `Этот такой тонкий намёк на толстые обстоятельства.`
- typo2: `догнать`→`дрнать`: `Мне не дать Тома.`
- typo2: `Как`→`Кекк`: `Кек мне за неделю похудеть на три килограмма?`

Cases where context changed the outcome (✓ right, ✗ wrong, · unchanged):
- typo2: `богат`→`богаир` in `Говорят, он был очень богат.`: ctx ✓ `Говорят, он был очень богат.`, no ctx · `Говорят, он был очень богаир.`
- typo2: `три`→`трмм` in `Я младше его на три года.`: ctx ✓ `Я младше его на три года.`, no ctx · `Я младше его на трмм года.`
- typo1: `борт`→`брт` in `Мы поднялись на борт в десять.`: ctx · `Мы поднялись на брт в десять.`, no ctx ✗ `Мы поднялись на брат в десять.`
- layout+typo: `весьма`→`dtcmf` in `Это весьма перспективный чешский писатель.`: ctx ✓ `Это весьма перспективный чешский писатель.`, no ctx · `Это dtcmf перспективный чешский писатель.`
- layout+typo: `знал`→`pyfm` in `Я ещё никогда не бывал в этом городе, и, что ещё более важно, я не знал ни слова на том языке, на котором там говорили.`: ctx ✓ `Я ещё никогда не бывал в этом городе, и, что ещё более важно, я не знал ни слова на том языке, на котором там говорили.`, no ctx · `Я ещё никогда не бывал в этом городе, и, что ещё более важно, я не pyfm ни слова на том языке, на котором там говорили.`
- typo2: `траве`→`травае` in `По траве не ходить.`: ctx ✓ `По траве не ходить.`, no ctx · `По травае не ходить.`
- layout+typo: `мной`→`vjyq` in `Ты сидишь между мной и Мег.`: ctx ✓ `Ты сидишь между мной и Мег.`, no ctx · `Ты сидишь между vjyq и Мег.`
- typo1: `эту`→`этиу` in `Кто нарисовал эту прекрасную картину?`: ctx ✓ `Кто нарисовал эту прекрасную картину?`, no ctx · `Кто нарисовал этиу прекрасную картину?`
- typo2: `смыслах`→`смсллах` in `Было жарко во всех смыслах слова.`: ctx ✓ `Было жарко во всех смыслах слова.`, no ctx · `Было жарко во всех смсллах слова.`
- typo1: `неё`→`ннё` in `У неё сейчас очень плохое настроение.`: ctx ✗ `У нее сейчас очень плохое настроение.`, no ctx · `У ннё сейчас очень плохое настроение.`
- typo2: `играй`→`иггрйа` in `Не играй на улице.`: ctx ✓ `Не играй на улице.`, no ctx · `Не иггрйа на улице.`
- typo1: `врач`→`врча` in `Я не врач.`: ctx ✓ `Я не врач.`, no ctx · `Я не врча.`
- layout+typo: `дом`→`ljvv` in `В прошлом году они купили дом в Коннемаре.`: ctx ✓ `В прошлом году они купили дом в Коннемаре.`, no ctx ✗ `В прошлом году они купили домм в Коннемаре.`
- typo1: `бывает`→`быват` in `Зимой какая бывает погода?`: ctx · `Зимой какая быват погода?`, no ctx ✓ `Зимой какая бывает погода?`
- typo1: `чтобы`→`чтоы` in `Открой упаковку осторожно, чтобы не повредить гумми-мишек.`: ctx ✓ `Открой упаковку осторожно, чтобы не повредить гумми-мишек.`, no ctx · `Открой упаковку осторожно, чтоы не повредить гумми-мишек.`
- layout+typo: `чтобы`→`xnjs` in `Открой упаковку осторожно, чтобы не повредить гумми-мишек.`: ctx ✓ `Открой упаковку осторожно, чтобы не повредить гумми-мишек.`, no ctx · `Открой упаковку осторожно, xnjs не повредить гумми-мишек.`
- typo1: `искали`→`искалм` in `Мы искали её.`: ctx ✓ `Мы искали её.`, no ctx · `Мы искалм её.`
- layout+typo: `искали`→`bcrfkg` in `Мы искали её.`: ctx ✓ `Мы искали её.`, no ctx · `Мы bcrfkg её.`
- typo2: `пошла`→`пошао` in `"Не прокатило", - подумала Мэри и пошла звонить Джону.`: ctx ✗ `"Не прокатило", - подумала Мэри и пошел звонить Джону.`, no ctx · `"Не прокатило", - подумала Мэри и пошао звонить Джону.`
- typo2: `без`→`беед` in `Человек может существовать и без друзей.`: ctx ✓ `Человек может существовать и без друзей.`, no ctx · `Человек может существовать и беед друзей.`
- typo2: `женщина`→`женщник` in `Говорят, что я пожилая женщина.`: ctx ✓ `Говорят, что я пожилая женщина.`, no ctx · `Говорят, что я пожилая женщник.`
- typo2: `знаю`→`зннад` in `Насколько я знаю, это единственно возможный перевод.`: ctx ✓ `Насколько я знаю, это единственно возможный перевод.`, no ctx · `Насколько я зннад, это единственно возможный перевод.`
- typo2: `собака`→`союбка` in `Мне нравится ваша собака.`: ctx ✓ `Мне нравится ваша собака.`, no ctx · `Мне нравится ваша союбка.`
- typo2: `интересны`→`интеаренсы` in `Эти рассказы для меня очень интересны.`: ctx · `Эти рассказы для меня очень интеаренсы.`, no ctx ✗ `Эти рассказы для меня очень интересы.`
- typo2: `помолчать`→`помовчать` in `А не помолчать ли Вам?`: ctx ✗ `А не помогать ли Вам?`, no ctx · `А не помовчать ли Вам?`
- typo1: `вечере`→`вечереа` in `Утро вечера мудренее, но и в вечере что-то есть.`: ctx · `Утро вечера мудренее, но и в вечереа что-то есть.`, no ctx ✗ `Утро вечера мудренее, но и в вечера что-то есть.`
- typo1: `был`→`былл` in `Я был оскорблён.`: ctx · `Я былл оскорблён.`, no ctx ✓ `Я был оскорблён.`
- typo2: `был`→`быллл` in `Я был оскорблён.`: ctx · `Я быллл оскорблён.`, no ctx ✓ `Я был оскорблён.`
- typo2: `повесил`→`пвеил` in `Том повесил на стену календарь.`: ctx · `Том пвеил на стену календарь.`, no ctx ✗ `Том поверил на стену календарь.`
- typo2: `старом`→`сатрмо` in `Я не люблю, когда меня держат за болвана в старом польском преферансе.`: ctx ✓ `Я не люблю, когда меня держат за болвана в старом польском преферансе.`, no ctx ✗ `Я не люблю, когда меня держат за болвана в само польском преферансе.`
- typo1: `сдал`→`садл` in `Он сдал экзамен кое-как.`: ctx ✓ `Он сдал экзамен кое-как.`, no ctx · `Он садл экзамен кое-как.`
- typo2: `сдал`→`сакдл` in `Он сдал экзамен кое-как.`: ctx · `Он сакдл экзамен кое-как.`, no ctx ✗ `Он сад экзамен кое-как.`
- typo1: `это`→`эот` in `Если бы по каким-либо причинам это произошло, что бы ты сделал?`: ctx ✗ `Если бы по каким-либо причинам этот произошло, что бы ты сделал?`, no ctx · `Если бы по каким-либо причинам эот произошло, что бы ты сделал?`
- layout+typo: `роет`→`hjtfn` in `Он роет свою собственную могилу.`: ctx ✗ `Он роеат свою собственную могилу.`, no ctx ✗ `Он рот свою собственную могилу.`
- typo1: `сердцем`→`сердцес` in `Я люблю её всем сердцем.`: ctx ✓ `Я люблю её всем сердцем.`, no ctx · `Я люблю её всем сердцес.`
- typo2: `простить`→`прсттить` in `Я не могу его простить.`: ctx ✓ `Я не могу его простить.`, no ctx · `Я не могу его прсттить.`
- typo2: `едите`→`едмит` in `Вы не едите мясо?`: ctx ✓ `Вы не едите мясо?`, no ctx · `Вы не едмит мясо?`
- typo1: `пора`→`поа` in `Тебе не кажется, что тебе пора?`: ctx ✓ `Тебе не кажется, что тебе пора?`, no ctx · `Тебе не кажется, что тебе поа?`
- typo1: `было`→`быо` in `Решение было единодушным.`: ctx ✓ `Решение было единодушным.`, no ctx · `Решение быо единодушным.`
- typo2: `Тому`→`Томууу` in `Расскажи всё Тому.`: ctx ✓ `Расскажи всё Тому.`, no ctx · `Расскажи всё Томууу.`

## en: 3000 sentences, 2994 clean in both modes

| case | ctx on (all) | ctx off (all) | ctx on (mid) | ctx off (mid) | n (all/mid) |
|---|---|---|---|---|---|
| typo1 | 73.2% / 3.11% | 71.2% / 3.58% | 76.5% / 2.74% | 74.2% / 3.27% | 2346/2080 |
| typo2 | 51.6% / 14.91% | 48.5% / 17.15% | 55.9% / 14.29% | 52.5% / 16.83% | 2408/2121 |
| layout | 99.6% / 0.05% | 99.6% / 0.05% | 99.5% / 0.00% | 99.5% / 0.00% | 2082/1761 |
| layout+typo | 50.9% (+11.0%) / 10.39% | 49.2% (+11.9%) / 10.75% | 53.3% (+9.7%) / 10.76% | 51.5% (+10.7%) / 11.18% | 1665/1440 |

Unknown-word proxy in sentences (each word treated as missing from the lists; changed with / without context):
- rare words (rank ≥ 20000 or unranked, 139 words): 30.94% / 36.69%
- frequent words (8558 words, flattering: the corpus knows them): 17.33% / 32.32%

Rare words kept only thanks to context:
- `snows` (→ `shows` without context) after Some("it")
- `untied` (→ `united` without context) after Some("tom")
- `Georgian` (→ `Georgina` without context) after Some("the")
- `racking` (→ `racing` without context) after Some("m")
- `applauded` (→ `applause` without context) after Some("audience")
- `helipad` (→ `helped` without context) after Some("a")
- `woozy` (→ `way` without context) after Some("little")
- `Berber` (→ `Barber` without context) after Some("learning")
- `Berber` (→ `Barber` without context) after Some("learn")

Clean sentences changed: ctx on 6 (0.20%), ctx off 6 (0.20%)

FP examples (ctx on):
- `What a surprise! The onsen lodge had turned into a fish breeding farm.` → `What a surprise! The one lodge had turned into a fish breeding farm.`
- `Fadil and Layla hitched a ride to the motel.` → `Fail and Layla hitched a ride to the motel.`
- `I'll help you find Fadil.` → `I'll help you find Fail.`
- `Fadil tried to focus on his family.` → `Fail tried to focus on his family.`
- `Go help Fadil.` → `Go help Fail.`
- `Firmus is a Berber king from Kabylia.` → `Firms is a Berber king from Kabylia.`

FP examples (ctx off):
- `What a surprise! The onsen lodge had turned into a fish breeding farm.` → `What a surprise! The one lodge had turned into a fish breeding farm.`
- `Fadil and Layla hitched a ride to the motel.` → `Fail and Layla hitched a ride to the motel.`
- `I'll help you find Fadil.` → `I'll help you find Fail.`
- `Fadil tried to focus on his family.` → `Fail tried to focus on his family.`
- `Go help Fadil.` → `Go help Fail.`
- `Firmus is a Berber king from Kabylia.` → `Firms is a Berber king from Kabylia.`

Wrong fixes with context (sample):
- typo2: `game`→`gqam`: `If it rains, the gam will be canceled.`
- typo2: `was`→`wqax`: `There are good grounds for the view that Paris wax the scene of frequent riots even before the revolution of 1789.`
- layout+typo: `you`→`нщог`: `Looks like ног didn't study much for the test.`
- typo2: `came`→`czem`: `The accident cem about through my carelessness.`
- typo2: `The`→`Thuee`: `Thee astronaut was slow to get used to the condition of weightlessness.`
- typo2: `seeing`→`seeib`: `You mustn't miss seein this wonderful film.`
- layout+typo: `where`→`црцку`: `I wonder цру he is hiding.`
- layout+typo: `have`→`рфмму`: `I don't рифму any family to support.`
- typo2: `home`→`hoen`: `I have a lot of baggage, so I can't walk horn.`
- typo2: `goes`→`glew`: `It glee without saying that he's the best person for the job.`
- typo2: `health`→`healrth`: `He wanted to succeed, even at the cost of his hearth.`
- typo2: `the`→`thesw`: `He gave a rap on these door.`
- typo2: `heart`→`hret`: `He seems nice, but he's wicked at het.`
- layout+typo: `that`→`ерфен`: `I believe beyond doubt ерен she is innocent.`
- typo2: `often`→`ofefn`: `She would offend play tricks on me.`
- typo1: `wants`→`wnats`: `She whats but doesn't need it.`
- layout+typo: `tell`→`еуддд`: `I'm going to еду you everything.`
- typo2: `She`→`Sehh`: `Shh isn't picky.`
- typo2: `very`→`veyt`: `The senators were vet interested in what he had to say.`
- typo2: `losing`→`loisin`: `You're losin yourself in this minor job, when you could be the owner of an international enterprise.`
- typo2: `people`→`poele`: `It seems that many pele don't know the difference between "loose" and "lose".`
- typo2: `hurt`→`huyr`: `Don't hurry her.`
- typo2: `think`→`thikb`: `Whenever I hear that song, I решил of my younger days.`
- typo2: `think`→`tuhnk`: `The hospital that I went to for my allergies recommended me a shampoo that wouldn't trigger them. It's certainly effective - I thunk - but it's just way too expensive.`
- typo1: `boats`→`baots`: `It was one of the quietest bots I've ever been on.`
- typo2: `boats`→`bostts`: `It was one of the quietest boss I've ever been on.`
- typo2: `His`→`Hsxi`: `Рычш life has completely changed when he married her.`
- layout+typo: `hurt`→`ргрке`: `Did Tom get роке in the accident yesterday?`
- layout+typo: `want`→`цфет`: `Do you цвет something bad to happen?`
- layout+typo: `from`→`аккщь`: `I think he's hiding something аль me.`
- layout+typo: `that`→`ерфее`: `You look gorgeous in ерофее dress.`
- typo2: `find`→`finhds`: `Tom, tell me the truth because I'll finds out anyway.`
- typo2: `here`→`hsdre`: `I'm sitting рывку.`
- typo2: `told`→`tocl`: `I haven't tock Tom anything yet.`
- layout+typo: `know`→`лтщщц`: `I don't лиц why Tom got sick.`
- typo2: `Tom`→`Tklm`: `Елдь checked the rearview mirror.`
- typo1: `cab`→`cabn`: `I'll get a can.`
- typo2: `The`→`Theew`: `Threw coins are made of metal.`
- typo2: `kind`→`kdi`: `Tom said this kid of thing happens all the time.`
- layout+typo: `kind`→`лшитв`: `Tom said this лив of thing happens all the time.`
- typo2: `camp`→`cqap`: `Tom hated every day of summer cap.`
- typo2: `have`→`hsae`: `What we hae is enough.`
- typo2: `there`→`trhee`: `You should've been thee.`
- layout+typo: `three`→`ерку`: `It is exactly реку o'clock now.`
- typo2: `This`→`Tjhi`: `Thi is so romantic.`
- typo2: `were`→`wsee`: `We wee almost run over by a truck.`
- typo2: `movies`→`mives`: `Do you like horror moves?`
- typo2: `went`→`wesn`: `Tom thinks he knows where Mary wasn.`
- layout+typo: `take`→`елу`: `If you want to еду a break, just say so.`
- typo2: `pay`→`pzau`: `How are you going to pau me?`
- typo2: `his`→`hij`: `Tom told us about him trip.`
- typo1: `means`→`meana`: `That mana a lot to me.`
- layout+typo: `with`→`цшпр`: `Are you going to go weigh Tom?`
- layout+typo: `like`→`дшшлу`: `You're going to делу working for me.`
- typo2: `should`→`shour`: `Where sour we start?`
- layout+typo: `very`→`мвукн`: `Tom has been мун friendly toward me.`
- typo2: `That`→`Thha`: `Tha looked like it hurt.`
- typo2: `sure`→`sudr`: `Tom isn't as sur of himself as he used to be.`
- layout+typo: `you`→`нщог`: `I don't always understand everything ног write.`
- typo2: `Tom`→`Tlmo`: `Tomo and I got off on the wrong foot.`

Cases where context changed the outcome (✓ right, ✗ wrong, · unchanged):
- typo2: `story`→`soyr` in `Please go on with your story.`: ctx · `Please go on with your soyr.`, no ctx ✗ `Please go on with your sorry.`
- typo1: `him`→`hik` in `I had my radio repaired by him.`: ctx ✓ `I had my radio repaired by him.`, no ctx · `I had my radio repaired by hik.`
- typo2: `want`→`wnay` in `The more popcorn you eat, the more you want.`: ctx · `The more popcorn you eat, the more you wnay.`, no ctx ✗ `The more popcorn you eat, the more you way.`
- typo2: `sweet`→`sweyt` in `The sound of the violin is very sweet.`: ctx ✓ `The sound of the violin is very sweet.`, no ctx · `The sound of the violin is very sweyt.`
- typo2: `her`→`hrr` in `At last, her dream to be a doctor came true.`: ctx · `At last, hrr dream to be a doctor came true.`, no ctx ✓ `At last, her dream to be a doctor came true.`
- typo2: `matter`→`matet` in `You should look into the matter at once.`: ctx ✓ `You should look into the matter at once.`, no ctx · `You should look into the matet at once.`
- typo1: `for`→`fof` in `Jim is short for James.`: ctx ✓ `Jim is short for James.`, no ctx · `Jim is short fof James.`
- typo2: `that`→`tgzt` in `Jane insisted that she was right.`: ctx · `Jane insisted tgzt she was right.`, no ctx ✗ `Jane insisted епяе she was right.`
- typo2: `where`→`whette` in `This is the town where he was born.`: ctx ✓ `This is the town where he was born.`, no ctx · `This is the town whette he was born.`
- typo2: `sweets`→`sweerrs` in `There were various kinds of sweets.`: ctx ✓ `There were various kinds of sweets.`, no ctx · `There were various kinds of sweerrs.`
- typo1: `woman`→`womn` in `The history of every country begins in the heart of a man or a woman.`: ctx ✓ `The history of every country begins in the heart of a man or a woman.`, no ctx · `The history of every country begins in the heart of a man or a womn.`
- typo1: `sea`→`swa` in `Do you travel by sea or by air?`: ctx ✓ `Do you travel by sea or by air?`, no ctx ✗ `Do you travel by saw or by air?`
- typo2: `sea`→`sewaa` in `Do you travel by sea or by air?`: ctx ✓ `Do you travel by sea or by air?`, no ctx · `Do you travel by sewaa or by air?`
- typo2: `turns`→`tures` in `The birds ate the bugs taking turns.`: ctx · `The birds ate the bugs taking tures.`, no ctx ✗ `The birds ate the bugs taking tires.`
- layout+typo: `give`→`пшмуц` in `I give up.`: ctx ✗ `I givew up.`, no ctx ✗ `I gives up.`
- typo1: `eaten`→`eaen` in `I've eaten a great deal of food this week.`: ctx ✓ `I've eaten a great deal of food this week.`, no ctx · `I've eaen a great deal of food this week.`
- typo2: `eaten`→`eawen` in `I've eaten a great deal of food this week.`: ctx ✓ `I've eaten a great deal of food this week.`, no ctx · `I've eawen a great deal of food this week.`
- typo1: `cannot`→`cannt` in `I cannot get enough sleep any more.`: ctx ✓ `I cannot get enough sleep any more.`, no ctx · `I cannt get enough sleep any more.`
- layout+typo: `gave`→`пфау` in `The dog gave a yelp when I trod on its paw.`: ctx ✗ `The dog gafe a yelp when I trod on its paw.`, no ctx ✓ `The dog gave a yelp when I trod on its paw.`
- typo2: `first`→`fiets` in `We went to Hawaii last year for the first time.`: ctx ✓ `We went to Hawaii last year for the first time.`, no ctx · `We went to Hawaii last year for the fiets time.`
- typo2: `are`→`aeed` in `We are to meet in front of his house.`: ctx ✗ `We add to meet in front of his house.`, no ctx · `We aeed to meet in front of his house.`
- typo2: `total`→`totazll` in `I'm a total stranger to things of this kind.`: ctx ✓ `I'm a total stranger to things of this kind.`, no ctx · `I'm a totazll stranger to things of this kind.`
- typo1: `the`→`thre` in `I refuse to answer the question.`: ctx ✓ `I refuse to answer the question.`, no ctx · `I refuse to answer thre question.`
- typo2: `said`→`sizad` in `I said nothing, which made him angry.`: ctx ✗ `I sid nothing, which made him angry.`, no ctx · `I sizad nothing, which made him angry.`
- typo1: `doll`→`dlol` in `He presented her with a doll.`: ctx ✓ `He presented her with a doll.`, no ctx · `He presented her with a dlol.`
- typo2: `made`→`masdd` in `Careful as he was, he made an unexpected mistake.`: ctx ✓ `Careful as he was, he made an unexpected mistake.`, no ctx · `Careful as he was, he masdd an unexpected mistake.`
- typo1: `him`→`hij` in `We congratulated him on winning a gold medal.`: ctx ✓ `We congratulated him on winning a gold medal.`, no ctx · `We congratulated hij on winning a gold medal.`
- typo1: `the`→`thre` in `I caught him by the arm before he could fall.`: ctx ✓ `I caught him by the arm before he could fall.`, no ctx · `I caught him by thre arm before he could fall.`
- typo1: `his`→`hiis` in `Let him go his own way.`: ctx · `Let him go hiis own way.`, no ctx ✓ `Let him go his own way.`
- layout+typo: `his`→`ршшы` in `Let him go his own way.`: ctx ✗ `Let him go hiis own way.`, no ctx ✓ `Let him go his own way.`
- typo2: `please`→`pleasrr` in `I found it difficult to please him.`: ctx · `I found it difficult to pleasrr him.`, no ctx ✓ `I found it difficult to please him.`
- typo2: `made`→`maad` in `He made up his mind to go there.`: ctx ✓ `He made up his mind to go there.`, no ctx ✗ `He mad up his mind to go there.`
- typo2: `opened`→`oenecd` in `He opened the drawer, and took out a letter.`: ctx ✓ `He opened the drawer, and took out a letter.`, no ctx ✗ `He щутусв the drawer, and took out a letter.`
- typo2: `for`→`fofc` in `He left for America by air.`: ctx · `He left fofc America by air.`, no ctx ✗ `He left ащас America by air.`
- typo2: `feels`→`feeel` in `He feels a lot better today.`: ctx ✓ `He feels a lot better today.`, no ctx ✗ `He feel a lot better today.`
- typo2: `went`→`wntt` in `He went on a one-day trip.`: ctx ✓ `He went on a one-day trip.`, no ctx · `He wntt on a one-day trip.`
- typo2: `committee`→`commjittes` in `She has been asked to sit on the committee.`: ctx ✓ `She has been asked to sit on the committee.`, no ctx · `She has been asked to sit on the commjittes.`
- typo2: `left`→`levf` in `This is one thing my father left to me.`: ctx · `This is one thing my father levf to me.`, no ctx ✗ `This is one thing my father дума to me.`
- typo2: `purpose`→`purpsos` in `A good purpose makes hard work a pleasure.`: ctx · `A good purpsos makes hard work a pleasure.`, no ctx ✓ `A good purpose makes hard work a pleasure.`
- typo2: `the`→`tjee` in `Regardless of the subject, he pretends to know all about it.`: ctx ✓ `Regardless of the subject, he pretends to know all about it.`, no ctx · `Regardless of tjee subject, he pretends to know all about it.`


