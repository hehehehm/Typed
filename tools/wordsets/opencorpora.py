#!/usr/bin/env python3
"""All OpenCorpora word forms via the pymorphy3 Russian dictionary -> oc_forms.txt."""
import pymorphy3, re
m = pymorphy3.MorphAnalyzer()
d = m.dictionary
out=set()
ok=re.compile(r'^[а-яё]+(-[а-яё]+)*$')
for word, tag, normal, para_id, idx in d.iter_known_words():
    w=word.lower().replace('ё','е')
    if ok.match(w): out.add(w)
print(len(out))
open('oc_forms.txt','w').write('\n'.join(sorted(out)))
