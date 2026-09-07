import json
import pathlib

cfg = json.loads(pathlib.Path("config.json").read_text())
print(cfg["dataset"], cfg["cutoff"])
