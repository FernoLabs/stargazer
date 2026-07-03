#!/usr/bin/env python3
import csv
import datetime as dt
import json
import math
import os
import statistics
import time
import urllib.request
from pathlib import Path

POOL = "2gdjgchz31VdaAxmERiAbTAJbzAtGBei7LAwAMV45P1M"
LBTC = "LBTCgU4b3wsFKsPwBn1rRZDx5DoFutM6RPiEt1TPDsY"
CBBTC = "cbbtcf3aa214zXHbiAZQwf4122FBYbraNdFqgw4iMij"
TARGET_SLOT = 421_651_686
MAX_PAGES = int(os.environ.get("LBTC_HISTORY_MAX_PAGES", "45"))
BATCH_SIZE = int(os.environ.get("LBTC_HISTORY_BATCH_SIZE", "20"))
OUT_DIR = Path(os.environ.get("LBTC_HISTORY_OUT_DIR", "backend/data/lbtc_liquidity"))
CSV_PATH = OUT_DIR / "lbtc_meteora_dlmm_reserves.csv"
SVG_PATH = OUT_DIR / "lbtc_meteora_dlmm_reserves.svg"
SUMMARY_PATH = OUT_DIR / "lbtc_meteora_dlmm_summary.json"


def load_backend_env(path="backend/.env"):
    if not Path(path).exists():
        return
    for raw_line in Path(path).read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        os.environ.setdefault(key, value)


def request_json(url, payload=None, timeout=60):
    headers = {
        "content-type": "application/json",
        "user-agent": "ferno-firewatch-lbtc-liquidity-investigation/1.0",
    }
    data = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(url, data=data, headers=headers)
    with urllib.request.urlopen(req, timeout=timeout) as res:
        return json.load(res)


def rpc_url():
    endpoint = os.environ["SOLANA_GRPC_ENDPOINT"].rstrip("/")
    token = os.environ["SOLANA_GRPC_X_TOKEN"]
    return f"{endpoint}/{token}"


def rpc(payload, timeout=60):
    return request_json(rpc_url(), payload=payload, timeout=timeout)


def fetch_market():
    url = f"https://api.dexscreener.com/token-pairs/v1/solana/{LBTC}"
    pairs = request_json(url, timeout=20)
    pair = next(pair for pair in pairs if pair.get("pairAddress") == POOL)
    price_usd = float(pair["priceUsd"])
    price_native = float(pair["priceNative"])
    return {
        "dex_id": pair["dexId"],
        "url": pair["url"],
        "price_usd": price_usd,
        "price_native": price_native,
        "quote_price_usd": price_usd / price_native,
        "liquidity_usd": float(pair["liquidity"]["usd"]),
        "dex_lbtc_reserve": float(pair["liquidity"]["base"]),
        "dex_cbbtc_reserve": float(pair["liquidity"]["quote"]),
    }


def signatures():
    before = None
    out = []
    for page in range(1, MAX_PAGES + 1):
        config = {"commitment": "finalized", "limit": 1000}
        if before:
            config["before"] = before
        payload = {
            "jsonrpc": "2.0",
            "id": page,
            "method": "getSignaturesForAddress",
            "params": [POOL, config],
        }
        result = rpc(payload, timeout=30)
        items = result.get("result") or []
        if not items:
            break
        successful = [item for item in items if item.get("err") is None]
        out.extend(successful)
        before = items[-1]["signature"]
        print(
            f"page {page:02d}: signatures={len(items)} ok={len(successful)} "
            f"last_slot={items[-1].get('slot')}"
        )

    seen = set()
    deduped = []
    for item in out:
        signature = item["signature"]
        if signature in seen:
            continue
        seen.add(signature)
        deduped.append(signature)
    return deduped


def account_keys(transaction):
    keys = []
    for account in (
        transaction.get("transaction", {}).get("message", {}).get("accountKeys") or []
    ):
        if isinstance(account, str):
            keys.append(account)
        else:
            keys.append(account.get("pubkey"))
    return keys


def token_amount(ui_amount):
    if not ui_amount:
        return None
    if ui_amount.get("uiAmountString") is not None:
        return float(ui_amount["uiAmountString"])
    if ui_amount.get("amount") is not None and ui_amount.get("decimals") is not None:
        return float(ui_amount["amount"]) / (10 ** int(ui_amount["decimals"]))
    return None


