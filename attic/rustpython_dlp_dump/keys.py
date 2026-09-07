import json
d=json.load(open('ast_data.json'))
keys=set(d['structs'].keys())
import subprocess
gv=set()
for line in open('/tmp/gv_map.txt'):
    line=line.strip()
    if not line: continue
    snake,typ=line.split()
    gv.add(typ)
print("ast_data keys:",len(keys))
print("gv node-types:",len(gv))
print("in gv not in ast_data:", sorted(gv-keys))
print("in ast_data not in gv:", sorted(keys-gv))
