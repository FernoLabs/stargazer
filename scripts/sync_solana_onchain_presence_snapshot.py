#!/usr/bin/env python3

import json
import os
import re
import subprocess
import sys
import time
from collections import defaultdict
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
from tempfile import TemporaryDirectory
from urllib.error import HTTPError, URLError
from urllib.parse import urljoin, urlparse
from urllib.request import Request, urlopen


ROOT = Path(__file__).resolve().parents[1]
API_BASE = "https://api.llama.fi"
CHAIN = "Solana"
SOLANA_RPC = "https://api.mainnet-beta.solana.com"
INCLUDE_PROTOCOL_WEBSITES = os.environ.get("INCLUDE_PROTOCOL_WEBSITES", "1") != "0"
INCLUDE_SOLANA_COMPASS = os.environ.get("INCLUDE_SOLANA_COMPASS") == "1"
INCLUDE_IDL_PROBE = os.environ.get("INCLUDE_IDL_PROBE", "1") != "0"
DEFILLAMA_ADAPTERS_REPO = "https://github.com/DefiLlama/DefiLlama-Adapters.git"
DIMENSION_ADAPTERS_REPO = "https://github.com/DefiLlama/dimension-adapters.git"
DEFILLAMA_ADAPTERS_BRANCH = "main"
DIMENSION_ADAPTERS_BRANCH = "master"
BASE58_RE = re.compile(r"(?<![1-9A-HJ-NP-Za-km-z])[1-9A-HJ-NP-Za-km-z]{32,44}(?![1-9A-HJ-NP-Za-km-z])")
PROTOCOL_SPEC_RE = re.compile(r'ProtocolSpec\s*\{\s*slug:\s*"([^"]+)"')

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
    (
        "options",
        f"{API_BASE}/overview/options?excludeTotalDataChart=true&excludeTotalDataChartBreakdown=true",
    ),
]

COMMON_EXECUTABLE_PROGRAMS = {
    "11111111111111111111111111111111",
    "AddressLookupTab1e1111111111111111111111111",
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
    "BPFLoader1111111111111111111111111111111111",
    "BPFLoader2111111111111111111111111111111111",
    "BPFLoaderUpgradeab1e11111111111111111111111",
    "ComputeBudget111111111111111111111111111111",
    "Config1111111111111111111111111111111111111",
    "Ed25519SigVerify111111111111111111111111111",
    "KeccakSecp256k11111111111111111111111111111",
    "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo",
    "Memo4c2pN8afCj432Lb7RMVKi9PbQnnW7ewFFaV3oAH",
    "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr",
    "Stake11111111111111111111111111111111111111",
    "Sysvar1111111111111111111111111111111111111",
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
    "TokenzQdBNbLqP5VEhdkAS6EPF2VKQ5CVY4BxF9jLkQ",
    "Vote111111111111111111111111111111111111111",
    "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s",
}

BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
BASE58_INDEX = {char: index for index, char in enumerate(BASE58_ALPHABET)}
DOC_LINK_KEYWORDS = (
    "address",
    "contract",
    "deploy",
    "developer",
    "dev",
    "docs",
    "doc",
    "gitbook",
    "program",
    "protocol",
    "sdk",
    "solana",
)
WEBSITE_PROGRAM_CONTEXT_KEYWORDS = (
    "anchor",
    "contract",
    "declare_id",
    "deployed",
    "executing_account",
    "program",
    "program id",
    "program_id",
    "smart contract",
)
SKIPPED_URL_HOSTS = (
    "discord.",
    "docs.google.",
    "explorer.solana.",
    "facebook.",
    "instagram.",
    "linkedin.",
    "medium.",
    "orbmarkets.",
    "solana.com",
    "solana.fm",
    "solscan.",
    "t.me",
    "telegram.",
    "twitter.",
    "x.com",
    "youtube.",
)


@dataclass(frozen=True)
class SourceFile:
    source: str
    local_path: Path
    public_url: str


class LinkCollector(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]):
        if tag.lower() != "a":
            return
        for name, value in attrs:
            if name.lower() == "href" and value:
                self.links.append(value)


