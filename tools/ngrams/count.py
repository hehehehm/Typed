#!/usr/bin/env python3

import bz2, re, sys, os, collections

OOV_MIN = 30
BI_MIN = 2
TRI_MIN = 3

lang, out_dir, tatoeba = sys.argv[1], sys.argv[2], sys.argv[3]
wiki = sys.argv[4] if len(sys.argv) > 4 else None
max_wiki = int(sys.argv[5]) if len(sys.argv) > 5 else 40_000_000
here = os.path.dirname(os.path.abspath(__file__))
data = os.path.join(here, '..', '..', 'crates', 'core', 'data')
alpha = 'a-z' if lang == 'en' else 'а-яё'
word_re = re.compile(r"[%s]+|[.!?\n]|[^\s%s,;:\"'«»()\-—–]+" % (alpha, alpha), re.IGNORECASE)
letters_re = re.compile(r"^[%s]+$" % alpha)

def norm(w):
    return w.lower().replace('ё', 'е')

dict_words = set()
for f in (f'{lang}.txt', f'{lang}_extra.txt'):
    for line in open(os.path.join(data, f), encoding='utf-8'):
        line = line.strip()
        if line and not line.startswith('#'):
            dict_words.add(norm(line))

def sentences_tatoeba(held):
    for line in bz2.open(tatoeba, 'rt', encoding='utf-8'):
        p = line.rstrip('\n').split('\t')
        if len(p) < 3:
            continue
        if int(p[0]) % 50 == 0:
            held.write(p[2] + '\n')
            continue
        yield p[2]

WIKI_DROP = [re.compile(r) for r in [r"\{\{[^{}]*\}\}", r"<ref[^>]*/>", r"<ref.*?</ref>", r"<[^>]+>", r"\[\[(?:[^\]|]*\|)?([^\]]*)\]\]", r"\[https?://[^\] ]*( [^\]]*)?\]", r"'''?", r"&[a-z]+;"]]

def sentences_wiki():
    total = 0
    in_text = False
    for line in bz2.open(wiki, 'rt', encoding='utf-8', errors='ignore'):
        if '<text' in line:
            in_text = True
        if in_text:
            s = line
            if s.startswith(('{{', '|', '}}', '{|', '!', '*', '#', '=', '[[Файл', '[[File', '[[Категория', '[[Category')) or len(s) < 40:
                pass
            else:
                for _ in range(2):
                    s = WIKI_DROP[0].sub(' ', s)
                s = WIKI_DROP[1].sub(' ', s); s = WIKI_DROP[2].sub(' ', s); s = WIKI_DROP[3].sub(' ', s)
                s = WIKI_DROP[4].sub(r'\1', s); s = WIKI_DROP[5].sub(r' \1', s)
                s = WIKI_DROP[6].sub('', s); s = WIKI_DROP[7].sub(' ', s)
                for sent in re.split(r'(?<=[.!?])\s+', s):
                    total += sent.count(' ') + 1
                    yield sent
                if total > max_wiki:
                    return
        if '</text>' in line:
            in_text = False

def tokens(sent):
    for m in word_re.finditer(sent):
        yield m.group(0)

uni = collections.Counter()
raw = collections.Counter()
seqs_path = os.path.join(out_dir, 'seqs.tmp')
os.makedirs(out_dir, exist_ok=True)
held = open(os.path.join(out_dir, 'heldout.txt'), 'w', encoding='utf-8')
with open(seqs_path, 'w', encoding='utf-8') as seqs:
    srcs = [sentences_tatoeba(held)] + ([sentences_wiki()] if wiki else [])
    for src in srcs:
        for sent in src:
            run = []
            for t in tokens(sent):
                if t in '.!?\n' :
                    if run: seqs.write(' '.join(run) + '\n'); run = []
                    continue
                if not letters_re.match(t.lower()):
                    if run: seqs.write(' '.join(run) + '\n'); run = []
                    run = ['|']
                    continue
                w = norm(t)
                raw[w] += 1
                run.append(w)
            if run: seqs.write(' '.join(run) + '\n')
held.close()
vocab = dict_words | {w for w, c in raw.items() if c >= OOV_MIN}
print(f'{lang}: tokens {sum(raw.values())}, vocab {len(vocab)} ({len(dict_words)} dict)', file=sys.stderr)
bi = collections.Counter()
tri = collections.Counter()
nsent = 0
with open(seqs_path, encoding='utf-8') as seqs:
    for line in seqs:
        ws = line.split()
        if not ws:
            continue
        ctx = [] if ws[0] == '|' else ['<s>']
        if ws[0] == '|':
            ws = ws[1:]
        else:
            nsent += 1
        for w in ws:
            if w not in vocab:
                ctx = []
                continue
            uni[w] += 1
            if ctx:
                bi[(ctx[-1], w)] += 1
                if len(ctx) >= 2:
                    tri[(ctx[-2], ctx[-1], w)] += 1
            ctx = (ctx + [w])[-2:]
os.remove(seqs_path)
with open(os.path.join(out_dir, 'ngrams.tsv'), 'w', encoding='utf-8') as out:
    out.write(f'<s>\t{nsent}\n')
    for w, c in uni.items():
        out.write(f'{w}\t{c}\n')
    nb = nt = 0
    for (a, b), c in bi.items():
        if c >= BI_MIN:
            out.write(f'{a} {b}\t{c}\n'); nb += 1
    for (a, b, d), c in tri.items():
        if c >= TRI_MIN:
            out.write(f'{a} {b} {d}\t{c}\n'); nt += 1
print(f'{lang}: unigrams {len(uni)}, bigrams {nb} of {len(bi)}, trigrams {nt} of {len(tri)}, sentences {nsent}', file=sys.stderr)
