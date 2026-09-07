#!/usr/bin/env python3
"""Summarise per-residue RMSF across a set of trajectory CSVs."""
import json
import os
from pathlib import Path

import numpy as np
import pandas as pd

DATA = Path(os.environ.get("RMSF_DIR", "data/rmsf"))
CUTOFF = 2.5


def load_run(path: Path) -> pd.DataFrame:
    """Read one run and drop the equilibration window."""
    df = pd.read_csv(path)
    df = df[df["time_ns"] > 10.0]
    df["run"] = path.stem
    return df


def summarise(frames):
    merged = pd.concat(frames, ignore_index=True)
    stats = merged.groupby("residue")["rmsf"].agg(["mean", "std", "count"])
    stats["flexible"] = stats["mean"] > CUTOFF
    return stats


def main():
    runs = sorted(DATA.glob("*.csv"))
    if not runs:
        raise SystemExit(f"no CSVs under {DATA}")

    frames = []
    total_rows = 0
    for path in runs:
        try:
            frame = load_run(path)
        except ValueError as exc:
            print(f"skipping {path.name}: {exc}")
            continue
        total_rows += len(frame)
        frames.append(frame)

    stats = summarise(frames)
    flexible = stats[stats["flexible"]]

    out = Path("results")
    out.mkdir(exist_ok=True)
    stats.to_csv(out / "rmsf_summary.csv")
    (out / "flexible.json").write_text(json.dumps(sorted(flexible.index.tolist())))

    print(f"{len(runs)} runs, {total_rows} rows")
    print(f"{len(flexible)} residues above {CUTOFF} A")
    print(np.round(stats["mean"].describe(), 3))


if __name__ == "__main__":
    main()
