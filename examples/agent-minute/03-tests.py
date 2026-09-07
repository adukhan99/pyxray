import subprocess

result = subprocess.run(["pytest", "-q", "tests/"], check=False, capture_output=True)
print(result.stdout.decode()[-400:])
