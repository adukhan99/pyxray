import json
d=json.load(open('ast_data.json'))
import sys
for n,f in d['structs'].items():
    sys.stderr.write("STRUCT %s\n"%n)
    for k,t in f:
        sys.stderr.write("   %s : %s\n"%(k,t))
