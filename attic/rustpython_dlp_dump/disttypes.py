import json
d=json.load(open('ast_data.json'))
types=set()
for n,f in d['structs'].items():
    for k,t in f:
        types.add(t)
for t in sorted(types):
    print(t)