def snapshot(transaction, signature, market):
    if not transaction or transaction.get("meta", {}).get("err") is not None:
        return None
    keys = account_keys(transaction)
    balances = {}
    for balance in transaction.get("meta", {}).get("postTokenBalances") or []:
        if balance.get("owner") != POOL:
            continue
        mint = balance.get("mint")
        if mint not in (LBTC, CBBTC):
            continue
        amount = token_amount(balance.get("uiTokenAmount"))
        if amount is None:
            continue
        account_index = balance.get("accountIndex")
        vault = (
            keys[account_index]
            if isinstance(account_index, int) and account_index < len(keys)
            else ""
        )
        current = balances.get(mint)
        if current is None or amount > current[0]:
            balances[mint] = (amount, vault)
    if LBTC not in balances or CBBTC not in balances:
        return None

    lbtc_reserve, lbtc_vault = balances[LBTC]
    cbbtc_reserve, cbbtc_vault = balances[CBBTC]
    return {
        "slot": int(transaction["slot"]),
        "block_time": int(transaction["blockTime"])
        if transaction.get("blockTime")
        else None,
        "signature": signature,
        "lbtc_reserve": lbtc_reserve,
        "cbbtc_reserve": cbbtc_reserve,
        "lbtc_vault": lbtc_vault,
        "cbbtc_vault": cbbtc_vault,
        "lbtc_reserve_usd_current_price": lbtc_reserve * market["price_usd"],
        "cbbtc_reserve_usd_current_price": cbbtc_reserve * market["quote_price_usd"],
        "estimated_liquidity_usd_current_price": lbtc_reserve * market["price_usd"]
        + cbbtc_reserve * market["quote_price_usd"],
    }


def fetch_snapshots(items, market):
    rows = []
    for start in range(0, len(items), BATCH_SIZE):
        batch = items[start : start + BATCH_SIZE]
        payload = [
            {
                "jsonrpc": "2.0",
                "id": index,
                "method": "getTransaction",
                "params": [
                    signature,
                    {
                        "encoding": "jsonParsed",
                        "commitment": "finalized",
                        "maxSupportedTransactionVersion": 0,
                    },
                ],
            }
            for index, signature in enumerate(batch)
        ]
        try:
            result = rpc(payload, timeout=90)
        except Exception as error:
            print(f"batch failed at {start}: {error}")
            time.sleep(1)
            continue
        by_id = {item.get("id"): item for item in result if isinstance(item, dict)}
        for index, signature in enumerate(batch):
            item = by_id.get(index, {})
            row = snapshot(item.get("result"), signature, market)
            if row:
                rows.append(row)
        if start % 500 == 0:
            print(f"fetched tx {start + len(batch)}/{len(items)} snapshots={len(rows)}")

    rows.sort(key=lambda row: (row["slot"], row["signature"]))
    return rows


def format_time(timestamp):
    if not timestamp:
        return ""
    return (
        dt.datetime.fromtimestamp(timestamp, dt.timezone.utc)
        .isoformat()
        .replace("+00:00", "Z")
    )


def write_csv(rows):
    fields = [
        "slot",
        "block_time",
        "block_time_iso",
        "signature",
        "lbtc_reserve",
        "cbbtc_reserve",
        "lbtc_reserve_usd_current_price",
        "cbbtc_reserve_usd_current_price",
        "estimated_liquidity_usd_current_price",
        "lbtc_vault",
        "cbbtc_vault",
    ]
    with CSV_PATH.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for row in rows:
            output = dict(row)
            output["block_time_iso"] = format_time(row.get("block_time"))
            writer.writerow({field: output.get(field, "") for field in fields})


def quantile(values, q):
    if not values:
        return None
    values = sorted(values)
    position = (len(values) - 1) * q
    low = math.floor(position)
    high = math.ceil(position)
    if low == high:
        return values[low]
    return values[low] * (high - position) + values[high] * (position - low)