def fetch_json(url: str):
    request = Request(url, headers={"User-Agent": "ferno-watcher/onchain-presence-sync"})
    with urlopen(request, timeout=30) as response:
        return json.loads(response.read().decode("utf-8"))


def normalize_slug(value: str) -> str:
    return value.strip().lower().replace(" ", "_").replace("-", "_")


def chain_matches(item: dict) -> bool:
    chains = item.get("chains") or []
    chain_tvls = item.get("chainTvls") or {}
    return CHAIN in chains or CHAIN in chain_tvls


def source_items(source_name: str, payload):
    if source_name == "protocols":
        return [item for item in payload if chain_matches(item)]

    protocols = payload.get("protocols") or []
    if source_name in {"open_interest", "options"}:
        return [item for item in protocols if chain_matches(item)]

    return protocols


def clean_string(value):
    return value.strip() if isinstance(value, str) and value.strip() else None


def protocol_records_from_defillama() -> dict[str, dict]:
    protocols: dict[str, dict] = {}
    for source_name, url in SOURCE_ENDPOINTS:
        try:
            payload = fetch_json(url)
        except (HTTPError, URLError, TimeoutError) as error:
            print(f"Skipped {source_name}: {error}", file=sys.stderr)
            continue

        for item in source_items(source_name, payload):
            slug = clean_string(item.get("slug"))
            name = clean_string(item.get("displayName")) or clean_string(item.get("name"))
            if not slug or not name:
                continue

            normalized = normalize_slug(slug)
            record = protocols.setdefault(
                normalized,
                {
                    "slug": normalized,
                    "defillama_slug": slug,
                    "name": name,
                    "raw": [],
                    "source_names": set(),
                },
            )
            record["name"] = record["name"] or name
            record["raw"].append(item)
            record["source_names"].add(source_name)

    return protocols


def existing_supported_slugs() -> set[str]:
    source = (ROOT / "backend" / "src" / "protocols.rs").read_text(encoding="utf-8")
    slugs = {normalize_slug(slug) for slug in PROTOCOL_SPEC_RE.findall(source)}
    slugs.update({"squads", "ferno"})

    watchable_tags = {
        "amm",
        "bridge",
        "defi",
        "dex",
        "institutional",
        "launchpad",
        "lending",
        "liquidity-provider",
        "options",
        "oracle",
        "payments",
        "prediction",
        "restaking",
        "rwa",
        "stablecoin",
        "trading",
        "vault",
    }
    registry = json.loads(
        (ROOT / "backend" / "data" / "platform_registry_snapshot.json").read_text(
            encoding="utf-8"
        )
    )
    for item in registry["items"]:
        if item.get("is_deprecated") or not item.get("programs"):
            continue
        if any(tag in watchable_tags for tag in item.get("tags", [])):
            slugs.add(normalize_slug(item["slug"]))

    return slugs


def existing_program_ids() -> set[str]:
    source = (ROOT / "backend" / "src" / "protocols.rs").read_text(encoding="utf-8")
    program_ids = {
        match
        for match in re.findall(r'program_id:\s*"([^"]+)"', source)
        if is_base58_pubkey(match)
    }
    squads_match = re.search(r'pub const SQUADS_V4_PROGRAM_ID:\s*&str\s*=\s*"([^"]+)"', source)
    if squads_match:
        program_ids.add(squads_match.group(1))

    registry = json.loads(
        (ROOT / "backend" / "data" / "platform_registry_snapshot.json").read_text(
            encoding="utf-8"
        )
    )
    for item in registry["items"]:
        for program in item.get("programs", []):
            program_id = program.get("program_id")
            if program_id and is_base58_pubkey(program_id):
                program_ids.add(program_id)

    return program_ids


