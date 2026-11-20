import sys
import os; d=os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'testdata') + os.sep
w=[l.rstrip('\r\n').split('\t') for l in open(d+'reference-dump.tsv',encoding='utf-8-sig',errors='replace')]
v=[l.rstrip('\r\n').split('\t') for l in open(d+'rdw-dump.tsv',encoding='utf-8',errors='replace')]
w=[x for x in w if len(x)==3 and x[2].strip()]
v=[x for x in v if len(x)==3 and x[2].strip()]
used=set(); tot=0;n=0;bad=0
for wp,wy,wt in w:
    for i,(vp,vy,vt) in enumerate(v):
        if i in used: continue
        if vt[:12]==wt[:12]:
            used.add(i); d=float(vy)-float(wy)
            ok = vp==wp and abs(d)<1.5
            bad += not ok
            print(f"{wt[:24]:26} ref p{wp} {float(wy):6.1f} | rdw p{vp} {float(vy):6.1f}  d={d:+.1f}{'' if ok else '  <<<'}")
            if vp==wp: tot+=abs(d); n+=1
            break
print("mean |dy| on same page:", round(tot/max(n,1),2), "over", n, "| mismatches:", bad)
