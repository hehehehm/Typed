#!/usr/bin/env python3
"""Minimal Hunspell affix expander (PFX/SFX, cross products, one continuation level).
usage: unmunch.py ru|en <file.dic> <file.aff> <out.txt>"""
import re, sys
def load_aff(path):
    rules={}  # flag -> (kind, cross, [(strip, add, cond_re)])
    cur=None
    for line in open(path, encoding='utf-8', errors='replace'):
        p=line.split('#')[0].split() if not line.startswith('#') else []
        if len(p)>=4 and p[0] in ('SFX','PFX'):
            kind,flag=p[0],p[1]
            if flag not in rules and p[2] in ('Y','N') and len(p)==4:
                rules[flag]=(kind,p[2]=='Y',[]); continue
            if flag in rules and len(p)>=4:
                strip=p[2] if p[2]!='0' else ''
                add=p[3].split('/')[0]; add='' if add=='0' else add
                cont=p[3].split('/')[1] if '/' in p[3] else ''
                cond=p[4] if len(p)>4 else '.'
                rx=re.compile((cond+'$') if kind=='SFX' else ('^'+cond))
                rules[flag][2].append((strip,add,rx,cont))
    return rules
def expand(dic, aff):
    rules=load_aff(aff); out=set()
    lines=open(dic,encoding='utf-8',errors='replace').read().split('\n')[1:]
    for l in lines:
        l=l.strip().split('\t')[0].split(' ')[0]
        if not l: continue
        w,_,flags=l.partition('/')
        out.add(w)
        sfx=[]; pfx=[]
        for f in flags:
            r=rules.get(f)
            if not r: continue
            kind,cross,ents=r
            for strip,add,rx,cont in ents:
                if kind=='SFX':
                    if rx.search(w) and (not strip or w.endswith(strip)):
                        nw=(w[:len(w)-len(strip)] if strip else w)+add
                        out.add(nw); 
                        if cross: sfx.append(nw)
                        for f2 in cont:   # one level of continuation
                            r2=rules.get(f2)
                            if r2 and r2[0]=='SFX':
                                for s2,a2,rx2,_ in r2[2]:
                                    if rx2.search(nw) and (not s2 or nw.endswith(s2)):
                                        out.add((nw[:len(nw)-len(s2)] if s2 else nw)+a2)
                else:
                    if rx.search(w) and (not strip or w.startswith(strip)):
                        nw=add+w[len(strip):]; out.add(nw)
                        if cross: pfx.append((add,strip))
        # cross products
        for f in flags:
            r=rules.get(f)
            if r and r[0]=='PFX' and r[1]:
                for strip,add,rx,_ in r[2]:
                    for s in sfx:
                        if rx.search(s) and (not strip or s.startswith(strip)):
                            out.add(add+s[len(strip):])
    return out
if __name__=='__main__':
    lang,dic,aff,dst=sys.argv[1:5]
    ok=re.compile(r'^[а-яё]+$' if lang=='ru' else r"^[a-z]+$")
    s=set()
    for w in expand(dic,aff):
        w=w.lower().replace('ё','е')
        if ok.match(w): s.add(w)
    print(dst,len(s)); open(dst,'w').write('\n'.join(sorted(s)))