def summarize(rows, market):
    before = [row for row in rows if row["slot"] < TARGET_SLOT]
    after = [row for row in rows if row["slot"] >= TARGET_SLOT]

    def stats(items):
        values = [row["lbtc_reserve"] for row in items]
        if not values:
            return {}
        maximum = max(items, key=lambda row: row["lbtc_reserve"])
        minimum = min(items, key=lambda row: row["lbtc_reserve"])
        return {
            "count": len(items),
            "first_slot": items[0]["slot"],
            "first_time": format_time(items[0].get("block_time")),
            "last_slot": items[-1]["slot"],
            "last_time": format_time(items[-1].get("block_time")),
            "latest_lbtc_reserve": items[-1]["lbtc_reserve"],
            "min_lbtc_reserve": minimum["lbtc_reserve"],
            "min_slot": minimum["slot"],
            "min_time": format_time(minimum.get("block_time")),
            "median_lbtc_reserve": statistics.median(values),
            "p90_lbtc_reserve": quantile(values, 0.90),
            "max_lbtc_reserve": maximum["lbtc_reserve"],
            "max_slot": maximum["slot"],
            "max_time": format_time(maximum.get("block_time")),
        }

    first_below_01 = next((row for row in rows if row["lbtc_reserve"] < 0.1), None)
    first_below_005 = next((row for row in rows if row["lbtc_reserve"] < 0.05), None)
    summary = {
        "pool": POOL,
        "target_slot": TARGET_SLOT,
        "market": market,
        "snapshot_count": len(rows),
        "first_snapshot": rows[0] if rows else None,
        "latest_snapshot": rows[-1] if rows else None,
        "before_target": stats(before),
        "after_target": stats(after),
        "threshold_crossings": {
            "first_below_0_1": first_below_01,
            "first_below_0_05": first_below_005,
        },
    }
    SUMMARY_PATH.write_text(json.dumps(summary, indent=2))
    return summary


def svg_escape(value):
    return (
        str(value).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    )


