import json
import re
import sys
from pathlib import Path

SWEEP_FP = re.compile(r"^(\S.*?)\s+ran=(\d+)\s+pass=(\d+)\s+fail=(\d+)")
SWEEP_RECALL = re.compile(r"^(\S.*?)\s+ran=(\d+)\s+recall=\s*([\d.]+)%\s+wrong=\s*([\d.]+)%")
PCT = re.compile(r"([\d.]+)%")


def sweep_metrics(path):
    out = {}
    if not path.exists():
        return out
    for line in path.read_text().splitlines():
        m = SWEEP_RECALL.match(line)
        if m:
            name = m.group(1).strip()
            if int(m.group(2)) == 0:
                continue
            out[f"sweep/{name}/recall"] = (float(m.group(3)), "up")
            continue
        m = SWEEP_FP.match(line)
        if m:
            name = m.group(1).strip()
            ran, fail = int(m.group(2)), int(m.group(4))
            if ran == 0 or name.endswith("_intentional"):
                continue
            out[f"sweep/{name}/fail"] = (fail * 1e6 / ran, "down")
    return out


def table_rows(text):
    rows = []
    header = None
    title = ""
    for line in text.splitlines():
        if line.startswith("## "):
            title = line[3:].split(":")[0].strip()
        if not line.startswith("|"):
            header = None
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if all(set(c) <= set("-: ") for c in cells):
            continue
        if header is None:
            header = cells
            continue
        rows.append((header, cells, title))
    return rows


def correction_metrics(path):
    out = {}
    if not path.exists():
        return out
    text = path.read_text()
    section = ""
    for line in text.splitlines():
        if line.startswith("#"):
            section = line.strip("# ").strip()
    for header, cells, title in table_rows(text):
        key = cells[0]
        if header[0] == "kind" and title in ("Ru", "En"):
            key = f"{title} {key}"
        if header[:3] == ["set", "right", "wrong"]:
            r, w = PCT.findall(cells[1]), PCT.findall(cells[2])
            if r and w:
                out[f"correction/{key}/right"] = (float(r[0]), "up")
                out[f"correction/{key}/wrong"] = (float(w[0]), "down")
        elif header[0] in ("kind", "set") and header[-1] == "all":
            vals = PCT.findall(cells[-1])
            if header[0] == "kind" and len(vals) >= 2:
                out[f"correction/{key}/all/right"] = (float(vals[0]), "up")
                out[f"correction/{key}/all/wrong"] = (float(vals[-1]), "down")
            elif header[0] == "set" and vals:
                out[f"correction/unknown-fp/{key}"] = (float(vals[0]), "down")
    m = re.search(r"Curated FP set.*?changed (\d+)/(\d+)", text)
    if m:
        out["correction/curated_fp"] = (int(m.group(1)), "down")
    m = re.search(r"Known frequent words changed: (\d+)/(\d+)", text)
    if m:
        out["correction/frequent_changed"] = (int(m.group(1)), "down")
    return out


def context_metrics(path):
    out = {}
    if not path.exists():
        return out
    lang = ""
    seen = {}
    for line in path.read_text().splitlines():
        m = re.match(r"^## (\w+):", line)
        if m:
            lang = m.group(1)
            continue
        if line.startswith("|") and lang:
            cells = [c.strip() for c in line.strip().strip("|").split("|")]
            if cells[0] in ("case",) or set(cells[0]) <= set("-"):
                continue
            vals = PCT.findall(cells[1])
            if len(vals) >= 2:
                k = f"context/{lang}/{cells[0]}"
                n = seen.get(k, 0)
                seen[k] = n + 1
                if n:
                    k = f"{k}#{n}"
                out[f"{k}/right"] = (float(vals[0]), "up")
                out[f"{k}/wrong"] = (float(vals[-1]), "down")
    return out


def typist_metrics(path):
    out = {}
    if not path.exists():
        return out
    data = json.loads(path.read_text())
    for run in data.get("runs", []):
        p, lv = run.get("persona"), run.get("level")
        c = run["counts"]
        base = f"typist/{p}/{lv}"
        out[f"{base}/fp"] = (c["fp"]["n"], "down")
        out[f"{base}/missed_pct"] = (c["missed"]["pct"], "down")
        out[f"{base}/wrong_pct"] = (c["wrong"]["pct"], "down")
        out[f"{base}/fixed_pct"] = (c["fixed"]["pct"], "up")
    return out


def real_failures(path):
    if not path.exists():
        return None
    text = path.read_text()
    return "test result: ok" in text and "FAILED" not in text


def collect(d):
    d = Path(d)
    m = {}
    m.update(sweep_metrics(d / "sweep.txt"))
    m.update(correction_metrics(d / "correction.txt"))
    m.update(context_metrics(d / "context.txt"))
    m.update(typist_metrics(d / "typist_eval.json"))
    return m


def worse(new, ref, direction, tol):
    if direction == "up":
        return new < ref - tol
    return new > ref + tol


def main():
    if len(sys.argv) < 3:
        print("usage: release_gate.py NEW_DIR BASE_DIR [BASE_DIR ...] [--tol X] [--md OUT]")
        return 2
    args = sys.argv[1:]
    tol = 0.0
    md = None
    if "--tol" in args:
        i = args.index("--tol")
        tol = float(args[i + 1])
        del args[i : i + 2]
    if "--md" in args:
        i = args.index("--md")
        md = args[i + 1]
        del args[i : i + 2]
    new_dir, base_dirs = args[0], args[1:]
    new = collect(new_dir)
    bases = [collect(b) for b in base_dirs]
    failures = []
    lines = ["| metric | " + " | ".join(Path(b).name for b in base_dirs) + " | new | status |", "|---|" + "---|" * (len(base_dirs) + 2)]
    for key in sorted(set().union(*[set(b) for b in bases]) | set(new)):
        refs = [b.get(key) for b in bases]
        nv = new.get(key)
        status = "ok"
        if nv is None:
            status = "MISSING"
            failures.append(key)
        else:
            for r in refs:
                if r is not None and worse(nv[0], r[0], nv[1], tol):
                    status = "WORSE"
            if status != "ok":
                failures.append(key)
        cell = lambda v: "—" if v is None else (f"{v[0]:.3f}" if isinstance(v[0], float) else str(v[0]))
        lines.append(f"| {key} | " + " | ".join(cell(r) for r in refs) + f" | {cell(nv)} | {status} |")
    rf = real_failures(Path(new_dir) / "real_failures.txt")
    lines.append(f"| real_failures | | {'pass' if rf else 'FAIL'} | {'ok' if rf else 'FAIL'} |")
    if not rf:
        failures.append("real_failures")
    for key, limit in (("correction/curated_fp", 7), ("correction/frequent_changed", 0)):
        v = new.get(key)
        if v is None or v[0] > limit:
            failures.append(f"{key}>{limit}")
    report = "\n".join(lines)
    if md:
        Path(md).write_text(report + "\n")
    print(report)
    if failures:
        print(f"\nGATE FAILED: {len(failures)} metric(s): " + ", ".join(failures[:60]))
        return 1
    print("\nGATE PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
