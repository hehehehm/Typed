#!/usr/bin/env python3
"""Build crates/core/data/{en,ru}_brands.txt: brand / product / company / service names.

Inputs (see crates/core/data/README.md):
  curated_en.txt, curated_ru.txt          hand-made lists (this directory)
  <wd>/wd_*.tsv                           Wikidata labels (CC0), made by fetch_wikidata.sh
  <src>/en_all.txt, <src>/ru_all.txt      the full-form sets (tools/wordsets/merge.py output)
Usage: build.py <wd dir> <full-form dir> <repo root>

Only letters-only tokens of 4+ letters are kept (short ones collide with mistyped words),
lowercased, ё -> е. Every token whose reading in the other keyboard layout is a real word
there is dropped (it would stop a wrongly typed real word from being converted). Wikidata
tokens one edit away from a frequent word are dropped (they could be typos of that word)
unless the item is well known (single-token label, many sitelinks). Tokens already in the
dictionaries are not repeated. Brands are "known words" only: never offered as fixes.
"""
import csv, glob, os, re, sys
from collections import defaultdict

WD, SRC, REPO = sys.argv[1], sys.argv[2], sys.argv[3]
HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(REPO, 'crates/core/data')
MIN_LEN, MAX_LEN = 4, 20
WELL_KNOWN = 25          # sitelinks: a single-token label this famous may sit next to a word
NEAR_RANK = 20000        # "frequent word" for the near-typo filter (= typo::MAX_RANK)
EN_AB = 'abcdefghijklmnopqrstuvwxyz'; RU_AB = 'абвгдежзийклмнопрстуфхцчшщъыьэюя'
EN_KEYS = "qwertyuiop[]asdfghjkl;'zxcvbnm,.`"
RU_KEYS = 'йцукенгшщзхъфывапролджэячсмитьбюе'  # ` -> ё, folded into е
TO_RU = dict(zip(EN_KEYS, RU_KEYS)); TO_EN = {r: e for e, r in zip(EN_KEYS, RU_KEYS) if e != '`'}
TO_EN['е'] = 't'

def norm(w): return w.strip().lower().replace('ё', 'е')
def lines(p): return [norm(l) for l in open(p, encoding='utf-8') if l.strip() and not l.startswith('#')]

ranked = {'en': lines(f'{DATA}/en.txt'), 'ru': lines(f'{DATA}/ru.txt')}
rank = {l: {w: i for i, w in reversed(list(enumerate(ws)))} for l, ws in ranked.items()}
extras = {'en': set(lines(f'{DATA}/en_extra.txt')), 'ru': set(lines(f'{DATA}/ru_extra.txt'))}
print('loading full forms…', file=sys.stderr)
full = {l: set(open(f'{SRC}/{l}_all.txt', encoding='utf-8').read().split()) for l in ('en', 'ru')}
known = {l: full[l] | set(rank[l]) | extras[l] for l in ('en', 'ru')}
frequent = {l: {w for w, r in rank[l].items() if r < NEAR_RANK} | extras[l] for l in ('en', 'ru')}

def near_frequent(w, lang):
    ab = EN_AB if lang == 'en' else RU_AB; f = frequent[lang]
    for i in range(len(w)):
        if w[:i] + w[i+1:] in f: return True
        if i + 1 < len(w) and w[:i] + w[i+1] + w[i] + w[i+2:] in f: return True
        for c in ab:
            if c != w[i] and w[:i] + c + w[i+1:] in f: return True
    for i in range(len(w) + 1):
        for c in ab:
            if w[:i] + c + w[i:] in f: return True
    return False

def other_layout(w, lang):
    m = TO_RU if lang == 'en' else TO_EN
    return ''.join(m.get(c, '?') for c in w)

def collides(w, lang):
    """The other-layout reading of `w` is a real word there."""
    o = other_layout(w, lang); ol = 'ru' if lang == 'en' else 'en'
    # signs at the edges are trimmed by the engine (`эплу` -> `'gke` -> `gke`)
    return o in known[ol] or o.strip("'[];,.`") in known[ol]

TOKEN = {'en': re.compile(r'^[a-z]+$'), 'ru': re.compile(r'^[а-я]+$')}
def lang_of(t):
    for l, r in TOKEN.items():
        if r.match(t): return l
    return None
def ok_shape(t):
    return MIN_LEN <= len(t) <= MAX_LEN and len(set(t)) > 1 and re.search('[aeiouyаеиоуыэюя]', t)

