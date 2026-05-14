#!/usr/bin/env python3
"""Generate all test data CSV files under normalized_tests/."""

import gzip
import os

BASE = os.path.join(os.path.dirname(__file__), "normalized_tests")
os.makedirs(BASE, exist_ok=True)


def write(name, content, mode="w"):
    path = os.path.join(BASE, name)
    with open(path, mode, newline="") if mode == "w" else open(path, mode) as f:
        f.write(content)
    print(f"  {name}")


def write_bytes(name, content):
    path = os.path.join(BASE, name)
    with open(path, "wb") as f:
        f.write(content)
    print(f"  {name}")


# ── 1. Basic ──────────────────────────────────────────────────────────
# test.csv: no header, 5000 rows "i,i, test"
rows = "\n".join(f"{i},{i}, test" for i in range(5000))
write("test.csv", rows + "\n")

# test_default.csv: "0" as header → 4999 rows
rows = "0\n" + "\n".join(str(i) for i in range(1, 5000))
write("test_default.csv", rows + "\n")

# test_pipe.csv: pipe-delimited, no header, 10 rows, 3 columns
rows = "\n".join(f"{i}|{i * 10}|{i * 100}" for i in range(1, 11))
write("test_pipe.csv", rows + "\n")

# ── 2. Date / timestamp ───────────────────────────────────────────────
write("date.csv", "2019-06-05\n")
write("dateformat.csv", "05/06/2019\n")
write("dateformat_2.csv", "2019-06-05\n")
write("timestampformat.csv", "Mon 30, June 2003, 12:03:10 PM\n")

# timestampoffset.csv: 4 timestamp lines (no header row)
# First line could be detected as header, so all 4 lines are timestamps
write(
    "timestampoffset.csv",
    "2023-01-01 12:00:00+00:00\n2023-01-02 12:00:00+01:00\n2023-01-03 12:00:00-05:00\n2023-01-04 12:00:00+00:00\n",
)

# ── 3. Null / empty value handling ────────────────────────────────────
# test_null_csv.csv: pipe-delimited, "0||test" → 1 row, column 1 empty
write("test_null_csv.csv", "0||test\n")

# test_null_option.csv: 3 rows with empty values
write("test_null_option.csv", "a,\nb,\nc,\n")

# force_not_null.csv: 3 rows
write("force_not_null.csv", "i,j\n1,2\n3,4\n5,6\n")

# force_not_null_inull.csv: 3 rows with "NULL" string
write("force_not_null_inull.csv", "i,j\n1,NULL\n3,4\nNULL,6\n")

# force_not_null_reordered.csv: 3 rows
write("force_not_null_reordered.csv", "i,j\n1,2\n3,4\n5,6\n")

# force_quote.csv: 3 rows with quotes
write("force_quote.csv", 'i,j\n1,"hello"\n3,"world"\n5,"foo"\n')

# ── 4. Error / edge-case datasets ────────────────────────────────────
# error_invalid_type.csv: Header "i,j" + 6 data rows (row 5 has "a")
write("error_invalid_type.csv", "i,j\n1,2\n3,4\n5,6\n7,8\n9,a\n10,11\n")

# error_too_little.csv: Header "i,j" + 6 data rows (some with fewer columns)
write("error_too_little.csv", "i,j\n1,2\n3\n5,6\n7,8\n9,10\n11,12\n")

# error_too_little_single.csv: Header "i,j" + 1 data row with single value "7"
write("error_too_little_single.csv", "i,j\n7\n")

# error_too_many.csv: Header "i,j" + 6 data rows (one row has 3 columns)
write("error_too_many.csv", "i,j\n1,2\n3,4\n5,6,7\n8,9\n10,11\n12,13\n")

# error_too_little_end_of_filled_chunk.csv: Header "i,j" + 1025 data rows
rows = "i,j\n" + "\n".join(f"{i},{i}" for i in range(1025))
write("error_too_little_end_of_filled_chunk.csv", rows + "\n")

# too_many_values.csv: Single row "1,2,3,4" — no header
write("too_many_values.csv", "1,2,3,4\n")

# ── 5. Newline / line-ending variants ─────────────────────────────────
# quoted_newline.csv: quoted fields with newlines → 2 data rows
write("quoted_newline.csv", '1,"multi\nline",2\nd,e,f\n')

# mixed_line_endings.csv: mixed \n and \r\n → 3 data rows
write(
    "mixed_line_endings.csv",
    "a,b\nc,d\ne,f\n",
    mode="wb" if os.name != "posix" else "w",
)
# More careful: write with explicit bytes to ensure mixed endings
with open(os.path.join(BASE, "mixed_line_endings.csv"), "wb") as f:
    f.write(b"a,b\nc,d\r\ne,f\n")

