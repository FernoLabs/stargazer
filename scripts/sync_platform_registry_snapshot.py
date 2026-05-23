#!/usr/bin/env python3

import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from tempfile import TemporaryDirectory


REPO_URL = "https://github.com/jup-ag/platform-list.git"
PLATFORM_DIR = Path("src/platforms")


@dataclass
class ProgramSurface:
    label: str
    program_id: str


def find_matching_brace(text: str, start_index: int) -> int:
    depth = 0
    i = start_index
    in_string = None
    in_line_comment = False
    in_block_comment = False
    escaped = False

    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""

        if in_line_comment:
            if ch == "\n":
                in_line_comment = False
            i += 1
            continue

        if in_block_comment:
            if ch == "*" and nxt == "/":
                in_block_comment = False
                i += 2
                continue
            i += 1
            continue

        if in_string:
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == in_string:
                in_string = None
            i += 1
            continue

        if ch == "/" and nxt == "/":
            in_line_comment = True
            i += 2
            continue

        if ch == "/" and nxt == "*":
            in_block_comment = True
            i += 2
            continue

        if ch in ("'", '"', "`"):
            in_string = ch
            i += 1
            continue

        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return i

        i += 1

    raise ValueError("unmatched brace")


def extract_object_body(source: str, marker: str) -> str | None:
    index = source.find(marker)
    if index == -1:
        return None
    brace_start = source.find("{", index)
    if brace_start == -1:
        return None
    brace_end = find_matching_brace(source, brace_start)
    return source[brace_start + 1 : brace_end]


def extract_string(body: str, field: str) -> str | None:
    pattern = re.compile(rf"{field}\s*:\s*([\"'`])(.+?)\1", re.DOTALL)
    match = pattern.search(body)
    return match.group(2).strip() if match else None


def extract_bool(body: str, field: str) -> bool:
    pattern = re.compile(rf"{field}\s*:\s*(true|false)")
    match = pattern.search(body)
    return match.group(1) == "true" if match else False


def extract_tags(body: str) -> list[str]:
    match = re.search(r"tags\s*:\s*\[(.*?)\]", body, re.DOTALL)
    if not match:
        return []
    return re.findall(r'["\']([^"\']+)["\']', match.group(1))


def extract_contract_objects(source: str) -> list[ProgramSurface]:
    surfaces: list[ProgramSurface] = []
    seen: set[str] = set()
    for match in re.finditer(r"(?:const|export const)\s+\w+\s*=\s*\{", source):
        brace_start = source.find("{", match.start())
        brace_end = find_matching_brace(source, brace_start)
        body = source[brace_start + 1 : brace_end]
        if "networkId: NetworkId.solana" not in body:
            continue
        address = extract_string(body, "address")
        label = extract_string(body, "name")
        if not address or not label:
            continue
        if address in seen:
            continue
        seen.add(address)
        surfaces.append(ProgramSurface(label=label, program_id=address))
    return surfaces


def parse_platform_file(name: str, source: str):
    platform_body = extract_object_body(source, "export const platform")
    if not platform_body:
        return None

    platform_id = extract_string(platform_body, "id")
    platform_name = extract_string(platform_body, "name")
    if not platform_id or not platform_name:
        return None

    programs = extract_contract_objects(source)
    if not programs:
        return None

    return {
        "slug": platform_id,
        "name": platform_name,
        "is_deprecated": extract_bool(platform_body, "isDeprecated"),
        "tags": extract_tags(platform_body),
        "programs": [surface.__dict__ for surface in programs],
        "source_file": name,
    }


def main():
    output_path = (
        Path(sys.argv[1])
        if len(sys.argv) > 1
        else Path(__file__).resolve().parents[1] / "backend" / "data" / "platform_registry_snapshot.json"
    )

    items = []
    with TemporaryDirectory(prefix="platform-list-") as temp_dir:
        repo_dir = Path(temp_dir) / "platform-list"
        subprocess.run(
            [
                "git",
                "clone",
                "--depth",
                "1",
                "--filter=blob:none",
                "--sparse",
                REPO_URL,
                str(repo_dir),
            ],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        subprocess.run(
            ["git", "-C", str(repo_dir), "sparse-checkout", "set", str(PLATFORM_DIR)],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        for file_path in sorted((repo_dir / PLATFORM_DIR).glob("*.ts")):
            if file_path.name == "index.ts":
                continue
            parsed = parse_platform_file(file_path.name, file_path.read_text(encoding="utf-8"))
            if parsed:
                items.append(parsed)

    items.sort(key=lambda item: item["slug"])
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(
            {
                "source": "https://github.com/jup-ag/platform-list",
                "item_count": len(items),
                "items": items,
            },
            indent=2,
            sort_keys=False,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"Wrote {len(items)} platform entries to {output_path}")


if __name__ == "__main__":
    main()
