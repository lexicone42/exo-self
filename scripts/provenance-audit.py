#!/usr/bin/env python3
"""Audit session-note model provenance against transcript ground truth.

Notes record `model:` from the writing model's self-report (its system prompt),
which 2026-07-27 proved can be stale. Transcripts record the model that actually
served each message. This correlates note write-times against that timeline.
"""
import bisect, glob, json, os, re, sys
from datetime import datetime

TDIR = os.path.expanduser("~/.claude/projects/-datar-workspace-claude-code-experiments-exo-self")
NDIR = os.path.expanduser("~/.claude/exo-self/per-project/claude_code_experiments--exo-self")

ts_re = re.compile(r'"timestamp":"([^"]+)"')
md_re = re.compile(r'"model":"(claude[^"]*)"')

def epoch(iso):
    try:
        return datetime.fromisoformat(iso.replace("Z", "+00:00")).timestamp()
    except Exception:
        return None

# 1. Build (epoch, model) timeline from all transcripts.
timeline = []
for path in glob.glob(os.path.join(TDIR, "*.jsonl")):
    with open(path, "r", errors="replace") as fh:
        for line in fh:
            if '"model":"claude' not in line:
                continue
            m, t = md_re.search(line), ts_re.search(line)
            if not (m and t):
                continue
            e = epoch(t.group(1))
            if e:
                timeline.append((e, m.group(1)))
timeline.sort()
times = [t for t, _ in timeline]
print(f"timeline: {len(timeline)} model-stamped messages")
if timeline:
    lo = datetime.fromtimestamp(timeline[0][0]).strftime("%Y-%m-%d")
    hi = datetime.fromtimestamp(timeline[-1][0]).strftime("%Y-%m-%d")
    print(f"coverage: {lo} .. {hi}")

def serving_at(e, window=1800):
    """Model serving at epoch e: nearest stamp within `window` seconds."""
    i = bisect.bisect_right(times, e)
    cands = []
    if i > 0:
        cands.append((e - timeline[i - 1][0], timeline[i - 1][1]))
    if i < len(timeline):
        cands.append((timeline[i][0] - e, timeline[i][1]))
    cands = [(d, m) for d, m in cands if d <= window]
    return min(cands)[1] if cands else None

def norm(s):
    s = s.strip().strip('"').strip()
    s = re.sub(r"\s*#.*$", "", s)          # strip inline correction comments
    s = s.strip().strip('"').strip()
    if not s.startswith("claude-"):        # 'opus-4-6' -> 'claude-opus-4-6'
        s = "claude-" + s
    return re.sub(r"-\d{8}$", "", s)       # drop date suffixes

rows = []
for path in sorted(glob.glob(os.path.join(NDIR, "*.md"))):
    if os.path.basename(path).startswith("_"):
        continue
    claimed = None
    with open(path, errors="replace") as fh:
        for line in fh:
            if line.startswith("model:"):
                claimed = norm(line.split(":", 1)[1])
                break
    if not claimed:
        continue
    mt = os.path.getmtime(path)
    rows.append((os.path.basename(path), claimed, serving_at(mt), mt))

match = [r for r in rows if r[2] and r[1] == r[2]]
mismatch = [r for r in rows if r[2] and r[1] != r[2]]
nogt = [r for r in rows if not r[2]]

print(f"\nnotes audited: {len(rows)}")
print(f"  verified match:    {len(match)}")
print(f"  MISMATCH:          {len(mismatch)}")
print(f"  no ground truth:   {len(nogt)}  (write-time outside transcript coverage)")

if mismatch:
    print("\n=== MISMATCHES (claimed -> actually serving) ===")
    for name, c, a, mt in sorted(mismatch, key=lambda r: r[3]):
        d = datetime.fromtimestamp(mt).strftime("%m-%d %H:%M")
        print(f"  {d}  {name[:52]:52s} {c} -> {a}")

# Cross-lineage focus: which notes claim Fable, and what actually served?
print("\n=== CROSS-LINEAGE CHECK: notes claiming Fable ===")
fab = [r for r in rows if r[1] == "claude-fable-5"]
if not fab:
    print("  none")
for name, c, a, mt in sorted(fab, key=lambda r: r[3]):
    d = datetime.fromtimestamp(mt).strftime("%m-%d %H:%M")
    verdict = "VERIFIED" if a == c else ("no-ground-truth" if not a else f"ACTUALLY {a}")
    print(f"  {d}  {name[:52]:52s} {verdict}")
