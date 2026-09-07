import re, json
import os
CARGO_HOME = os.environ.get('CARGO_HOME') or os.path.expanduser('~/.cargo')
REG=__import__('glob').glob(f'{CARGO_HOME}/registry/src/*/rustpython-ast-0.4.0')[0]
src=open(REG+"/src/gen/generic.rs").read()
enums=set(re.findall(r'pub enum (\w+)<', src))
structs={}
for m in re.finditer(r'pub struct (\w+)<R = TextRange>\s*\{(.*?)\}', src, re.S):
    name=m.group(1); body=m.group(2)
    fields=[]
    for line in body.split('\n'):
        line=line.strip().strip(',')
        mm=re.match(r'pub (\w+):\s*(.+)$', line)
        if mm:
            fields.append((mm.group(1), mm.group(2).strip()))
    structs[name]=fields
# only emit overrides for struct types that are node types (exclude the 4 Mod* and any with OptionalRange)
out={}
for n,f in structs.items():
    rt=[t for (_,t) in f if _.lower()=='range']
    out[n]=f
json.dump({'enums':sorted(enums),'structs':out}, open('ast_data.json','w'), indent=1)
print("structs:", len(out), "enums:", len(enums))
# which have OptionalRange
print("OptionalRange-bearing:", [n for n,f in out.items() if any('OptionalRange' in t for _,t in f)])