# windows_newline.csv: Large \r\n file, no header → 20000 rows
with open(os.path.join(BASE, "windows_newline.csv"), "wb") as f:
    for i in range(20000):
        f.write(f"{i},{i}, test\r\n".encode())

# windows_newline_empty.csv: First line "1\r\n", rest empty → 1 row
with open(os.path.join(BASE, "windows_newline_empty.csv"), "wb") as f:
    f.write(b"1\r\n\r\n\r\n")

# ── 6. Pipe-delimited files ─────────────────────────────────────────
# new_line_string.csv: pipe-delimited, multiline quoted field.
# 1|6370|371|p1 detected as header → 2 data rows, 4 columns
write(
    "new_line_string.csv", '1|6370|371|p1\n2|1234|"multi\nline"|p2\n3|5678|simple|p3\n'
)

# new_line_string_rn.csv: \r\n version
with open(os.path.join(BASE, "new_line_string_rn.csv"), "wb") as f:
    f.write(b'1|6370|371|p1\r\n2|1234|"multi\r\nline"|p2\r\n3|5678|simple|p3\r\n')

# new_line_string_rn_exc.csv: another variant
with open(os.path.join(BASE, "new_line_string_rn_exc.csv"), "wb") as f:
    f.write(b'1|6370|371|p1\r\n2|1234|"exc\r\nline"|p2\r\n3|5678|simple|p3\r\n')

# multi_column_integer.csv: pipe-delimited, 8 rows, 3 columns
rows = "\n".join(f"{i}|{i * 10}|{i * 100}" for i in range(1, 9))
write("multi_column_integer.csv", rows + "\n")

# multi_column_integer_rn.csv: \r\n version
with open(os.path.join(BASE, "multi_column_integer_rn.csv"), "wb") as f:
    for i in range(1, 9):
        f.write(f"{i}|{i * 10}|{i * 100}\r\n".encode())

# multi_column_string.csv: pipe-delimited, 8 rows, 4 columns
rows = "\n".join(f"{i}|{i * 10}|val_{i}|str{i}" for i in range(1, 9))
write("multi_column_string.csv", rows + "\n")

# multi_column_string_rn.csv: \r\n version
with open(os.path.join(BASE, "multi_column_string_rn.csv"), "wb") as f:
    for i in range(1, 9):
        f.write(f"{i}|{i * 10}|val_{i}|str{i}\r\n".encode())

# long_escaped_value.csv: 1 row, 1 column, very long value (>29000 chars)
# No pipes, no commas → comma default → 1 column
long_val = "A" * 30000
write("long_escaped_value.csv", long_val + "\n")

# long_escaped_value_unicode.csv: 2 commas → 3 columns, 1 row
write("long_escaped_value_unicode.csv", f"a,{long_val},b\n")

# ── 7. Large / edge-case files ────────────────────────────────────────
# many_empty_lines.csv: "1\n\n\n..." → csv crate skips empty lines → 1 row
write("many_empty_lines.csv", "1\n\n\n\n\n\n\n\n\n\n")

# no_newline.csv: 1024 rows, last line without newline
content = "\n".join(f"{i},{i}, test" for i in range(1024))
with open(os.path.join(BASE, "no_newline.csv"), "w", newline="") as f:
    f.write(content)  # no trailing newline

# no_newline_unicode.csv: 1024 rows, unicode values, no trailing newline
content = "\n".join(f"{i},val_{i},\u00e9\u00e0\u00fc" for i in range(1024))
with open(os.path.join(BASE, "no_newline_unicode.csv"), "w", newline="") as f:
    f.write(content)

# vsize.csv: 1024 data rows
rows = "\n".join(f"{i},{i}, test" for i in range(1024))
write("vsize.csv", rows + "\n")

# issue2518.csv: 10 rows with quoted commas, column 0 is Int64
# Expected col0: ["4690", "5", "6", "7", "8", "9", "10", "1090", "11", "1184"]
# col4: "A,C,T" preserved
write(
    "issue2518.csv",
    '4690,2,3,4,"A,C,T",6\n'
    '5,2,3,4,"A,C,T",6\n'
    '6,2,3,4,"A,C,T",6\n'
    '7,2,3,4,"A,C,T",6\n'
    '8,2,3,4,"A,C,T",6\n'
    '9,2,3,4,"A,C,T",6\n'
    '10,2,3,4,"A,C,T",6\n'
    '1090,2,3,4,"A,C,T",6\n'
    '11,2,3,4,"A,C,T",6\n'
    '1184,2,3,4,"A,C,T",6\n',
)

# struct_padding.csv: 15 rows of struct-like strings
rows = "\n".join(f"struct_{i},value_{i}" for i in range(15))
write("struct_padding.csv", rows + "\n")