# --- curated ---------------------------------------------------------------
RU_VOW = 'аеиоуыэюя'
def decline_ru(w):
    """Noun forms for a curated Russian brand spelling (see curated_ru.txt header)."""
    if w.endswith('~'): return [w[:-1]]
    if w[-1] in 'бвгджзклмнпрстфхцчшщ':
        soft = w[-1] in 'гкхжшчщ'; hiss = w[-1] in 'жшчщц'
        y = 'и' if soft else 'ы'
        return [w, w+'а', w+'у', w+('ем' if hiss else 'ом'), w+'е', w+y, w+('ей' if w[-1] in 'жшчщ' else 'ов'),
                w+'ам', w+'ами', w+'ах']
    if w.endswith(('ей', 'эй', 'ай')):
        s = w[:-1]; return [w, s+'я', s+'ю', s+'ем', s+'е', s+'и', s+'ев', s+'ям', s+'ями', s+'ях']
    if w.endswith('а') and len(w) > 3 and w[-2] not in RU_VOW:
        s = w[:-1]; y = 'и' if s[-1] in 'гкхжшчщ' else 'ы'
        return [w, s+y, s+'е', s+'у', s+'ой', s+'ам', s+'ами', s+'ах']
    return [w]

curated = {'en': set(), 'ru': set()}
for l in ('en', 'ru'):
    for raw in open(os.path.join(HERE, f'curated_{l}.txt'), encoding='utf-8'):
        raw = raw.split('#')[0].strip()
        if not raw: continue
        if ':' in raw:
            curated[l].update(norm(x) for x in raw.split(':', 1)[1].split()); continue
        for t in raw.split():
            t = norm(t)
            if l == 'ru': curated[l].update(decline_ru(t))
            else:
                curated[l].add(t.rstrip('~'))
                if not t.endswith('~') and t[-1] not in 'sxz' and len(t) >= 4: curated[l].add(t + 's')

# --- Wikidata --------------------------------------------------------------
wd = {'en': defaultdict(int), 'ru': defaultdict(int)}   # token -> best sitelinks
wd_single = {'en': set(), 'ru': set()}
labels = 0
for p in sorted(glob.glob(os.path.join(WD, 'wd_*.tsv'))):
    with open(p, encoding='utf-8') as f:
        next(f)
        for row in f:
            parts = row.rstrip('\n').split('\t')
            if len(parts) < 2: continue
            m = re.match(r'^"(.*)"@(en|ru)$', parts[0])
            if not m: continue
            labels += 1
            s = int(parts[1] or 0)
            label = norm(re.sub(r'[®™©]', '', m.group(1)))
            label = re.sub(r"['’`]", '', label)
            pieces = [x for x in re.split(r'[\s\-–—/.,:;!?()«»"&+]+', label) if x]
            single = len(pieces) == 1
            for t in pieces:
                l = lang_of(t)
                if not l or not ok_shape(t): continue
                wd[l][t] = max(wd[l][t], s)
                if single: wd_single[l].add(t)

# --- merge -----------------------------------------------------------------
stats = {}
for l in ('en', 'ru'):
    out = set(); st = defaultdict(int)
    for t in sorted(curated[l]):
        if not lang_of(t) == l or not ok_shape(t): st['curated: bad shape/short'] += 1; continue
        if collides(t, l): st['curated: other-layout word'] += 1; print(f'  {l} curated dropped (other layout): {t} -> {other_layout(t, l)}', file=sys.stderr); continue
        if t in known[l]: st['curated: already known'] += 1; continue
        out.add(t); st['curated: added'] += 1
    for t, s in sorted(wd[l].items()):
        if t in out: continue
        if t in known[l]: st['wikidata: already known'] += 1; continue
        if collides(t, l): st['wikidata: other-layout word'] += 1; continue
        if not (t in wd_single[l] and s >= WELL_KNOWN) and near_frequent(t, l):
            st['wikidata: near a frequent word'] += 1; continue
        out.add(t); st['wikidata: added'] += 1
    st['wikidata tokens'] = len(wd[l]); st['total new'] = len(out)
    stats[l] = dict(st)
    with open(f'{DATA}/{l}_brands.txt', 'w', encoding='utf-8') as f:
        f.write(f'# Brand / product / company / service names, generated by tools/brands/build.py\n'
                f'# (Wikidata labels, CC0, + curated lists, MIT). Known-only: never offered as fixes.\n')
        f.write('\n'.join(sorted(out)) + '\n')
print('labels read:', labels)
for l, st in stats.items():
    print(l, st)
