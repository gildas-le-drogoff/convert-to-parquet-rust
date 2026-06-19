#!/usr/bin/env bash
set -euo pipefail

DELAY=0.02
PAUSE=1.0

type_text() {
  local t="$1"
  for ((i = 0; i < ${#t}; i++)); do
    printf '%s' "${t:i:1}"
    sleep "$DELAY"
  done
  sleep 0.2
  printf '\n'
}

heading() {
  printf '\n\033[1;36m── %s ──\033[0m\n' "$1"
  sleep "$PAUSE"
}

run_cmd() {
  printf '\033[1;32m$ \033[0;33m'
  type_text "$1"
  printf '\033[0m'
  eval "$1"
  sleep "$PAUSE"
}

WORKDIR=$(mktemp -d /tmp/ctp_demo_XXXX)
trap 'rm -rf "$WORKDIR"' EXIT
CSV="$WORKDIR/sales.csv"
TSV="$WORKDIR/logs.tsv"
NO_HEADER="$WORKDIR/sensors.csv"
BIN="cargo run --release --quiet --bin convert_to_parquet --"

# --- Intro ---

clear
printf '\033[1;35m'
cat << 'EOF'

   ██████╗  ██████╗ ███╗   ██╗██╗   ██╗███████╗██████╗ ████████╗
  ██╔════╝ ██╔═══██╗████╗  ██║██║   ██║██╔════╝██╔══██╗╚══██╔══╝
  ██║      ██║   ██║██╔██╗ ██║██║   ██║█████╗  ██████╔╝   ██║
  ██║      ██║   ██║██║╚██╗██║╚██╗ ██╔╝██╔══╝  ██╔══██╗   ██║
  ╚██████╗ ╚██████╔╝██║ ╚████║ ╚████╔╝ ███████╗██║  ██║   ██║
   ╚═════╝  ╚═════╝ ╚═╝  ╚═══╝  ╚═══╝  ╚══════╝╚═╝  ╚═╝   ╚═╝
                  ──── to ────
  ██████╗  █████╗ ██████╗  ██████╗ ██╗   ██╗███████╗████████╗
  ██╔══██╗██╔══██╗██╔══██╗██╔═══██╗██║   ██║██╔════╝╚══██╔══╝
  ██████╔╝███████║██████╔╝██║   ██║██║   ██║█████╗     ██║
  ██╔═══╝ ██╔══██║██╔══██╗██║▄▄ ██║██║   ██║██╔══╝     ██║
  ██║     ██║  ██║██║  ██║╚██████╔╝╚██████╔╝███████╗   ██║
  ╚═╝     ╚═╝  ╚═╝╚═╝  ╚═╝ ╚══▀▀═╝ ╚═════╝ ╚══════╝   ╚═╝
                ━━━  R u s t  ━━━

EOF
printf '\033[0m'
printf '  Rust • Arrow • ZSTD • Parallel\n\n'
sleep 2

# --- 1. Generate files ---

heading "1. Generate demo files"

cat > "$CSV" << 'CSVEOF'
id,product,price,quantity,in_stock,sale_date,created_at
1,Keyboard,49.99,120,true,2024-01-15,2024-01-15T09:30:00Z
2,Mouse,29.50,85,true,2024-02-20,2024-02-20T14:15:30Z
3,Monitor 27",349.00,12,false,2024-03-01,2024-03-01T11:00:00Z
4,USB Cable,8.99,500,true,2024-03-10,2024-03-10T08:45:12Z
5,Headset,79.90,NULL,true,2024-04-05,2024-04-05T16:20:00Z
6,HD Webcam,45.00,30,false,2024-04-12,N/A
7,USB-C Hub,34.99,200,true,2024-05-01,2024-05-01T10:00:00Z
8,Mouse Pad,12.50,0,true,2024-05-15,2024-05-15T13:30:45Z
9,Laptop Stand,59.00,15,false,2024-06-01,2024-06-01T09:00:00Z
10,Dock Station,189.99,8,true,2024-06-20,2024-06-20T17:45:00Z
CSVEOF

printf -- '  ✓ CSV   %s  (mixed types, NULLs, timestamps)\n' "$CSV"

cat > "$TSV" << 'TSVEOF'
timestamp level service message latency_ms  status_code
2024-06-01 00:00:01.123 INFO  api-gateway Request received  12  200
2024-06-01 00:00:01.456 DEBUG auth-svc  Token validated 3 200
2024-06-01 00:00:02.001 WARN  api-gateway Slow upstream 1520  200
2024-06-01 00:00:02.500 ERROR payment-svc Timeout 5000  504
2024-06-01 00:00:03.100 INFO  api-gateway Request received  8 200
TSVEOF

printf -- '  ✓ TSV   %s  (tab-delimited, fractional timestamps)\n' "$TSV"

python3 -c "
import random, datetime
random.seed(42)
base = datetime.datetime(2024, 6, 1)
for i in range(500):
    ts = base + datetime.timedelta(seconds=i*30)
    t = round(20 + random.gauss(0, 3), 2)
    h = round(55 + random.gauss(0, 8), 1)
    p = round(1013 + random.gauss(0, 5), 1)
    print(f'{ts:%Y-%m-%d %H:%M:%S},{t},{h},{p}')
" > "$NO_HEADER"

printf -- '  ✓ NoHdr %s  (500 rows, no header)\n' "$NO_HEADER"
sleep "$PAUSE"

# --- 2. CSV conversion + size ---

heading "2. Convert CSV → Parquet"

run_cmd "$BIN $CSV"

run_cmd "ls -lh $CSV ${CSV%.csv}.parquet"

# --- 3. TSV ---

heading "3. Convert TSV (auto-detected tab delimiter)"

run_cmd "$BIN $TSV"

# --- 4. No header ---

heading "4. Headerless file (auto-detected)"

run_cmd "head -3 $NO_HEADER"

run_cmd "$BIN $NO_HEADER"

# --- 5. Schema inspection ---

heading "5. Inspect Parquet schema"

run_cmd "$BIN --view-schema ${CSV%.csv}.parquet"

# --- 6. force-utf8 + stdin ---

heading "6. --force-utf8 & stdin pipe"

run_cmd "cat $CSV | $BIN --force-utf8 -"

run_cmd "ls -lh stdin.parquet"

rm -f stdin.parquet

# --- 7. Tests ---

heading "7. Test suite"

run_cmd "cargo test 2>&1 | tail -20"

# --- Done ---

printf '\n\033[1;35m  ━━━ Demo complete ━━━\033[0m\n\n'
