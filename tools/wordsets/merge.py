#!/usr/bin/env python3
"""Merge full-form word sources into ru_all.txt / en_all.txt (see crates/core/data/README.md).

Run in a directory with the extracted sources (made by fetch.sh); prints per-source counts.
Only lowercase a-z / а-я words survive (ё folded into е); hyphenated forms are dropped
because the typing engine never sees them as one letters-only word.
"""
import os, re
RU = re.compile(r'^[а-я]+$'); EN = re.compile(r'^[a-z]+$')
def norm(w): return w.strip().lower().replace('ё', 'е')
def read(path):
    if not os.path.exists(path): return None
    return [norm(l.split('\t')[0]) for l in open(path, encoding='utf-8', errors='replace')]
def wf(path, zmin):
    if not os.path.exists(path): return None
    out = []
    for l in open(path, encoding='utf-8'):
        w, z = l.rstrip('\n').split('\t'); 
        if float(z) >= zmin: out.append(norm(w))
    return out
ZMIN = float(os.environ.get('WF_ZIPF', '2.5'))
SRC = {
 'ru': [('OpenCorpora (pymorphy3-dicts-ru)', read('oc_forms.txt')), ('Hunspell ru_RU (expanded)', read('hs_ru.txt')),
        ('Wiktionary forms (kaikki.org)', read('wikt_ru.txt')), (f'wordfreq large, zipf>={ZMIN}', wf('wf_ru.txt', ZMIN))],
 'en': [('SCOWL size<=80 (all variants)', read('scowl80.txt')), ('Hunspell en_US (expanded)', read('hs_en_us.txt')),
        ('Hunspell en_GB (expanded)', read('hs_en_gb.txt')), ('Wiktionary forms (kaikki.org)', read('wikt_en.txt')),
        (f'wordfreq large, zipf>={ZMIN}', wf('wf_en.txt', ZMIN))],
}
RU_AB = 'абвгдежзийклмнопрстуфхцчшщъыьэюя'; EN_AB = 'abcdefghijklmnopqrstuvwxyz'
def edits1(w, ab):
    for i in range(len(w)):
        yield w[:i] + w[i+1:]
        if i + 1 < len(w): yield w[:i] + w[i+1] + w[i] + w[i+2:]
        for c in ab: yield w[:i] + c + w[i+1:]
    for i in range(len(w) + 1):
        for c in ab: yield w[:i] + c + w[i:]
# Frequency lists (wordfreq) and Wiktionary also carry common misspellings (`becuase`,
# `recieve`, `teh`, `seperate`): a word found only there is dropped when it is one edit
# (letter added, dropped, replaced, or two neighbours swapped) away from a word of the
# curated morphological dictionaries.
FREQ_ONLY = {f'wordfreq large, zipf>={ZMIN}', 'Wiktionary forms (kaikki.org)'}

class Near:
    """One-edit neighbourhood test against a fixed word set."""
    def __init__(self, words, ab):
        self.c, self.ab = set(words), ab
        self.pos = None
        if len(self.c) < 1_500_000:  # positional deletes index (fast, memory-hungry)
            self.pos = set()
            for w in self.c:
                for i in range(len(w)): self.pos.add((i, w[:i] + w[i+1:]))
    def near(self, x):
        c = self.c
        if self.pos is None:
            return any(e in c for e in edits1(x, self.ab))
        for i in range(len(x)):
            d = x[:i] + x[i+1:]
            if d in c or (i, d) in self.pos: return True       # dropped / replaced letter
            if i + 1 < len(x) and x[:i] + x[i+1] + x[i] + x[i+2:] in c: return True  # swap
        return any((i, x) in self.pos for i in range(len(x) + 1))  # extra letter
for lang, srcs in SRC.items():
    ok = RU if lang == 'ru' else EN
    total = set()
    near = None
    for name, words in srcs:
        if words is None: print(f'{lang}: {name}: MISSING'); continue
        s = {w for w in words if ok.match(w) and len(w) >= 2}
        if name in FREQ_ONLY:
            if near is None:  # built once, from the curated sources before the first filtered one
                near = Near(total, RU_AB if lang == 'ru' else EN_AB)
            before = len(s - total)
            s = {w for w in s if w in total or not near.near(w)}
            print(f'{lang}: {name}: dropped {before - len(s - total)} near-misspellings')
        new = len(s - total); total |= s
        print(f'{lang}: {name}: {len(s)} forms, {new} new')
    open(f'{lang}_all.txt', 'w').write('\n'.join(sorted(total)) + '\n')
    print(f'{lang}: total {len(total)}')