def make_svg(rows, summary):
    width, height = 1280, 760
    margin_left, margin_right = 82, 34
    top_one, chart_height = 78, 250
    top_two = 430
    times = [row["block_time"] for row in rows if row.get("block_time")]
    min_time, max_time = min(times), max(times)

    def x(timestamp):
        return margin_left + (timestamp - min_time) / (max_time - min_time) * (
            width - margin_left - margin_right
        )

    def y(value, minimum, maximum, top):
        if maximum == minimum:
            return top + chart_height / 2
        return top + chart_height - (value - minimum) / (maximum - minimum) * chart_height

    max_lbtc = max(row["lbtc_reserve"] for row in rows)
    max_liquidity = max(row["estimated_liquidity_usd_current_price"] for row in rows)
    min_liquidity = min(row["estimated_liquidity_usd_current_price"] for row in rows)

    def points(key, minimum, maximum, top):
        return " ".join(
            f"{x(row['block_time']):.1f},{y(row[key], minimum, maximum, top):.1f}"
            for row in rows
            if row.get("block_time")
        )

    def axis(top, minimum, maximum, label, money=False):
        parts = [
            f'<line x1="{margin_left}" y1="{top}" x2="{margin_left}" y2="{top + chart_height}" stroke="#8a8f98"/>',
            f'<line x1="{margin_left}" y1="{top + chart_height}" x2="{width - margin_right}" y2="{top + chart_height}" stroke="#8a8f98"/>',
        ]
        for index in range(6):
            value = minimum + (maximum - minimum) * index / 5
            yy = y(value, minimum, maximum, top)
            text = f"${value / 1_000_000:.1f}M" if money else f"{value:.2f}"
            parts.append(
                f'<line x1="{margin_left - 5}" y1="{yy:.1f}" x2="{width - margin_right}" y2="{yy:.1f}" stroke="#e6e8eb"/>'
            )
            parts.append(
                f'<text x="{margin_left - 10}" y="{yy + 4:.1f}" text-anchor="end" class="tick">{svg_escape(text)}</text>'
            )
        parts.append(
            f'<text x="{margin_left}" y="{top - 18}" class="subtitle">{svg_escape(label)}</text>'
        )
        return "\n".join(parts)

    ticks = []
    span = max_time - min_time
    for index in range(7):
        timestamp = min_time + span * index / 6
        xx = x(timestamp)
        label = dt.datetime.fromtimestamp(timestamp, dt.timezone.utc).strftime("%b %d")
        ticks.append(
            f'<line x1="{xx:.1f}" y1="{top_two + chart_height}" x2="{xx:.1f}" y2="{top_two + chart_height + 5}" stroke="#8a8f98"/>'
        )
        ticks.append(
            f'<text x="{xx:.1f}" y="{top_two + chart_height + 24}" text-anchor="middle" class="tick">{label}</text>'
        )

    target_rows = [
        row for row in rows if row["slot"] >= TARGET_SLOT and row.get("block_time")
    ]
    vertical_line = ""
    if target_rows:
        xx = x(target_rows[0]["block_time"])
        vertical_line = (
            f'<line x1="{xx:.1f}" y1="{top_one}" x2="{xx:.1f}" y2="{top_two + chart_height}" '
            'stroke="#111827" stroke-dasharray="6 6" opacity="0.5"/>'
            f'<text x="{xx + 8:.1f}" y="{top_one + 16}" class="note">slot {TARGET_SLOT}</text>'
        )

    y_01 = y(0.1, 0, max_lbtc * 1.05, top_one)
    y_005 = y(0.05, 0, max_lbtc * 1.05, top_one)
    summary_text = (
        f"before slot max {summary['before_target'].get('max_lbtc_reserve', 0):.4f} LBTC; "
        f"current {summary['latest_snapshot']['lbtc_reserve']:.4f} LBTC"
    )
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
<style>
  .bg {{ fill:#fbfbfa; }} .title {{ font:700 24px -apple-system,BlinkMacSystemFont,Segoe UI,sans-serif; fill:#111827; }}
  .subtitle {{ font:700 15px -apple-system,BlinkMacSystemFont,Segoe UI,sans-serif; fill:#111827; }}
  .tick {{ font:12px -apple-system,BlinkMacSystemFont,Segoe UI,sans-serif; fill:#5b616e; }}
  .note {{ font:12px -apple-system,BlinkMacSystemFont,Segoe UI,sans-serif; fill:#374151; }}
  .legend {{ font:13px -apple-system,BlinkMacSystemFont,Segoe UI,sans-serif; fill:#374151; }}
</style>
<rect class="bg" x="0" y="0" width="{width}" height="{height}"/>
<text x="{margin_left}" y="38" class="title">LBTC Meteora DLMM pool reserves</text>
<text x="{margin_left}" y="60" class="note">Pool {POOL}; USD estimates use current DexScreener prices. {svg_escape(summary_text)}</text>
{axis(top_one, 0, max_lbtc * 1.05, "LBTC-side reserve")}
<line x1="{margin_left}" y1="{y_01:.1f}" x2="{width - margin_right}" y2="{y_01:.1f}" stroke="#dc2626" stroke-dasharray="4 4" opacity="0.55"/><text x="{width - margin_right - 4}" y="{y_01 - 6:.1f}" text-anchor="end" class="note">0.10 LBTC</text>
<line x1="{margin_left}" y1="{y_005:.1f}" x2="{width - margin_right}" y2="{y_005:.1f}" stroke="#f97316" stroke-dasharray="4 4" opacity="0.65"/><text x="{width - margin_right - 4}" y="{y_005 - 6:.1f}" text-anchor="end" class="note">0.05 LBTC</text>
<polyline fill="none" stroke="#d52727" stroke-width="2.2" points="{points("lbtc_reserve", 0, max_lbtc * 1.05, top_one)}"/>
{axis(top_two, min_liquidity * 0.96, max_liquidity * 1.04, "Estimated total pool liquidity, current USD prices", money=True)}
<polyline fill="none" stroke="#2563eb" stroke-width="2.1" points="{points("estimated_liquidity_usd_current_price", min_liquidity * 0.96, max_liquidity * 1.04, top_two)}"/>
{"".join(ticks)}
{vertical_line}
<circle cx="{margin_left + 18}" cy="{height - 34}" r="5" fill="#d52727"/><text x="{margin_left + 30}" y="{height - 30}" class="legend">LBTC reserve</text>
<circle cx="{margin_left + 150}" cy="{height - 34}" r="5" fill="#2563eb"/><text x="{margin_left + 162}" y="{height - 30}" class="legend">Total liquidity estimate</text>
</svg>'''
    SVG_PATH.write_text(svg)


def main():
    load_backend_env()
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    market = fetch_market()
    items = signatures()
    print("successful signatures", len(items))
    rows = fetch_snapshots(items, market)
    print("snapshots", len(rows))
    write_csv(rows)
    summary = summarize(rows, market)
    make_svg(rows, summary)
    print("CSV", CSV_PATH)
    print("SVG", SVG_PATH)
    print("SUMMARY", SUMMARY_PATH)
    print(
        json.dumps(
            {
                "snapshot_count": summary["snapshot_count"],
                "before_target": summary["before_target"],
                "after_target": summary["after_target"],
                "first_below_0_1": (
                    {
                        key: summary["threshold_crossings"]["first_below_0_1"][key]
                        for key in ["slot", "block_time", "lbtc_reserve"]
                    }
                    if summary["threshold_crossings"]["first_below_0_1"]
                    else None
                ),
                "first_below_0_05": (
                    {
                        key: summary["threshold_crossings"]["first_below_0_05"][key]
                        for key in ["slot", "block_time", "lbtc_reserve"]
                    }
                    if summary["threshold_crossings"]["first_below_0_05"]
                    else None
                ),
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
