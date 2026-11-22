import ast
import bisect
import io
import re
import subprocess
import sys
import tokenize
from pathlib import Path

SKIP_PREFIXES = ("third_party/",)
EXTS = {".rs": "rust", ".py": "python", ".sh": "shell", ".ps1": "powershell", ".iss": "inno", ".vbs": "vbs", ".yml": "yaml", ".yaml": "yaml", ".toml": "toml"}
NAMES = {".gitignore": "hashline", ".gitattributes": "hashline"}


def kind_of(path):
    if path.startswith(SKIP_PREFIXES):
        return None
    name = Path(path).name
    if name in NAMES:
        return NAMES[name]
    return EXTS.get(Path(path).suffix)


def rust_spans(s):
    spans = []
    i, n = 0, len(s)
    while i < n:
        c = s[i]
        if s.startswith("//", i):
            j = s.find("\n", i)
            j = n if j < 0 else j
            spans.append((i, j))
            i = j
        elif s.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if s.startswith("/*", j):
                    depth += 1
                    j += 2
                elif s.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            spans.append((i, j))
            i = j
        elif (c == "r" or s.startswith("br", i)) and (i == 0 or not (s[i - 1].isalnum() or s[i - 1] == "_")):
            k = i + (2 if c == "b" else 1)
            m = re.match(r'#*"', s[k:])
            if m:
                hashes = len(m.group(0)) - 1
                end = s.find('"' + "#" * hashes, k + len(m.group(0)))
                i = n if end < 0 else end + 1 + hashes
            else:
                i += 1
        elif c == '"':
            j = i + 1
            while j < n and s[j] != '"':
                j += 2 if s[j] == "\\" else 1
            i = j + 1
        elif c == "'":
            if i + 1 < n and s[i + 1] == "\\":
                j = i + 2
                while j < n and s[j] != "'":
                    j += 2 if s[j] == "\\" else 1
                i = j + 1
            elif i + 2 < n and s[i + 2] == "'":
                i += 3
            else:
                i += 1
        else:
            i += 1
    return spans


def line_starts(s):
    starts = [0]
    for m in re.finditer("\n", s):
        starts.append(m.end())
    return starts


def python_spans(s):
    spans = []
    starts = line_starts(s)
    off = lambda row, col: starts[row - 1] + col
    for tok in tokenize.generate_tokens(io.StringIO(s).readline):
        if tok.type == tokenize.COMMENT:
            if tok.start[0] == 1 and tok.string.startswith("#!"):
                continue
            spans.append((off(*tok.start), off(*tok.end)))
    tree = ast.parse(s)
    for node in ast.walk(tree):
        if isinstance(node, (ast.Module, ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)) and node.body:
            first = node.body[0]
            if isinstance(first, ast.Expr) and isinstance(first.value, ast.Constant) and isinstance(first.value.value, str):
                a = off(first.lineno, first.col_offset)
                b = off(first.end_lineno, first.end_col_offset)
                spans.append((a, b, "pass" if len(node.body) == 1 and not isinstance(node, ast.Module) else ""))
    return spans


def shell_spans(s):
    spans = []
    i, n = 0, len(s)
    heredocs = []
    line_start = True
    while i < n:
        c = s[i]
        if c == "\n":
            i += 1
            line_start = True
            while heredocs:
                tag, strip = heredocs.pop(0)
                while i < n:
                    j = s.find("\n", i)
                    j = n if j < 0 else j
                    line = s[i:j]
                    i = min(j + 1, n)
                    if (line.lstrip("\t") if strip else line) == tag:
                        break
            continue
        if c == "\\":
            i += 2
            continue
        if c == "'":
            j = s.find("'", i + 1)
            i = n if j < 0 else j + 1
            continue
        if c == '"':
            j = i + 1
            while j < n and s[j] != '"':
                j += 2 if s[j] == "\\" else 1
            i = j + 1
            continue
        if c == "<" and s.startswith("<<", i) and not s.startswith("<<<", i):
            m = re.match(r"<<(-?)\s*(['\"]?)([A-Za-z_][A-Za-z0-9_]*)\2", s[i:])
            if m:
                heredocs.append((m.group(3), bool(m.group(1))))
                i += len(m.group(0))
                continue
        if c == "#":
            prev = s[i - 1] if i else "\n"
            if (prev in " \t\n;&|()" ) and not (i == 0 and s.startswith("#!")) and not s[max(0, i - 2):i] == "${":
                j = s.find("\n", i)
                j = n if j < 0 else j
                spans.append((i, j))
                i = j
                continue
        i += 1
    return spans


