import re, pathlib, sys
src = pathlib.Path("crates/pyxray-render/src/theme.rs").read_text()

def lum(c):
    def f(v):
        v /= 255
        return v/12.92 if v <= 0.03928 else ((v+0.055)/1.055)**2.4
    r,g,b = c
    return 0.2126*f(r) + 0.7152*f(g) + 0.0722*f(b)

def ratio(a, b):
    la, lb = lum(a), lum(b)
    hi, lo = max(la,lb), min(la,lb)
    return (hi+0.05)/(lo+0.05)

def hex2rgb(h): return (int(h[0:2],16), int(h[2:4],16), int(h[4:6],16))

EFFECTS = ["read","write","delete","net","shell","env","eval","print","rand","time","par","calc","exit"]
blocks = re.findall(r'pub const (\w+): Theme = Theme \{(.*?)\n\};', src, re.S)
worst_overall = []
for name, body in blocks:
    if 'Color::Reset' in body:
        print(f"{name.lower():<10} 16-colour / no-colour theme — skipped (terminal decides)")
        continue
    pal = dict(re.findall(r'(\w+): rgb\(0x([0-9a-f]{6})\)', body))
    fx = re.findall(r'rgb\(0x([0-9a-f]{6})\)', body.split("effects: [")[1])
    bg = hex2rgb(pal['bg'])
    rows = []
    for key in ("fg","dim","faint","rule","accent","accent_alt","ok","warn","danger"):
        if key in pal:
            rows.append((key, ratio(hex2rgb(pal[key]), bg)))
    for label, h in zip(EFFECTS, fx):
        rows.append(("fx."+label, ratio(hex2rgb(h), bg)))
    bad = [(k,v) for k,v in rows if v < 3.0 and k != "rule"]
    weak = [(k,v) for k,v in rows if 3.0 <= v < 4.5 and k not in ("rule","faint")]
    body_txt = dict(rows)["fg"]
    print(f"\n{name.lower():<10} bg #{pal['bg']}  body text {body_txt:.1f}:1")
    if bad:
        print("   under 3.0:1 →", ", ".join(f"{k} {v:.1f}" for k,v in sorted(bad, key=lambda x:x[1])))
    if weak:
        print("   3.0-4.5:1  →", ", ".join(f"{k} {v:.1f}" for k,v in sorted(weak, key=lambda x:x[1])))
    if not bad and not weak:
        print("   all roles at 4.5:1 or better")
    worst_overall.append((name.lower(), min(v for k,v in rows if k!="rule")))
print()
for n,v in sorted(worst_overall, key=lambda x:x[1]):
    print(f"  {n:<10} worst non-rule role: {v:.1f}:1")