# test_long_line.csv: 2 rows with very long lines (30000+ chars)
write("test_long_line.csv", "A" * 30000 + "\n" + "B" * 30000 + "\n")

# ── 8. Big header (tab-delimited) ───────────────────────────────────
# Tab-delimited, 4 header columns, 5 data rows (incl. "----" row)
write(
    "big_header.csv",
    "foo\tbar\tbaz\tbam\n1\t2\t3\t4\n----\t----\t----\t----\n5\t6\t7\t8\n9\t10\t11\t12\n13\t14\t15\t16\n",
)

# ── 9. Compressed files ──────────────────────────────────────────────
# test_comp.csv.gz: raw gzip of a small CSV
raw = b"a,b\n1,2\n3,4\n"
with gzip.open(os.path.join(BASE, "test_comp.csv.gz"), "wb") as f:
    f.write(raw)

# bgzf.gz: small BGZF-like content (just gzip with extra fields)
with gzip.open(os.path.join(BASE, "bgzf.gz"), "wb") as f:
    f.write(b"1,2\n3,4\n")

# ── 10. Empty file ───────────────────────────────────────────────────
write("empty.csv", "")

# ── 11. Unicode normalization ───────────────────────────────────────
# "ü" (NFC) on line 1, "ü" (NFD) on line 2 → header detected as "ü" → 1 data row
nfc_ue = "\u00fc"  # NFC: ü (single codepoint)
nfd_ue = "u\u0308"  # NFD: ü (u + combining diaeresis)
# NFC file: "ü" (NFC) detected as header → 1 data row "ü some data"
write("nfc.csv", f"{nfc_ue}\n{nfd_ue} some data\n")

# ── 12. NDJSON-like content ──────────────────────────────────────────
write("5438.csv", '{"duck": 1}\n{"duck": 2}\n')


# ── 13. Invalid UTF-8 ────────────────────────────────────────────────
def write_raw(name, raw_bytes):
    write_bytes(name, raw_bytes)


# invalid_utf.csv: simple invalid UTF-8
write_raw("invalid_utf.csv", b"a,b\n1,\xff\xff\n")

# invalid_utf_header.csv: invalid UTF-8 in header
write_raw("invalid_utf_header.csv", b"a\xff,b\n1,2\n3,4\n")

# invalid_utf_quoted.csv: invalid UTF-8 in quoted field
write_raw("invalid_utf_quoted.csv", b'a,"\xff\xff",c\n1,2,3\n')

# invalid_utf_quoted_nl.csv: invalid UTF-8 in quoted field with newline
write_raw("invalid_utf_quoted_nl.csv", b'a,"\xff\n\xff",c\n1,2,3\n')

# invalid_utf_big.csv: Large file ~54014 valid bytes + invalid bytes → 3030 valid rows
# Build rows 0-3029: "i,i, test\n" each
# Then pure corruption (no extra valid rows after corruption)
lines = []
for i in range(3030):
    lines.append(f"{i},{i}, test")
valid = "\n".join(lines).encode() + b"\n"
# Pure corruption bytes after the valid section
corrupt = b"\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff"
write_raw("invalid_utf_big.csv", valid + corrupt)

# invalid_utf_list.csv: "[1, 2]" with \xff\xff bytes
write_raw("invalid_utf_list.csv", b"[1, 2]\xff\xff\n")

# ── 14. Unterminated quoted field ────────────────────────────────────
write("unterminated.csv", '"12345')

# ── 15. Blob / escaped byte data ─────────────────────────────────────
write("blob.csv", r"\x00\x01\x02\x03\n")

# ── 16. Incompatible type with nullable ──────────────────────────────
write("test_incompatible_type_with_nullable.csv", "i,j\n1,\n2,hello\n")

# ── 17. Nested JSON objects (heterogeneous types) ────────────────────
# nested_objects.json: array of objects whose "nested" value is itself an
# object mixing int, string, bool, float, null and a deeper nested object
# with an array. Exercises the unsupported nested-type path.
import json

nested_objects = [
    {
        "id": 1,
        "nested": {
            "int_field": 42,
            "str_field": "text",
            "bool_field": True,
            "float_field": 3.14,
            "null_field": None,
            "deeper": {"list_field": [1, 2, 3], "tag": "x"},
        },
    },
    {
        "id": 2,
        "nested": {
            "int_field": -7,
            "str_field": "other",
            "bool_field": False,
            "float_field": 2.71,
            "null_field": None,
            "deeper": {"list_field": [4, 5], "tag": "y"},
        },
    },
]
write("nested_objects.json", json.dumps(nested_objects, indent=2) + "\n")

print("Done generating all test data files.")
