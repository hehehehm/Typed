#!/usr/bin/env python3

import re, wordfreq
for lang, ok in [('ru', r'^[а-яё]+$'), ('en', r'^[a-z]+$')]:
    with open(f'wf_{lang}.txt', 'w') as out:
        for w in wordfreq.top_n_list(lang, 10**7, wordlist='large'):
            w2 = w.lower().replace('ё', 'е')
            if re.match(ok, w2):
                out.write(f"{w2}\t{wordfreq.zipf_frequency(w, lang, wordlist='large')}\n")
