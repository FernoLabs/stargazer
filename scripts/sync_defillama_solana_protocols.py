#!/usr/bin/env python3

import json
import sys
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


API_BASE = "https://api.llama.fi"
CHAIN = "Solana"
SOURCE_ENDPOINTS = [
    ("protocols", f"{API_BASE}/protocols"),
    (
        "fees",
        f"{API_BASE}/overview/fees/{CHAIN}?excludeTotalDataChart=true&excludeTotalDataChartBreakdown=true",
    ),
    (
        "dexs",
        f"{API_BASE}/overview/dexs/{CHAIN}?excludeTotalDataChart=true&excludeTotalDataChartBreakdown=true",
    ),
    (
        "open_interest",
        f"{API_BASE}/overview/open-interest?excludeTotalDataChart=true&excludeTotalDataChartBreakdown=true",
    ),
]


def fetch_json(url: str):
    request = Request(url, headers={"User-Agent": "ferno-watcher/defillama-sync"})
    with urlopen(request, timeout=30) as response:
        return json.loads(response.read().decode("utf-8"))


def chain_matches(item: dict) -> bool:
    chains = item.get("chains") or []
    chain_tvls = item.get("chainTvls") or {}
    return CHAIN in chains or CHAIN in chain_tvls


def source_items(source_name: str, payload):
    if source_name == "protocols":
        return [item for item in payload if chain_matches(item)]

    protocols = payload.get("protocols") or []
    if source_name == "open_interest":
        return [item for item in protocols if chain_matches(item)]

    return protocols


def clean_string(value):
    return value.strip() if isinstance(value, str) and value.strip() else None


def upsert_protocol(protocols: dict, source_name: str, item: dict):
    slug = clean_string(item.get("slug"))
    name = clean_string(item.get("displayName")) or clean_string(item.get("name"))
    if not slug or not name:
        return

    existing = protocols.setdefault(
        slug,
        {
            "slug": slug,
            "name": name,
            "category": None,
            "chains": [],
            "url": None,
            "twitter": None,
            "defillama_id": None,
            "module": None,
            "parent_protocol": None,
            "logo": None,
            "sources": [],
        },
    )

    existing["name"] = existing["name"] or name
    existing["category"] = existing["category"] or clean_string(item.get("category"))
    existing["url"] = existing["url"] or clean_string(item.get("url"))
    existing["twitter"] = existing["twitter"] or clean_string(item.get("twitter"))
    existing["defillama_id"] = existing["defillama_id"] or clean_string(
        item.get("defillamaId") or item.get("id")
    )
    existing["module"] = existing["module"] or clean_string(item.get("module"))
    existing["parent_protocol"] = existing["parent_protocol"] or clean_string(
        item.get("parentProtocol")
    )
    existing["logo"] = existing["logo"] or clean_string(item.get("logo"))

    for chain in [*(item.get("chains") or []), CHAIN]:
        if chain not in existing["chains"]:
            existing["chains"].append(chain)

    if source_name not in existing["sources"]:
        existing["sources"].append(source_name)


def main():
    output_path = (
        Path(sys.argv[1])
        if len(sys.argv) > 1
        else Path(__file__).resolve().parents[1]
        / "backend"
        / "data"
        / "defillama_solana_protocols_snapshot.json"
    )

    protocols = {}
    successful_sources = []
    for source_name, source_url in SOURCE_ENDPOINTS:
        try:
            payload = fetch_json(source_url)
        except (HTTPError, URLError, TimeoutError) as error:
            print(f"Skipped {source_name}: {error}", file=sys.stderr)
            continue

        successful_sources.append({"name": source_name, "url": source_url})
        for item in source_items(source_name, payload):
            upsert_protocol(protocols, source_name, item)

    items = sorted(
        protocols.values(),
        key=lambda item: (item["slug"].lower(), item["name"].lower()),
    )

    for item in items:
        item["chains"].sort()
        item["sources"].sort()

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(
            {
                "source": API_BASE,
                "chain": CHAIN,
                "item_count": len(items),
                "sources": successful_sources,
                "items": items,
            },
            indent=2,
            sort_keys=False,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"Wrote {len(items)} DefiLlama Solana protocol entries to {output_path}")


if __name__ == "__main__":
    main()
