"""Pretty-print a scenario transcript compactly (boards as glyph strings)."""
import json, re, sys
G = {"Red": "R", "Yellow": "Y", "Blue": "B", "Black": "K", "FourArrow": "+"}
def cell(c):
    if c is None: return "."
    if isinstance(c, str): return G[c]
    if "Slash" in c: return "/" if c["Slash"] == "Forward" else "\\"
    return {"UpRight": "1", "DownRight": "2", "DownLeft": "3", "UpLeft": "4"}[c["Arrow"]]
def walk(o):
    if isinstance(o, dict):
        if set(o) == {"size", "cells"}:
            return "▦" + "".join(cell(c) for c in o["cells"])
        if set(o) >= {"size", "counts", "source"}:
            return f"rules({o['size']},{o['source']},{o['resolution']},{'timed' if o['timed'] else 'untimed'})"
        return {k: walk(v) for k, v in o.items()}
    if isinstance(o, list): return [walk(v) for v in o]
    return o
for line in open(sys.argv[1]):
    m = re.match(r"(\w+ (?:<-|->) )(\{.*\})$", line.rstrip("\n"))
    if m:
        print(m.group(1) + json.dumps(walk(json.loads(m.group(2))), ensure_ascii=False, separators=(",", ":")))
    else:
        print(line.rstrip("\n"))
