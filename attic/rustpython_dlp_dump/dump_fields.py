import json
d=json.load(open('ast_data.json'))
for n,f in d['structs'].items():
    print(n, "::", ", ".join("%s:%s"%(k,t) for k,t in f))
