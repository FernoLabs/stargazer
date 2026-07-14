#!/usr/bin/env python3
"""Audit downloaded Solana IDLs for instructions that may be administrative.

The audit is deliberately conservative about automatic classification. It combines
instruction names with signer roles and argument names, then emits enough IDL
metadata to support manual review and future instruction-alert rule generation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any


INTEGER_TYPES = {
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
}
FLOAT_TYPES = {"f32", "f64"}
PUBKEY_TYPES = {"pubkey", "publickey", "public_key"}

CONTROL_VERBS = (
    "accept",
    "change",
    "nominate",
    "propose",
    "renounce",
    "replace",
    "set",
    "transfer",
    "update",
)
CONTROL_SUBJECTS = (
    "admin",
    "authority",
    "controller",
    "curator",
    "delegate",
    "governance",
    "guardian",
    "manager",
    "operator",
    "owner",
    "risk_council",
)
CONFIG_SUBJECTS = (
    "cap",
    "config",
    "fee",
    "limit",
    "oracle",
    "parameter",
    "permission",
    "price_feed",
    "rate",
    "risk",
    "status",
    "whitelist",
)
USER_SCOPES = (
    "borrower",
    "deposit",
    "lender",
    "nft",
    "obligation",
    "position",
    "stake_account",
    "token_account",
    "token_owner",
    "user",
)


def snake_case(value: str) -> str:
    value = re.sub(r"[^A-Za-z0-9]+", "_", value)
    value = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", value)
    value = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", value)
    return value.strip("_").lower()


def anchor_discriminator(name: str) -> str:
    return hashlib.sha256(f"global:{snake_case(name)}".encode()).digest()[:8].hex()


def type_description(value: Any) -> str:
    if isinstance(value, str):
        return value
    if not isinstance(value, dict):
        return "unknown"
    if "defined" in value:
        defined = value["defined"]
        if isinstance(defined, dict):
            return f"defined:{defined.get('name', 'unknown')}"
        return f"defined:{defined}"
    key = next(iter(value), "object")
    return f"{key}:{type_description(value[key])}"


def type_category(value: Any) -> str:
    description = type_description(value).lower()
    bare = description.split(":", 1)[0]
    if bare in INTEGER_TYPES:
        return "integer"
    if bare in FLOAT_TYPES:
        return "float"
    if bare in PUBKEY_TYPES:
        return "pubkey"
    if bare == "bool":
        return "boolean"
    if bare == "string":
        return "string"
    if bare in {"option", "coption"}:
        return "optional"
    if bare in {"array", "vec", "bytes"}:
        return "collection"
    if bare == "defined":
        return "defined"
    return "other"


def codama_argument_type(argument: dict[str, Any]) -> Any:
    value = argument.get("type")
    if value is not None:
        return value
    return argument.get("defaultValue", {}).get("type", "codama_node")


def privileged_role(name: str) -> bool:
    if any(scope in name for scope in USER_SCOPES):
        return False
    return any(subject in name for subject in CONTROL_SUBJECTS)


def sensitive_argument(name: str) -> bool:
    return any(subject in name for subject in CONTROL_SUBJECTS + CONFIG_SUBJECTS)


@dataclass
class Classification:
    confidence: str
    score: int
    recommended_severity: str | None
    reasons: list[str]


def classify_instruction(
    name: str, signer_names: list[str], argument_names: list[str]
) -> Classification:
    score = 0
    reasons: list[str] = []
    user_scoped = any(scope in name for scope in USER_SCOPES)

    if re.search(r"(^|_)admin($|_)", name):
        score += 8
        reasons.append("instruction explicitly names an admin role")

    has_control_verb = any(
        name == verb or name.startswith(f"{verb}_") or f"_{verb}_" in name
        for verb in CONTROL_VERBS
    )
    named_control_subjects = [subject for subject in CONTROL_SUBJECTS if subject in name]
    if has_control_verb and named_control_subjects:
        score += 6
        reasons.append(
            "changes a control role: " + ", ".join(named_control_subjects[:3])
        )

    emergency_terms = [
        term
        for term in (
            "emergency",
            "freeze",
            "halt",
            "pause",
            "resume",
            "shutdown",
            "unfreeze",
            "unpause",
        )
        if re.search(rf"(^|_){term}($|_)", name)
    ]
    if emergency_terms:
        score += 6
        reasons.append("emergency control: " + ", ".join(emergency_terms))

    if re.search(r"(^|_)(upgrade|migrate_program|set_upgrade_authority)($|_)", name):
        score += 7
        reasons.append("program upgrade or migration control")

    if "governance" in name and has_control_verb:
        score += 5
        reasons.append("governance mutation")

    if re.search(
        r"(withdraw|sweep|collect).*(treasury|insurance|protocol_fee|protocol_fees|admin_fee|admin_fees)",
        name,
    ):
        score += 7
        reasons.append("moves protocol, treasury, or insurance funds")

    config_hits = [subject for subject in CONFIG_SUBJECTS if subject in name]
    if has_control_verb and config_hits:
        score += 4
        reasons.append("changes protocol configuration: " + ", ".join(config_hits[:3]))

    if re.search(r"^(close|delete|remove)_(protocol|market|reserve|bank|vault|config|realm)", name):
        score += 4
        reasons.append("closes or removes protocol-level state")

    privileged_signers = [value for value in signer_names if privileged_role(value)]
    if privileged_signers:
        score += 3
        reasons.append("privileged signer: " + ", ".join(privileged_signers[:3]))

    sensitive_arguments = [value for value in argument_names if sensitive_argument(value)]
    if sensitive_arguments:
        score += 2
        reasons.append("sensitive argument: " + ", ".join(sensitive_arguments[:3]))

    if has_control_verb and not named_control_subjects and not config_hits:
        score += 1
        reasons.append("generic state-changing verb")

    if user_scoped and not re.search(r"(^|_)(admin|governance|guardian)($|_)", name):
        score -= 4
        reasons.append("reduced confidence because the instruction is user-scoped")

    if score >= 10:
        confidence = "high_confidence_admin"
    elif score >= 7:
        confidence = "likely_admin"
    elif score >= 4:
        confidence = "manual_review"
    else:
        confidence = "not_admin"

    critical_reason = any(
        phrase in " ".join(reasons)
        for phrase in (
            "admin role",
            "control role",
            "emergency control",
            "program upgrade",
            "treasury",
            "insurance funds",
        )
    )
    if confidence == "high_confidence_admin":
        severity = "critical" if critical_reason else "high"
    elif confidence == "likely_admin":
        severity = "high"
    else:
        severity = None

    return Classification(confidence, max(score, 0), severity, reasons)


def program_labels(workspace: Path) -> dict[str, dict[str, str]]:
    labels: dict[str, dict[str, str]] = {}
    for filename in (
        "platform_registry_snapshot.json",
        "solana_onchain_presence_snapshot.json",
    ):
        path = workspace / "backend" / "data" / filename
        if not path.exists():
            continue
        snapshot = json.loads(path.read_text())
        for item in snapshot.get("items", []):
            for program in item.get("programs", []):
                labels.setdefault(
                    program["program_id"],
                    {
                        "protocol_slug": item.get("slug", "unknown"),
                        "protocol_name": item.get("name", "Unknown"),
                        "program_label": program.get("label", ""),
                    },
                )
    return labels


def load_idls(idl_dir: Path) -> dict[str, dict[str, Any]]:
    programs: dict[str, dict[str, Any]] = {}
    for path in sorted(idl_dir.glob("*.*.*")):
        if ".program_metadata." in path.name:
            origin = "program_metadata"
        elif ".anchor_legacy." in path.name:
            origin = "anchor_legacy"
        else:
            continue
        try:
            value = json.loads(path.read_text())
        except (OSError, json.JSONDecodeError):
            continue

        if isinstance(value.get("instructions"), list):
            instructions = value["instructions"]
            schema = "anchor"
        elif value.get("kind") == "rootNode" and isinstance(value.get("program"), dict):
            instructions = value["program"].get("instructions", [])
            schema = "codama"
        else:
            continue

        program_id = path.name.split(".", 1)[0]
        candidate = {
            "program_id": program_id,
            "origin": origin,
            "schema": schema,
            "idl_file": str(path),
            "instructions": instructions,
        }
        existing = programs.get(program_id)
        if existing is None or origin == "program_metadata":
            programs[program_id] = candidate
    return programs


def instruction_record(
    program: dict[str, Any], instruction: dict[str, Any], labels: dict[str, str]
) -> dict[str, Any] | None:
    raw_name = instruction.get("name")
    if not isinstance(raw_name, str) or not raw_name.strip():
        return None
    name = snake_case(raw_name)

    discriminator = instruction.get("discriminator")
    if isinstance(discriminator, list) and discriminator:
        discriminator_hex = bytes(discriminator).hex()
        discriminator_source = "idl"
    else:
        discriminator_hex = anchor_discriminator(name)
        discriminator_source = "anchor_global_hash"

    accounts = []
    signer_names = []
    for index, account in enumerate(instruction.get("accounts", []) or []):
        if not isinstance(account, dict):
            continue
        account_name = snake_case(str(account.get("name", "")))
        is_signer = bool(account.get("isSigner", account.get("signer", False)))
        is_writable = bool(account.get("isMut", account.get("writable", False)))
        accounts.append(
            {
                "index": index,
                "name": account_name,
                "is_signer": is_signer,
                "is_writable": is_writable,
            }
        )
        if is_signer and account_name:
            signer_names.append(account_name)

    raw_arguments = instruction.get("args", instruction.get("arguments", [])) or []
    arguments = []
    argument_names = []
    for index, argument in enumerate(raw_arguments):
        if not isinstance(argument, dict):
            continue
        argument_name = snake_case(str(argument.get("name", "")))
        argument_type = codama_argument_type(argument)
        arguments.append(
            {
                "index": index,
                "name": argument_name,
                "type": type_description(argument_type),
                "type_category": type_category(argument_type),
            }
        )
        if argument_name:
            argument_names.append(argument_name)

    classification = classify_instruction(name, signer_names, argument_names)
    return {
        **labels,
        "program_id": program["program_id"],
        "program_label": labels.get("program_label", ""),
        "idl_origin": program["origin"],
        "idl_schema": program["schema"],
        "idl_file": program["idl_file"],
        "instruction_name": name,
        "raw_instruction_name": raw_name,
        "discriminator_hex": discriminator_hex,
        "discriminator_source": discriminator_source,
        "admin_confidence": classification.confidence,
        "admin_score": classification.score,
        "recommended_severity": classification.recommended_severity,
        "admin_reasons": classification.reasons,
        "accounts": accounts,
        "arguments": arguments,
    }


def build_report(workspace: Path, idl_dir: Path) -> dict[str, Any]:
    labels_by_program = program_labels(workspace)
    programs = load_idls(idl_dir)
    instructions = []
    for program_id, program in programs.items():
        labels = labels_by_program.get(
            program_id,
            {
                "protocol_slug": "unknown",
                "protocol_name": "Unknown",
                "program_label": "",
            },
        )
        for instruction in program["instructions"]:
            if not isinstance(instruction, dict):
                continue
            record = instruction_record(program, instruction, labels)
            if record is not None:
                instructions.append(record)

    counts = {
        confidence: sum(
            item["admin_confidence"] == confidence for item in instructions
        )
        for confidence in (
            "high_confidence_admin",
            "likely_admin",
            "manual_review",
            "not_admin",
        )
    }
    candidate_programs = {
        item["program_id"]
        for item in instructions
        if item["admin_confidence"] != "not_admin"
    }
    return {
        "summary": {
            "program_count": len(programs),
            "instruction_count": len(instructions),
            "candidate_program_count": len(candidate_programs),
            "confidence_counts": counts,
        },
        "instructions": sorted(
            instructions,
            key=lambda item: (
                -item["admin_score"],
                item["protocol_name"],
                item["program_id"],
                item["instruction_name"],
            ),
        ),
    }


def markdown_report(report: dict[str, Any]) -> str:
    summary = report["summary"]
    candidates = [
        item
        for item in report["instructions"]
        if item["admin_confidence"] != "not_admin"
    ]
    lines = [
        "# FireWatch IDL admin-candidate audit",
        "",
        f"- Programs reviewed: {summary['program_count']}",
        f"- Instructions reviewed: {summary['instruction_count']}",
        f"- Programs with candidates: {summary['candidate_program_count']}",
    ]
    for confidence, count in summary["confidence_counts"].items():
        lines.append(f"- {confidence.replace('_', ' ').title()}: {count}")
    lines.extend(
        [
            "",
            "The automatic tiers are heuristic. Only high-confidence and likely-admin",
            "entries should be considered for default alerts; manual-review entries need",
            "protocol-specific confirmation.",
            "",
            "| Confidence | Severity | Protocol | Program | Instruction | Reasons |",
            "|---|---|---|---|---|---|",
        ]
    )
    for item in candidates:
        reasons = "; ".join(item["admin_reasons"]).replace("|", "\\|")
        lines.append(
            "| {confidence} | {severity} | {protocol} | `{program}` | `{instruction}` | {reasons} |".format(
                confidence=item["admin_confidence"],
                severity=item["recommended_severity"] or "review",
                protocol=item["protocol_name"],
                program=item["program_id"],
                instruction=item["instruction_name"],
                reasons=reasons,
            )
        )
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, default=Path.cwd())
    parser.add_argument("--idl-dir", type=Path)
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    args = parser.parse_args()

    workspace = args.workspace.resolve()
    idl_dir = (args.idl_dir or workspace / "backend" / "data" / "idls").resolve()
    report = build_report(workspace, idl_dir)

    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(json.dumps(report, indent=2) + "\n")
    if args.markdown_output:
        args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_output.write_text(markdown_report(report))
    if not args.json_output and not args.markdown_output:
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
