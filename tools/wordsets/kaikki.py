#!/usr/bin/env python3
"""Extract every headword and inflected form from a kaikki.org Wiktionary JSONL dump.
usage: kaikki.py <dump.jsonl> <out.txt> ru|en   (stress marks stripped, multiword forms split)"""
import json, re, sys, unicodedata
src, dst, lang = sys.argv[1:4]
ok = re.compile(r'^[а-яё]+$' if lang == 'ru' else r'^[a-z]+$')
SKIP = {'romanization', 'transliteration', 'table-tags', 'inflection-template', 'class', 'error-unknown-tag', 'misspelling'}
out = set()
def add(s):
    s = unicodedata.normalize('NFD', s)
    s = ''.join(c for c in s if unicodedata.category(c) != 'Mn' or c in '\u0306\u0308')  # keep й, ё
    s = unicodedata.normalize('NFC', s).lower().replace('ё', 'е')
    for t in re.split(r'[\s/,()]+', s):
        if ok.match(t) and len(t) >= 2: out.add(t)
for line in open(src, encoding='utf-8'):
    try: e = json.loads(line)
    except Exception: continue
    if e.get('lang_code') != lang: continue
    senses = e.get('senses', [])
    if senses and all('misspelling' in (s.get('tags') or []) for s in senses): continue
    add(e.get('word', ''))
    for f in e.get('forms', []) or []:
        if SKIP & set(f.get('tags') or []): continue
        add(f.get('form', ''))
open(dst, 'w').write('\n'.join(sorted(out)) + '\n')
print(dst, len(out))