def powershell_spans(s):
    spans = []
    i, n = 0, len(s)
    while i < n:
        c = s[i]
        if s.startswith("<#", i):
            j = s.find("#>", i + 2)
            j = n if j < 0 else j + 2
            spans.append((i, j))
            i = j
        elif s.startswith('@"', i) or s.startswith("@'", i):
            q = s[i + 1]
            m = re.compile("\n" + q + "@").search(s, i + 2)
            i = n if not m else m.end()
        elif c == "`":
            i += 2
        elif c == "'":
            j = i + 1
            while j < n:
                if s[j] == "'":
                    if j + 1 < n and s[j + 1] == "'":
                        j += 2
                        continue
                    break
                j += 1
            i = j + 1
        elif c == '"':
            j = i + 1
            while j < n and s[j] != '"':
                j += 2 if s[j] == "`" else 1
            i = j + 1
        elif c == "#" and (i == 0 or s[i - 1] in " \t\n;({}"):
            j = s.find("\n", i)
            j = n if j < 0 else j
            spans.append((i, j))
            i = j
        else:
            i += 1
    return spans


def inno_spans(s):
    spans = []
    starts = line_starts(s)
    lines = s.split("\n")
    in_code = False
    depth_kind = None
    for no, line in enumerate(lines):
        base = starts[no]
        stripped = line.lstrip("\ufeff \t")
        if depth_kind is None and re.match(r"\[[A-Za-z]+\]\s*$", stripped):
            in_code = stripped.lower().startswith("[code]")
            continue
        if not in_code:
            if stripped.startswith(";") or stripped.startswith("//"):
                spans.append((base + len(line) - len(stripped), base + len(line)))
            continue
        if depth_kind is None and stripped.startswith("#"):
            continue
        i, n = 0, len(line)
        open_at = 0 if depth_kind else None
        while i < n:
            if depth_kind == "{":
                j = line.find("}", i)
                if j < 0:
                    i = n
                    break
                spans.append((base + open_at, base + j + 1))
                depth_kind, i = None, j + 1
                continue
            if depth_kind == "(*":
                j = line.find("*)", i)
                if j < 0:
                    i = n
                    break
                spans.append((base + open_at, base + j + 2))
                depth_kind, i = None, j + 2
                continue
            c = line[i]
            if c == "'":
                j = line.find("'", i + 1)
                i = n if j < 0 else j + 1
            elif line.startswith("//", i):
                spans.append((base + i, base + n))
                i = n
            elif c == "{" and not line.startswith("{#", i):
                depth_kind, open_at, i = "{", i, i + 1
            elif line.startswith("(*", i):
                depth_kind, open_at, i = "(*", i, i + 2
            else:
                i += 1
        if depth_kind:
            spans.append((base + (open_at or 0), base + n))
    merged = []
    for a, b in sorted(spans):
        if merged and a <= merged[-1][1] + 1 and s[merged[-1][1]:a] in ("", "\n"):
            merged[-1] = (merged[-1][0], b)
        else:
            merged.append((a, b))
    return merged


def vbs_spans(s):
    spans = []
    starts = line_starts(s)
    for no, line in enumerate(s.split("\n")):
        base = starts[no]
        i, n = 0, len(line)
        if re.match(r"\s*rem(\s|$)", line, re.I):
            k = len(line) - len(line.lstrip())
            spans.append((base + k, base + n))
            continue
        while i < n:
            if line[i] == '"':
                j = i + 1
                while j < n:
                    if line[j] == '"':
                        if j + 1 < n and line[j + 1] == '"':
                            j += 2
                            continue
                        break
                    j += 1
                i = j + 1
            elif line[i] == "'":
                spans.append((base + i, base + n))
                break
            else:
                i += 1
    return spans


def hash_scan_line(line, quotes):
    i, n = 0, len(line)
    while i < n:
        c = line[i]
        if c in quotes:
            j = i + 1
            while j < n and line[j] != c:
                j += 2 if (c == '"' and line[j] == "\\") else 1
            i = j + 1
        elif c == "#" and (i == 0 or line[i - 1] in " \t"):
            return i
        else:
            i += 1
    return None


