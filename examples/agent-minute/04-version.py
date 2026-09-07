import requests

r = requests.get("https://pypi.org/pypi/numpy/json", timeout=5)
print(r.json()["info"]["version"])