def clone_sparse(repo_url: str, branch: str, sparse_paths: list[str], destination: Path):
    subprocess.run(
        [
            "git",
            "clone",
            "--depth",
            "1",
            "--filter=blob:none",
            "--sparse",
            "--branch",
            branch,
            repo_url,
            str(destination),
        ],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    subprocess.run(
        ["git", "-C", str(destination), "sparse-checkout", "set", *sparse_paths],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def github_blob_to_local(url: str, repo_dir: Path, owner: str, repo: str) -> Path | None:
    parsed = urlparse(url)
    parts = [part for part in parsed.path.split("/") if part]
    if len(parts) < 5 or parts[0].lower() != owner.lower() or parts[1].lower() != repo.lower():
        return None
    if parts[2] not in {"blob", "tree"}:
        return None

    return repo_dir.joinpath(*parts[4:])


def source_files_for_record(
    record: dict, adapters_dir: Path, dimensions_dir: Path
) -> list[SourceFile]:
    candidates: list[SourceFile] = []
    seen_paths: set[Path] = set()

    def add_path(source: str, path: Path, public_url: str):
        if path in seen_paths or not path.exists():
            return
        seen_paths.add(path)
        candidates.append(SourceFile(source=source, local_path=path, public_url=public_url))

    for item in record["raw"]:
        tvl_code_path = clean_string(item.get("tvlCodePath"))
        if tvl_code_path:
            path = github_blob_to_local(
                tvl_code_path, adapters_dir, "DefiLlama", "DefiLlama-Adapters"
            )
            if path:
                add_path("defillama_adapters", path, tvl_code_path)

        methodology_url = clean_string(item.get("methodologyURL"))
        if methodology_url:
            path = github_blob_to_local(
                methodology_url, dimensions_dir, "DefiLlama", "dimension-adapters"
            )
            if path:
                add_path("dimension_adapters", path, methodology_url)

        module = clean_string(item.get("module"))
        if module and module != "dummy.js":
            path = adapters_dir / "projects" / module
            add_path(
                "defillama_adapters",
                path,
                f"https://github.com/DefiLlama/DefiLlama-Adapters/tree/main/projects/{module}",
            )

    slug = record["defillama_slug"]
    for relative in [
        Path("projects") / slug,
        Path("projects") / f"{slug}.js",
        Path("projects") / f"{slug}.ts",
    ]:
        add_path(
            "defillama_adapters",
            adapters_dir / relative,
            f"https://github.com/DefiLlama/DefiLlama-Adapters/tree/main/{relative.as_posix()}",
        )

    return candidates


def iter_files(path: Path):
    if path.is_file():
        yield path
        return

    for file_path in path.rglob("*"):
        if file_path.is_file() and file_path.suffix in {
            ".cjs",
            ".js",
            ".json",
            ".mjs",
            ".ts",
            ".tsx",
        }:
            yield file_path


def is_base58_pubkey(value: str) -> bool:
    number = 0
    for char in value:
        index = BASE58_INDEX.get(char)
        if index is None:
            return False
        number = number * 58 + index

    decoded = number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b""
    leading_zeroes = len(value) - len(value.lstrip("1"))
    return len((b"\0" * leading_zeroes) + decoded) == 32


def candidate_context(text: str, start: int, end: int) -> str:
    return re.sub(r"\s+", " ", text[max(0, start - 90) : min(len(text), end + 90)]).strip()


def extract_candidates(source_file: SourceFile) -> dict[str, dict]:
    candidates = {}
    for file_path in iter_files(source_file.local_path):
        try:
            text = file_path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        if len(text) > 2_000_000:
            continue

        for match in BASE58_RE.finditer(text):
            address = match.group(0)
            if address in COMMON_EXECUTABLE_PROGRAMS or not is_base58_pubkey(address):
                continue
            entry = candidates.setdefault(
                address,
                {
                    "source": source_file.source,
                    "source_url": source_file.public_url,
                    "source_path": str(file_path.relative_to(source_file.local_path))
                    if source_file.local_path.is_dir()
                    else file_path.name,
                    "context": candidate_context(text, match.start(), match.end()),
                },
            )
            if not entry.get("context"):
                entry["context"] = candidate_context(text, match.start(), match.end())
    return candidates


def protocol_website_urls(record: dict) -> list[str]:
    urls = []
    seen = set()
    for item in record["raw"]:
        url = normalize_web_url(clean_string(item.get("url")))
        if not url or url in seen:
            continue
        seen.add(url)
        urls.append(url)
    return urls


def normalize_web_url(url: str | None) -> str | None:
    if not url:
        return None
    if url.startswith("//"):
        url = f"https:{url}"
    elif not url.startswith(("http://", "https://")):
        url = f"https://{url}"

    parsed = urlparse(url)
    if parsed.scheme not in {"http", "https"} or not parsed.netloc:
        return None
    host = parsed.netloc.lower()
    if any(skipped in host for skipped in SKIPPED_URL_HOSTS):
        return None
    return url


def fetch_web_text(url: str) -> str | None:
    try:
        request = Request(url, headers={"User-Agent": "ferno-watcher/onchain-presence-sync"})
        with urlopen(request, timeout=8) as response:
            content_type = response.headers.get("content-type", "").lower()
            if not any(
                kind in content_type
                for kind in ("html", "json", "markdown", "text", "xml")
            ):
                return None
            return response.read(750_000).decode("utf-8", errors="ignore")
    except (HTTPError, URLError, TimeoutError, OSError):
        return None


def candidate_page_urls(home_url: str, html: str) -> list[str]:
    parsed_home = urlparse(home_url)
    collector = LinkCollector()
    try:
        collector.feed(html)
    except Exception:
        return []

    scored = []
    seen = set()
    for href in collector.links:
        if href.startswith(("#", "mailto:", "tel:", "javascript:")):
            continue
        url = normalize_web_url(urljoin(home_url, href))
        if not url or url in seen:
            continue
        seen.add(url)
        parsed = urlparse(url)
        if parsed.scheme not in {"http", "https"}:
            continue

        same_host = parsed.netloc.lower() == parsed_home.netloc.lower()
        docs_host = parsed.netloc.lower().startswith("docs.")
        searchable = f"{parsed.netloc} {parsed.path} {parsed.query}".lower()
        keyword_score = sum(1 for keyword in DOC_LINK_KEYWORDS if keyword in searchable)
        if not same_host and not docs_host and keyword_score == 0:
            continue
        if keyword_score == 0:
            continue
        scored.append((-keyword_score, 0 if docs_host else 1 if same_host else 2, url))

    return [url for _, _, url in sorted(scored)[:5]]


def extract_webpage_candidates(url: str, text: str) -> dict[str, dict]:
    candidates = {}
    for match in BASE58_RE.finditer(text):
        address = match.group(0)
        if address in COMMON_EXECUTABLE_PROGRAMS or not is_base58_pubkey(address):
            continue
        context = candidate_context(text, match.start(), match.end())
        if not any(keyword in context.lower() for keyword in WEBSITE_PROGRAM_CONTEXT_KEYWORDS):
            continue
        candidates[address] = {
            "source": "protocol_website",
            "source_url": url,
            "source_path": "webpage",
            "context": context,
        }
    return candidates


def scrape_protocol_website_candidates(record: dict) -> dict[str, dict]:
    candidates = {}
    for home_url in protocol_website_urls(record)[:2]:
        home_text = fetch_web_text(home_url)
        if not home_text:
            continue
        for address, candidate in extract_webpage_candidates(home_url, home_text).items():
            candidates.setdefault(address, candidate)

        for page_url in candidate_page_urls(home_url, home_text):
            page_text = fetch_web_text(page_url)
            if not page_text:
                continue
            for address, candidate in extract_webpage_candidates(page_url, page_text).items():
                candidates.setdefault(address, candidate)

    return candidates


def solana_rpc(method: str, params: list):
    payload = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode(
        "utf-8"
    )
    request = Request(
        SOLANA_RPC,
        data=payload,
        headers={
            "Content-Type": "application/json",
            "User-Agent": "ferno-watcher/onchain-presence-sync",
        },
    )
    with urlopen(request, timeout=30) as response:
        return json.loads(response.read().decode("utf-8"))


def executable_accounts(addresses: set[str]) -> set[str]:
    executable = set()
    ordered = sorted(addresses)
    for index in range(0, len(ordered), 100):
        batch = ordered[index : index + 100]
        for attempt in range(4):
            try:
                response = solana_rpc(
                    "getMultipleAccounts",
                    [
                        batch,
                        {
                            "encoding": "base64",
                            "dataSlice": {"offset": 0, "length": 0},
                        },
                    ],
                )
                break
            except (HTTPError, URLError, TimeoutError) as error:
                if attempt == 3:
                    print(f"RPC batch skipped after retries: {error}", file=sys.stderr)
                    response = {"result": {"value": [None] * len(batch)}}
                else:
                    time.sleep(0.5 * (attempt + 1))
        values = (response.get("result") or {}).get("value") or []
        for address, account in zip(batch, values):
            if account and account.get("executable"):
                executable.add(address)
    return executable


def probe_idl_accounts(program_ids: list[str]) -> dict[str, dict]:
    if not program_ids:
        return {}

    env = os.environ.copy()
    env.setdefault("SOLANA_RPC_URL", SOLANA_RPC)
    try:
        result = subprocess.run(
            ["cargo", "run", "--quiet", "--bin", "idl_probe"],
            input=json.dumps(program_ids),
            text=True,
            capture_output=True,
            check=True,
            cwd=ROOT / "backend",
            env=env,
        )
    except subprocess.CalledProcessError as error:
        print(
            "Skipped IDL probing after cargo/RPC failure: "
            f"{error.stderr.strip() or error.stdout.strip()}",
            file=sys.stderr,
        )
        return {}

    probes = json.loads(result.stdout)
    return {probe["program_id"]: probe for probe in probes}


def scrape_solana_compass_candidates(record: dict) -> dict[str, dict]:
    slug = record["defillama_slug"]
    url = f"https://solanacompass.com/projects/{slug}"
    try:
        request = Request(url, headers={"User-Agent": "ferno-watcher/onchain-presence-sync"})
        with urlopen(request, timeout=15) as response:
            if response.status != 200:
                return {}
            text = response.read().decode("utf-8", errors="ignore")
    except (HTTPError, URLError, TimeoutError):
        return {}

    candidates = {}
    for match in BASE58_RE.finditer(text):
        address = match.group(0)
        if address in COMMON_EXECUTABLE_PROGRAMS or not is_base58_pubkey(address):
            continue
        candidates[address] = {
            "source": "solana_compass",
            "source_url": url,
            "source_path": "html",
            "context": candidate_context(text, match.start(), match.end()),
        }
    return candidates


def program_label(index: int, candidate: dict) -> str:
    source = candidate.get("source")
    if source == "protocol_website":
        return "Protocol website program" if index == 1 else f"Protocol website program {index}"
    if source == "solana_compass":
        return "Solana Compass program" if index == 1 else f"Solana Compass program {index}"
    if source == "dimension_adapters":
        return "DefiLlama dimensions program" if index == 1 else f"DefiLlama dimensions program {index}"
    return "DefiLlama adapter program" if index == 1 else f"DefiLlama adapter program {index}"


def main():
    output_path = (
        Path(sys.argv[1])
        if len(sys.argv) > 1
        else ROOT / "backend" / "data" / "solana_onchain_presence_snapshot.json"
    )

    protocols = protocol_records_from_defillama()
    existing = existing_supported_slugs()
    known_program_ids = existing_program_ids()
    unsupported = {
        slug: record for slug, record in protocols.items() if slug not in existing
    }

    print(
        f"Scanning {len(unsupported)} unsupported DefiLlama Solana protocols "
        f"(protocol websites {'enabled' if INCLUDE_PROTOCOL_WEBSITES else 'disabled'}, "
        f"Solana Compass {'enabled' if INCLUDE_SOLANA_COMPASS else 'disabled'}, "
        f"IDL probe {'enabled' if INCLUDE_IDL_PROBE else 'disabled'}).",
        file=sys.stderr,
    )

    with TemporaryDirectory(prefix="onchain-presence-") as temp_dir:
        temp_path = Path(temp_dir)
        adapters_dir = temp_path / "defillama-adapters"
        dimensions_dir = temp_path / "dimension-adapters"
        print("Cloning DefiLlama adapter sources...", file=sys.stderr)
        clone_sparse(DEFILLAMA_ADAPTERS_REPO, DEFILLAMA_ADAPTERS_BRANCH, ["projects"], adapters_dir)
        clone_sparse(
            DIMENSION_ADAPTERS_REPO,
            DIMENSION_ADAPTERS_BRANCH,
            ["fees", "dexs", "open-interest", "options"],
            dimensions_dir,
        )

        candidates_by_slug: dict[str, dict[str, dict]] = defaultdict(dict)
        all_candidates: set[str] = set()
        for count, (slug, record) in enumerate(unsupported.items(), start=1):
            if count % 50 == 0:
                print(f"Scanned {count}/{len(unsupported)} protocol source mappings...", file=sys.stderr)
            for source_file in source_files_for_record(record, adapters_dir, dimensions_dir):
                for address, candidate in extract_candidates(source_file).items():
                    if address in known_program_ids:
                        continue
                    candidates_by_slug[slug].setdefault(address, candidate)
                    all_candidates.add(address)

            if INCLUDE_PROTOCOL_WEBSITES and not candidates_by_slug.get(slug):
                for address, candidate in scrape_protocol_website_candidates(record).items():
                    if address in known_program_ids:
                        continue
                    candidates_by_slug[slug].setdefault(address, candidate)
                    all_candidates.add(address)

            if INCLUDE_SOLANA_COMPASS:
                for address, candidate in scrape_solana_compass_candidates(record).items():
                    if address in known_program_ids:
                        continue
                    candidates_by_slug[slug].setdefault(address, candidate)
                    all_candidates.add(address)

        print(f"Validating {len(all_candidates)} unique candidate addresses with Solana RPC...", file=sys.stderr)
        executable = executable_accounts(all_candidates)
        print(f"Found {len(executable)} executable candidate addresses.", file=sys.stderr)

    items = []
    for slug, candidates in sorted(candidates_by_slug.items()):
        programs = []
        for index, address in enumerate(sorted(set(candidates) & executable), start=1):
            candidate = candidates[address]
            programs.append(
                {
                    "label": program_label(index, candidate),
                    "program_id": address,
                    "source": candidate["source"],
                    "source_url": candidate["source_url"],
                    "source_path": candidate["source_path"],
                    "evidence": candidate["context"][:300],
                }
            )

        if not programs:
            continue

        record = unsupported[slug]
        items.append(
            {
                "slug": slug,
                "defillama_slug": record["defillama_slug"],
                "name": record["name"],
                "sources": sorted(record["source_names"]),
                "programs": programs,
            }
        )

    idl_probes = {}
    if INCLUDE_IDL_PROBE:
        program_ids = sorted(
            {
                program["program_id"]
                for item in items
                for program in item["programs"]
            }
        )
        print(f"Probing on-chain IDL accounts for {len(program_ids)} programs...", file=sys.stderr)
        idl_probes = probe_idl_accounts(program_ids)
        for item in items:
            for program in item["programs"]:
                probe = idl_probes.get(program["program_id"])
                if probe:
                    program["idl_accounts"] = {
                        "program_metadata": probe["program_metadata"],
                        "anchor_legacy": probe["anchor_legacy"],
                    }

    idl_program_count = sum(
        1
        for item in items
        for program in item["programs"]
        if any(
            account.get("present")
            for account in (program.get("idl_accounts") or {}).values()
        )
    )
    idl_account_count = sum(
        1
        for item in items
        for program in item["programs"]
        for account in (program.get("idl_accounts") or {}).values()
        if account.get("present")
    )

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(
            {
                "source": "defillama_adapters + dimension_adapters"
                + (" + protocol_websites" if INCLUDE_PROTOCOL_WEBSITES else "")
                + (" + solana_compass" if INCLUDE_SOLANA_COMPASS else ""),
                "chain": CHAIN,
                "unsupported_defillama_protocol_count": len(unsupported),
                "item_count": len(items),
                "program_count": sum(len(item["programs"]) for item in items),
                "idl_program_count": idl_program_count,
                "idl_account_count": idl_account_count,
                "items": items,
            },
            indent=2,
            sort_keys=False,
        )
        + "\n",
        encoding="utf-8",
    )
    print(
        "Wrote "
        f"{len(items)} protocol entries / {sum(len(item['programs']) for item in items)} "
        f"programs / {idl_program_count} programs with on-chain IDLs to {output_path}"
    )


if __name__ == "__main__":
    main()
