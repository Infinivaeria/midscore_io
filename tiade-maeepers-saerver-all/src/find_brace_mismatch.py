#!/usr/bin/env python3
import re, sys

if len(sys.argv) < 2:
    print("Usage: find_brace_mismatch.py <file>")
    sys.exit(2)

path = sys.argv[1]
text = open(path, "r", encoding="utf-8").read()

# Remove block comments first
text = re.sub(r'(?s)/\*.*?\*/', '', text)

lines = text.splitlines()

# regex to remove normal and raw string literals
str_re = re.compile(r'''
    r(#*)"(?:[^"\\]|\\.|"(?!"\1))*"\1   # raw strings r#"..."# or r##"..."## etc
  | "(?:\\.|[^"\\])*"                   # double quoted
  | '(?:\\.|[^'\\])*'                   # single quoted
''', re.VERBOSE)

brace = 0
paren = 0

for i, line in enumerate(lines, start=1):
    cleaned = str_re.sub("", line)          # strip strings
    cleaned = re.sub(r'//.*', '', cleaned)  # strip line comments

    # count braces and parens
    opens_b = cleaned.count("{")
    closes_b = cleaned.count("}")
    opens_p = cleaned.count("(")
    closes_p = cleaned.count(")")

    brace += opens_b - closes_b
    paren += opens_p - closes_p

    if brace < 0:
        print(f"UNMATCHED CLOSING '}}' at line {i}")
        print(f"{i:5}: {line.rstrip()}")
        sys.exit(1)
    if paren < 0:
        print(f"UNMATCHED CLOSING ')' at line {i}")
        print(f"{i:5}: {line.rstrip()}")
        sys.exit(1)

# end of file
if brace > 0 or paren > 0:
    print("FILE ENDED WITH UNMATCHED OPENS:")
    if brace > 0:
        print(f"  Unmatched '{{' count: {brace}")
    if paren > 0:
        print(f"  Unmatched '(' count: {paren}")
    print("\nLast 80 lines for context:")
    start = max(0, len(lines)-80)
    for ln, l in enumerate(lines[start:], start=start+1):
        print(f"{ln:5}: {l.rstrip()}")
    sys.exit(2)

print("Braces and parentheses appear balanced.")
sys.exit(0)
