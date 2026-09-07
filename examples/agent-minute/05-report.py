from pathlib import Path

out = Path("out")
out.mkdir(exist_ok=True)
(out / "report.md").write_text("# Results\n\nAll runs completed.\n")
print("wrote", out / "report.md")
