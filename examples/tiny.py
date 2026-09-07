import math
xs = [math.sin(i / 10) for i in range(100)]
peak = max(xs)
print(f"peak={peak:.4f}")
