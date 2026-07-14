#!/usr/bin/env python3
"""Build FireWatch's compact, deterministic instruction catalog from published IDLs.

The generated file is embedded in the backend binary.  Ordinary decoded
instructions therefore stay transient while alert rules can still validate and
decode instruction names, named accounts, and safely addressable scalar args.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from audit_idl_admin_candidates import (
    classify_instruction,
    program_labels,
    snake_case,
    type_category,
    type_description,
)


INTEGER_WIDTHS = {
    "i8": 8,
    "i16": 16,
    "i32": 32,
    "i64": 64,
    "i128": 128,
    "u8": 8,
    "u16": 16,
    "u32": 32,
    "u64": 64,
    "u128": 128,
}
FLOAT_WIDTHS = {"f32": 32, "f64": 64}
PUBKEY_NAMES = {"pubkey", "publickey", "public_key"}


@dataclass(frozen=True)
class SelectedIdl:
    program_id: str
    path: Path
    origin: str
    schema: str
    value: dict[str, Any]
    raw_bytes: bytes

    @property
    def instructions(self) -> list[Any]:
        if self.schema == "codama":
            return self.value["program"].get("instructions", [])
        return self.value.get("instructions", [])


def load_selected_idls(idl_dir: Path) -> dict[str, SelectedIdl]:
    """Load one IDL per program, preferring Program Metadata over legacy IDLs."""
    selected: dict[str, tuple[tuple[int, int], SelectedIdl]] = {}
    pattern = re.compile(
        r"^(?P<program>[^.]+)\.(?P<origin>program_metadata|anchor_legacy)\.(json|txt)$"
    )
    for path in sorted(idl_dir.iterdir()):
        match = pattern.match(path.name)
        if match is None:
            continue
        try:
            raw_bytes = path.read_bytes()
            value = json.loads(raw_bytes)
        except (OSError, UnicodeDecodeError, json.JSONDecodeError):
            continue
        if not isinstance(value, dict):
            continue
        if isinstance(value.get("instructions"), list):
            schema = "anchor"
        elif (
            value.get("kind") == "rootNode"
            and isinstance(value.get("program"), dict)
            and isinstance(value["program"].get("instructions"), list)
        ):
            schema = "codama"
        else:
            continue

        origin = match.group("origin")
        candidate = SelectedIdl(
            program_id=match.group("program"),
            path=path,
            origin=origin,
            schema=schema,
            value=value,
            raw_bytes=raw_bytes,
        )
        # JSON wins a same-origin tie, but Program Metadata always wins overall.
        priority = (1 if origin == "program_metadata" else 0, 1 if path.suffix == ".json" else 0)
        existing = selected.get(candidate.program_id)
        if existing is None or priority > existing[0]:
            selected[candidate.program_id] = (priority, candidate)
    return {program_id: item[1] for program_id, item in selected.items()}


def humanize(value: str) -> str:
    return snake_case(value).replace("_", " ").title()


def idl_program_name(idl: SelectedIdl) -> str:
    if idl.schema == "codama":
        value = idl.value["program"].get("name")
    else:
        metadata = idl.value.get("metadata")
        value = metadata.get("name") if isinstance(metadata, dict) else None
        value = value or idl.value.get("name")
    return value if isinstance(value, str) and value.strip() else "unknown"


def anchor_discriminator(name: str) -> bytes:
    return hashlib.sha256(f"global:{snake_case(name)}".encode()).digest()[:8]


def anchor_definitions(idl: SelectedIdl) -> dict[str, Any]:
    definitions: dict[str, Any] = {}
    for definition in idl.value.get("types", []) or []:
        if not isinstance(definition, dict):
            continue
        name = definition.get("name")
        if isinstance(name, str):
            definitions[name] = definition.get("type")
    return definitions


def anchor_defined_name(value: Any) -> str | None:
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        name = value.get("name")
        if isinstance(name, str):
            return name
    return None


def anchor_fixed_size(
    value: Any, definitions: dict[str, Any], stack: frozenset[str] = frozenset()
) -> int | None:
    if isinstance(value, str):
        normalized = value.lower().replace("_", "")
        if normalized in INTEGER_WIDTHS:
            return INTEGER_WIDTHS[normalized] // 8
        if normalized in FLOAT_WIDTHS:
            return FLOAT_WIDTHS[normalized] // 8
        if normalized in {name.replace("_", "") for name in PUBKEY_NAMES}:
            return 32
        if normalized == "bool":
            return 1
        return None
    if not isinstance(value, dict):
        return None
    if "array" in value:
        array = value["array"]
        if not isinstance(array, list) or len(array) != 2 or not isinstance(array[1], int):
            return None
        item_size = anchor_fixed_size(array[0], definitions, stack)
        return item_size * array[1] if item_size is not None and array[1] >= 0 else None
    if "tuple" in value:
        items = value["tuple"]
        if not isinstance(items, list):
            return None
        sizes = [anchor_fixed_size(item, definitions, stack) for item in items]
        return sum(sizes) if all(size is not None for size in sizes) else None
    if "defined" in value:
        name = anchor_defined_name(value["defined"])
        if name is None or name in stack:
            return None
        definition = definitions.get(name)
        return anchor_fixed_size(definition, definitions, stack | {name})

    kind = value.get("kind")
    if kind == "struct":
        fields = value.get("fields")
        if not isinstance(fields, list):
            return None
        sizes = [
            anchor_fixed_size(field.get("type"), definitions, stack)
            if isinstance(field, dict)
            else None
            for field in fields
        ]
        return sum(sizes) if all(size is not None for size in sizes) else None
    if kind == "alias":
        return anchor_fixed_size(value.get("value"), definitions, stack)
    # Vec, bytes, string, option, and enums are intentionally treated as variable.
    return None


def anchor_scalar(value: Any) -> dict[str, Any] | None:
    if not isinstance(value, str):
        return None
    normalized = value.lower().replace("_", "")
    if normalized in INTEGER_WIDTHS:
        width = INTEGER_WIDTHS[normalized]
        return {
            "value_kind": "integer",
            "signed": normalized.startswith("i"),
            "bit_width": width,
            "byte_length": width // 8,
            "endianness": "little",
        }
    if normalized in FLOAT_WIDTHS:
        width = FLOAT_WIDTHS[normalized]
        return {
            "value_kind": "float",
            "bit_width": width,
            "byte_length": width // 8,
            "endianness": "little",
        }
    if normalized in {name.replace("_", "") for name in PUBKEY_NAMES}:
        return {"value_kind": "pubkey", "byte_length": 32}
    if normalized == "bool":
        return {"value_kind": "boolean", "byte_length": 1}
    return None


def codama_definitions(idl: SelectedIdl) -> dict[str, Any]:
    definitions: dict[str, Any] = {}
    for definition in idl.value["program"].get("definedTypes", []) or []:
        if not isinstance(definition, dict):
            continue
        name = definition.get("name")
        if isinstance(name, str):
            definitions[name] = definition.get("type")
    return definitions


def codama_fixed_size(
    value: Any, definitions: dict[str, Any], stack: frozenset[str] = frozenset()
) -> int | None:
    if not isinstance(value, dict):
        return None
    kind = value.get("kind")
    if kind == "numberTypeNode":
        width = INTEGER_WIDTHS.get(str(value.get("format"))) or FLOAT_WIDTHS.get(
            str(value.get("format"))
        )
        return width // 8 if width is not None else None
    if kind == "publicKeyTypeNode":
        return 32
    if kind == "booleanTypeNode":
        return 1
    if kind == "fixedSizeTypeNode":
        size = value.get("size")
        return size if isinstance(size, int) and size >= 0 else None
    if kind == "arrayTypeNode":
        count = value.get("count")
        if not isinstance(count, dict) or count.get("kind") != "fixedCountNode":
            return None
        count_value = count.get("value")
        item_size = codama_fixed_size(value.get("item"), definitions, stack)
        if not isinstance(count_value, int) or count_value < 0 or item_size is None:
            return None
        return count_value * item_size
    if kind == "structTypeNode":
        fields = value.get("fields")
        if not isinstance(fields, list):
            return None
        sizes = [
            codama_fixed_size(field.get("type"), definitions, stack)
            if isinstance(field, dict)
            else None
            for field in fields
        ]
        return sum(sizes) if all(size is not None for size in sizes) else None
    if kind == "definedTypeLinkNode":
        name = value.get("name")
        if not isinstance(name, str) or name in stack:
            return None
        return codama_fixed_size(definitions.get(name), definitions, stack | {name})
    # Size prefixes, variable arrays/options, strings, bytes, and enums are variable.
    return None


def codama_scalar(value: Any) -> dict[str, Any] | None:
    if not isinstance(value, dict):
        return None
    kind = value.get("kind")
    if kind == "numberTypeNode":
        number_format = str(value.get("format"))
        if number_format in INTEGER_WIDTHS:
            width = INTEGER_WIDTHS[number_format]
            return {
                "value_kind": "integer",
                "signed": number_format.startswith("i"),
                "bit_width": width,
                "byte_length": width // 8,
                "endianness": "big" if value.get("endian") == "be" else "little",
            }
        if number_format in FLOAT_WIDTHS:
            width = FLOAT_WIDTHS[number_format]
            return {
                "value_kind": "float",
                "bit_width": width,
                "byte_length": width // 8,
                "endianness": "big" if value.get("endian") == "be" else "little",
            }
    if kind == "publicKeyTypeNode":
        return {"value_kind": "pubkey", "byte_length": 32}
    if kind == "booleanTypeNode":
        return {"value_kind": "boolean", "byte_length": 1}
    return None


def codama_type_description(value: Any) -> str:
    scalar = codama_scalar(value)
    if scalar is not None:
        if scalar["value_kind"] in {"integer", "float"}:
            prefix = "i" if scalar.get("signed") else (
                "f" if scalar["value_kind"] == "float" else "u"
            )
            return f"{prefix}{scalar['bit_width']}"
        return "bool" if scalar["value_kind"] == "boolean" else "pubkey"
    if isinstance(value, dict) and value.get("kind") == "definedTypeLinkNode":
        return f"defined:{value.get('name', 'unknown')}"
    if isinstance(value, dict):
        return str(value.get("kind", "object"))
    return "unknown"


def codama_type_category(value: Any) -> str:
    scalar = codama_scalar(value)
    if scalar is not None:
        return {
            "integer": "integer",
            "float": "float",
            "pubkey": "pubkey",
            "boolean": "boolean",
        }[scalar["value_kind"]]
    if not isinstance(value, dict):
        return "other"
    kind = str(value.get("kind", ""))
    if kind == "definedTypeLinkNode":
        return "defined"
    if kind in {"arrayTypeNode", "bytesTypeNode"}:
        return "collection"
    if kind == "optionTypeNode":
        return "optional"
    if kind in {"stringTypeNode", "sizePrefixTypeNode"}:
        return "string"
    return "other"


def flatten_accounts(accounts: Any, groups: tuple[str, ...] = ()) -> list[dict[str, Any]]:
    flattened: list[dict[str, Any]] = []
    if not isinstance(accounts, list):
        return flattened
    for account in accounts:
        if not isinstance(account, dict):
            continue
        raw_name = account.get("name")
        name = snake_case(raw_name) if isinstance(raw_name, str) else ""
        children = account.get("accounts")
        if isinstance(children, list):
            flattened.extend(flatten_accounts(children, groups + ((name or "group"),)))
            continue
        qualified_name = ".".join((*groups, name)) if groups else name
        flattened.append(
            {
                "index": len(flattened),
                "name": qualified_name,
                "is_signer": bool(account.get("isSigner", account.get("signer", False))),
                "is_writable": bool(account.get("isMut", account.get("writable", False))),
            }
        )
    return flattened


def reindex_accounts(accounts: list[dict[str, Any]]) -> list[dict[str, Any]]:
    # Recursive groups index locally; assign transaction positions after flattening.
    used_names: set[str] = set()
    for index, account in enumerate(accounts):
        account["index"] = index
        base_name = account["name"] or f"account_{index}"
        unique_name = base_name
        suffix = 2
        while unique_name in used_names:
            unique_name = f"{base_name}_{suffix}"
            suffix += 1
        account["name"] = unique_name
        used_names.add(unique_name)
    return accounts


def anchor_arguments(
    instruction: dict[str, Any], discriminator_length: int, definitions: dict[str, Any]
) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    cursor: int | None = discriminator_length
    for index, argument in enumerate(instruction.get("args", []) or []):
        if not isinstance(argument, dict):
            cursor = None
            continue
        raw_name = argument.get("name")
        name = snake_case(raw_name) if isinstance(raw_name, str) else f"argument_{index}"
        value_type = argument.get("type")
        scalar = anchor_scalar(value_type)
        record: dict[str, Any] = {
            "index": index,
            "name": name,
            "type_name": type_description(value_type),
            "type_category": type_category(value_type),
            "filterable": False,
        }
        if scalar is not None:
            record.update(scalar)
            if cursor is not None:
                record["byte_offset"] = cursor
                record["filterable"] = scalar["value_kind"] in {
                    "integer",
                    "boolean",
                    "pubkey",
                }
        records.append(record)
        size = anchor_fixed_size(value_type, definitions)
        cursor = cursor + size if cursor is not None and size is not None else None
    return records


def codama_discriminator(instruction: dict[str, Any]) -> tuple[int, bytes, str] | None:
    arguments = instruction.get("arguments", []) or []
    for discriminator in instruction.get("discriminators", []) or []:
        if not isinstance(discriminator, dict) or discriminator.get("kind") != "fieldDiscriminatorNode":
            continue
        name = discriminator.get("name")
        offset = discriminator.get("offset", 0)
        if not isinstance(name, str) or not isinstance(offset, int) or offset < 0:
            continue
        for argument in arguments:
            if not isinstance(argument, dict) or argument.get("name") != name:
                continue
            default = argument.get("defaultValue")
            if not isinstance(default, dict):
                continue
            if default.get("kind") == "bytesValueNode" and default.get("encoding") == "base16":
                data = default.get("data")
                if isinstance(data, str):
                    try:
                        return offset, bytes.fromhex(data), name
                    except ValueError:
                        continue
    return None


def codama_arguments(
    instruction: dict[str, Any], discriminator_name: str, definitions: dict[str, Any]
) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    cursor: int | None = 0
    visible_index = 0
    for argument in instruction.get("arguments", []) or []:
        if not isinstance(argument, dict):
            cursor = None
            continue
        value_type = argument.get("type")
        size = codama_fixed_size(value_type, definitions)
        is_omitted_discriminator = (
            argument.get("name") == discriminator_name
            and argument.get("defaultValueStrategy") == "omitted"
        )
        if not is_omitted_discriminator:
            raw_name = argument.get("name")
            name = (
                snake_case(raw_name)
                if isinstance(raw_name, str)
                else f"argument_{visible_index}"
            )
            scalar = codama_scalar(value_type)
            record: dict[str, Any] = {
                "index": visible_index,
                "name": name,
                "type_name": codama_type_description(value_type),
                "type_category": codama_type_category(value_type),
                "filterable": False,
            }
            if scalar is not None:
                record.update(scalar)
                if cursor is not None:
                    record["byte_offset"] = cursor
                    record["filterable"] = scalar["value_kind"] in {
                        "integer",
                        "boolean",
                        "pubkey",
                    }
            records.append(record)
            visible_index += 1
        cursor = cursor + size if cursor is not None and size is not None else None
    return records


def instruction_descriptor(
    idl: SelectedIdl, instruction: dict[str, Any], definitions: dict[str, Any]
) -> dict[str, Any] | None:
    raw_name = instruction.get("name")
    if not isinstance(raw_name, str) or not raw_name.strip():
        return None
    name = snake_case(raw_name)

    if idl.schema == "codama":
        resolved = codama_discriminator(instruction)
        if resolved is None:
            discriminator_offset, discriminator, discriminator_name = 0, b"", ""
            discriminator_source = "unresolved"
        else:
            discriminator_offset, discriminator, discriminator_name = resolved
            discriminator_source = "codama_field_default"
        arguments = codama_arguments(instruction, discriminator_name, definitions)
    else:
        explicit = instruction.get("discriminator")
        if isinstance(explicit, list) and explicit and all(
            isinstance(value, int) and 0 <= value <= 255 for value in explicit
        ):
            discriminator = bytes(explicit)
            discriminator_source = "idl"
        else:
            discriminator = anchor_discriminator(name)
            discriminator_source = "anchor_global_hash"
        discriminator_offset = 0
        arguments = anchor_arguments(instruction, len(discriminator), definitions)

    accounts = reindex_accounts(flatten_accounts(instruction.get("accounts", [])))
    # Keep classification identical to the standalone audit. Nested account
    # groups are flattened for decoding, but the audit intentionally scores only
    # signer roles directly declared on an instruction account entry.
    signer_names = []
    for account in instruction.get("accounts", []) or []:
        if not isinstance(account, dict):
            continue
        raw_account_name = account.get("name")
        account_name = (
            snake_case(raw_account_name) if isinstance(raw_account_name, str) else ""
        )
        if bool(account.get("isSigner", account.get("signer", False))) and account_name:
            signer_names.append(account_name)
    argument_names = [argument["name"] for argument in arguments]
    classification = classify_instruction(name, signer_names, argument_names)
    return {
        "name": name,
        "raw_name": raw_name,
        "discriminator_hex": discriminator.hex(),
        "discriminator_offset": discriminator_offset,
        "discriminator_source": discriminator_source,
        "admin_confidence": classification.confidence,
        "admin_score": classification.score,
        "recommended_severity": classification.recommended_severity,
        "admin_reasons": classification.reasons,
        "accounts": accounts,
        "arguments": arguments,
    }


def build_catalog(workspace: Path, idl_dir: Path) -> dict[str, Any]:
    labels_by_program = program_labels(workspace)
    selected = load_selected_idls(idl_dir)
    programs: list[dict[str, Any]] = []
    confidence_counts = {
        "high_confidence_admin": 0,
        "likely_admin": 0,
        "manual_review": 0,
        "not_admin": 0,
    }
    filterable_argument_count = 0
    unresolved_discriminator_count = 0

    for program_id, idl in sorted(selected.items()):
        idl_name = idl_program_name(idl)
        fallback_name = humanize(idl_name) if idl_name != "unknown" else "Unknown"
        labels = labels_by_program.get(
            program_id,
            {
                "protocol_slug": snake_case(idl_name),
                "protocol_name": fallback_name,
                "program_label": fallback_name,
            },
        )
        definitions = (
            codama_definitions(idl) if idl.schema == "codama" else anchor_definitions(idl)
        )
        instructions = [
            descriptor
            for value in idl.instructions
            if isinstance(value, dict)
            for descriptor in [instruction_descriptor(idl, value, definitions)]
            if descriptor is not None
        ]
        instructions.sort(key=lambda item: (item["name"], item["discriminator_hex"]))

        seen_discriminators: set[tuple[int, str]] = set()
        for instruction in instructions:
            discriminator_key = (
                instruction["discriminator_offset"],
                instruction["discriminator_hex"],
            )
            if instruction["discriminator_hex"]:
                if discriminator_key in seen_discriminators:
                    raise ValueError(
                        f"duplicate discriminator for {program_id}: {discriminator_key}"
                    )
                seen_discriminators.add(discriminator_key)
            else:
                unresolved_discriminator_count += 1
            confidence_counts[instruction["admin_confidence"]] += 1
            filterable_argument_count += sum(
                argument["filterable"] for argument in instruction["arguments"]
            )

        admin_candidate_count = sum(
            instruction["admin_confidence"] != "not_admin"
            for instruction in instructions
        )
        program_filterable_argument_count = sum(
            argument["filterable"]
            for instruction in instructions
            for argument in instruction["arguments"]
        )
        programs.append(
            {
                "program_id": program_id,
                "protocol_slug": labels.get("protocol_slug", "unknown"),
                "protocol_name": labels.get("protocol_name", "Unknown"),
                "program_label": labels.get("program_label", "") or fallback_name,
                "idl_name": idl_name,
                "idl_origin": idl.origin,
                "idl_schema": idl.schema,
                "idl_source": idl.path.name,
                "idl_sha256": hashlib.sha256(idl.raw_bytes).hexdigest(),
                "instruction_count": len(instructions),
                "admin_candidate_count": admin_candidate_count,
                "filterable_argument_count": program_filterable_argument_count,
                "instructions": instructions,
            }
        )

    instruction_count = sum(program["instruction_count"] for program in programs)
    return {
        "version": 1,
        "summary": {
            "program_count": len(programs),
            "instruction_count": instruction_count,
            "filterable_argument_count": filterable_argument_count,
            "unresolved_discriminator_count": unresolved_discriminator_count,
            "confidence_counts": confidence_counts,
        },
        "programs": programs,
    }


def render_catalog(catalog: dict[str, Any]) -> str:
    return json.dumps(catalog, ensure_ascii=False, separators=(",", ":")) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, default=Path.cwd())
    parser.add_argument("--idl-dir", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the existing output differs from a fresh deterministic build",
    )
    args = parser.parse_args()

    workspace = args.workspace.resolve()
    idl_dir = (args.idl_dir or workspace / "backend" / "data" / "idls").resolve()
    output = (
        args.output or workspace / "backend" / "data" / "idl_instruction_catalog.json"
    ).resolve()
    rendered = render_catalog(build_catalog(workspace, idl_dir))
    if args.check:
        if not output.exists() or output.read_text() != rendered:
            raise SystemExit(f"catalog is stale: {output}")
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(rendered)

    summary = json.loads(rendered)["summary"]
    print(
        "IDL catalog: "
        f"{summary['program_count']} programs, "
        f"{summary['instruction_count']} instructions, "
        f"{summary['filterable_argument_count']} filterable arguments"
    )


if __name__ == "__main__":
    main()