def yaml_spans(s):
    spans = []
    starts = line_starts(s)
    block_indent = None
    for no, line in enumerate(s.split("\n")):
        base = starts[no]
        indent = len(line) - len(line.lstrip(" "))
        body = line.strip()
        if block_indent is not None:
            if body == "" or indent > block_indent:
                if body.startswith("#"):
                    spans.append((base + indent, base + len(line)))
                continue
            block_indent = None
        k = hash_scan_line(line, "'\"")
        if k is not None:
            spans.append((base + k, base + len(line)))
            code = line[:k].rstrip()
        else:
            code = line.rstrip()
        if re.search(r":\s*[|>][-+0-9]*$", code) or re.match(r"\s*-\s*[|>][-+0-9]*$", code):
            block_indent = indent
    return spans


def toml_spans(s):
    spans = []
    starts = line_starts(s)
    multi = None
    for no, line in enumerate(s.split("\n")):
        base = starts[no]
        i, n = 0, len(line)
        while i < n:
            if multi:
                j = line.find(multi, i)
                if j < 0:
                    i = n
                    break
                i, multi = j + 3, None
                continue
            if line.startswith('"""', i) or line.startswith("'''", i):
                multi, i = line[i:i + 3], i + 3
            elif line[i] == '"':
                j = i + 1
                while j < n and line[j] != '"':
                    j += 2 if line[j] == "\\" else 1
                i = j + 1
            elif line[i] == "'":
                j = line.find("'", i + 1)
                i = n if j < 0 else j + 1
            elif line[i] == "#":
                spans.append((base + i, base + n))
                break
            else:
                i += 1
    return spans


def hashline_spans(s):
    spans = []
    starts = line_starts(s)
    for no, line in enumerate(s.split("\n")):
        k = len(line) - len(line.lstrip())
        if line[k:].startswith("#"):
            spans.append((starts[no] + k, starts[no] + len(line)))
    return spans


SCANNERS = {"rust": rust_spans, "python": python_spans, "shell": shell_spans, "powershell": powershell_spans, "inno": inno_spans, "vbs": vbs_spans, "yaml": yaml_spans, "toml": toml_spans, "hashline": hashline_spans}


def strip_removed_lines(s, spans):
    marker = "\x00"
    out = s
    for sp in sorted(spans, key=lambda x: x[0], reverse=True):
        out = out[:sp[0]] + (sp[2] if len(sp) > 2 else marker) + out[sp[1]:]
    res = []
    for line in out.split("\n"):
        if marker in line:
            line = line.replace(marker, "").rstrip()
            if line.strip() == "":
                continue
        if line == "" and res and res[-1] == "":
            continue
        res.append(line)
    return "\n".join(res)


def tracked():
    out = subprocess.run(["git", "ls-files"], capture_output=True, text=True, check=True).stdout
    return [p for p in out.splitlines() if kind_of(p)]


def main():
    args = sys.argv[1:]
    fix = "--strip" in args
    paths = [a for a in args if not a.startswith("--")] or tracked()
    found = 0
    for p in paths:
        k = kind_of(p)
        if not k:
            continue
        raw = Path(p).read_bytes()
        bom = raw.startswith(b"\xef\xbb\xbf")
        s = raw.decode("utf-8-sig")
        crlf = "\r\n" in s
        if crlf:
            s = s.replace("\r\n", "\n")
        spans = SCANNERS[k](s)
        if not spans:
            continue
        if fix:
            new = strip_removed_lines(s, spans)
            if crlf:
                new = new.replace("\n", "\r\n")
            Path(p).write_bytes((b"\xef\xbb\xbf" if bom else b"") + new.encode("utf-8"))
            print(f"{p}: removed {len(spans)}")
        else:
            starts = line_starts(s)
            for sp in spans:
                ln = bisect.bisect_right(starts, sp[0]) - 1
                print(f"{p}:{ln + 1}: {s[sp[0]:sp[1]].splitlines()[0][:80] if s[sp[0]:sp[1]] else ''}")
        found += len(spans)
    if not fix and found:
        print(f"{found} comment(s) found")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
