#!/usr/bin/env bash
set -euo pipefail

URL="${URL:-http://localhost:9999/fraud-score}"
REQUESTS="${REQUESTS:-1000}"
CONCURRENCY="${CONCURRENCY:-50}"
TIMEOUT_SECONDS="${TIMEOUT_SECONDS:-3}"

TMP_DIR="$(mktemp -d)"
RESULTS_FILE="$TMP_DIR/results.tsv"
PIDS=()

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

payload_for() {
  local id="$1"
  local amount=$((10 + (id % 5000)))
  local installments=$((1 + (id % 12)))
  local tx_count=$((id % 30))
  local km_home=$((id % 1200))
  local km_current=$((id % 300))
  local online="true"
  local card_present="false"

  if (( id % 2 == 0 )); then
    online="false"
    card_present="true"
  fi

  cat <<JSON
{
  "id": "load-test-$id",
  "transaction": {
    "amount": $amount.50,
    "installments": $installments,
    "requested_at": "2026-05-08T20:30:00Z"
  },
  "customer": {
    "avg_amount": 750.00,
    "tx_count_24h": $tx_count,
    "known_merchants": ["merchant-1", "merchant-2"]
  },
  "merchant": {
    "id": "merchant-$((id % 20))",
    "mcc": "5411",
    "avg_amount": 600.00
  },
  "terminal": {
    "is_online": $online,
    "card_present": $card_present,
    "km_from_home": $km_home.0
  },
  "last_transaction": {
    "timestamp": "2026-05-08T19:45:00Z",
    "km_from_current": $km_current.0
  }
}
JSON
}

request_once() {
  local id="$1"
  local result

  result="$(
    payload_for "$id" | curl \
      --silent \
      --output /dev/null \
      --write-out "%{http_code}\t%{time_total}" \
      --max-time "$TIMEOUT_SECONDS" \
      --header "Content-Type: application/json" \
      --data-binary @- \
      "$URL" || printf "000\t%s" "$TIMEOUT_SECONDS"
  )"

  printf "%s\t%s\n" "$id" "$result" >> "$RESULTS_FILE"
}

worker() {
  local worker_id="$1"
  local id="$worker_id"

  while (( id <= REQUESTS )); do
    request_once "$id"
    id=$((id + CONCURRENCY))
  done
}

printf "Load test target: %s\n" "$URL"
printf "Requests: %s | Concurrency: %s | Timeout: %ss\n" "$REQUESTS" "$CONCURRENCY" "$TIMEOUT_SECONDS"

start_epoch="$(date +%s)"

for worker_id in $(seq 1 "$CONCURRENCY"); do
  worker "$worker_id" &
  PIDS+=("$!")
done

for pid in "${PIDS[@]}"; do
  wait "$pid"
done

end_epoch="$(date +%s)"
elapsed=$((end_epoch - start_epoch))
LATENCIES_FILE="$TMP_DIR/latencies.txt"

awk -F '\t' '{ printf "%.6f\n", $3 * 1000 }' "$RESULTS_FILE" | sort -n > "$LATENCIES_FILE"

percentile() {
  local percent="$1"
  local count="$2"
  local index

  index="$(awk -v count="$count" -v percent="$percent" 'BEGIN { value = int(count * percent); if (value < 1) value = 1; print value }')"
  sed -n "${index}p" "$LATENCIES_FILE"
}

total="$(wc -l < "$RESULTS_FILE" | tr -d ' ')"

if [[ "$total" == "0" ]]; then
  printf "No results\n"
  exit 1
fi

p50="$(percentile "0.50" "$total")"
p95="$(percentile "0.95" "$total")"
p99="$(percentile "0.99" "$total")"

awk -F '\t' \
  -v elapsed="$elapsed" \
  -v p50="$p50" \
  -v p95="$p95" \
  -v p99="$p99" '
  {
    count += 1
    status[$2] += 1
    if ($2 < 200 || $2 >= 300) errors += 1
  }
  END {
    printf "\nResults\n"
    printf "Total: %d\n", count
    printf "Errors: %d (%.2f%%)\n", errors, errors * 100 / count
    printf "Elapsed: %ds\n", elapsed
    if (elapsed > 0) printf "Throughput: %.2f req/s\n", count / elapsed
    printf "p50: %.2fms\n", p50
    printf "p95: %.2fms\n", p95
    printf "p99: %.2fms\n", p99
    printf "\nHTTP status counts\n"
    for (code in status) printf "%s: %d\n", code, status[code]
  }
' "$RESULTS_FILE"
