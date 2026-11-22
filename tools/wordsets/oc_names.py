#!/usr/bin/env python3

import pymorphy3, re
NAME = {'Name', 'Surn', 'Patr', 'Geox', 'Orgn', 'Trad'}
m = pymorphy3.MorphAnalyzer()
ok = re.compile(r'^[а-яё]+$')
names, common = set(), set()
for word, tag, normal, para_id, idx in m.dictionary.iter_known_words():
    w = word.lower().replace('ё', 'е')
    if not ok.match(w):
        continue
    (names if NAME & set(tag.grammemes) else common).add(w)
out = sorted(names - common)
print(len(names), len(common), len(out))
open('oc_names.txt', 'w').write('\n'.join(out))
