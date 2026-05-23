use crate::program_bytes::sha256_hex;
use anyhow::{Context, Result};
use sbpf_common::instruction::AsmFormat;
use sbpf_disassembler::program::Program;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::Write as _,
};

pub(crate) const DISASSEMBLY_FORMAT: &str = "llvm+anchor-labels-v1";

#[derive(Clone, Debug)]
pub(crate) struct KnownProgramInstruction {
    pub(crate) name: String,
    pub(crate) discriminator: [u8; 8],
}

#[derive(Clone, Debug)]
pub(crate) struct ProgramDisassembly {
    pub(crate) format: String,
    pub(crate) instruction_count: i64,
    pub(crate) entrypoint_instruction: Option<i64>,
    pub(crate) asm_text: String,
    pub(crate) asm_sha256: String,
}

pub(crate) fn disassemble_program_bytes(
    bytes: &[u8],
    known_instructions: &[KnownProgramInstruction],
) -> Result<ProgramDisassembly> {
    let program = Program::from_bytes(bytes).context("parse sBPF ELF")?;
    let (instructions, _rodata, entrypoint_instruction) =
        program.to_ixs().context("disassemble sBPF program")?;

    let mut lines = Vec::with_capacity(instructions.len());
    for (index, instruction) in instructions.iter().enumerate() {
        let asm = instruction
            .to_asm(AsmFormat::Llvm)
            .with_context(|| format!("format sBPF instruction {index}"))?;
        lines.push(asm);
    }

    let labels = infer_anchor_labels(&lines, known_instructions);
    let asm_text = render_asm_text(&lines, &labels);
    let asm_sha256 = sha256_hex(asm_text.as_bytes());
    Ok(ProgramDisassembly {
        format: DISASSEMBLY_FORMAT.to_owned(),
        instruction_count: instructions.len() as i64,
        entrypoint_instruction: entrypoint_instruction.map(|value| value as i64),
        asm_text,
        asm_sha256,
    })
}

fn render_asm_text(lines: &[String], labels: &BTreeMap<usize, Vec<String>>) -> String {
    let mut asm_text = String::new();
    for (index, asm) in lines.iter().enumerate() {
        if let Some(labels) = labels.get(&index) {
            for label in labels {
                let _ = writeln!(asm_text, "; {label}");
            }
        }
        let _ = writeln!(asm_text, "{index:06}: {asm}");
    }
    asm_text
}

fn infer_anchor_labels(
    lines: &[String],
    known_instructions: &[KnownProgramInstruction],
) -> BTreeMap<usize, Vec<String>> {
    if known_instructions.is_empty() {
        return BTreeMap::new();
    }

    let known_by_discriminator = known_instructions
        .iter()
        .map(|instruction| {
            (
                u64::from_le_bytes(instruction.discriminator),
                instruction.name.as_str(),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut handlers = BTreeMap::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(discriminator) = parse_immediate_assignment(line) else {
            continue;
        };
        let Some(name) = known_by_discriminator.get(&discriminator).copied() else {
            continue;
        };
        let Some(branch_line) = lines.get(index + 1) else {
            continue;
        };
        let Some(stub_target) = parse_goto_target(index + 1, branch_line) else {
            continue;
        };
        let Some(handler) = first_call_target(lines, stub_target, 12) else {
            continue;
        };
        handlers.insert(handler, name.to_owned());
    }

    let mut labels = BTreeMap::new();
    for (handler, name) in &handlers {
        labels
            .entry(*handler)
            .or_insert_with(Vec::new)
            .push(format!("handler: {name}"));
    }

    let helper_labels = infer_helper_reachability(lines, &handlers);
    for (function_start, reachable_from) in helper_labels {
        labels
            .entry(function_start)
            .or_insert_with(Vec::new)
            .push(format!(
                "helper reachable from: {}",
                reachable_from.join(", ")
            ));
    }

    labels
}

fn infer_helper_reachability(
    lines: &[String],
    handlers: &BTreeMap<usize, String>,
) -> BTreeMap<usize, Vec<String>> {
    let call_targets = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| parse_call_target(index, line).map(|target| (index, target)))
        .filter(|(_, target)| *target < lines.len())
        .collect::<Vec<_>>();

    let mut function_starts = BTreeSet::new();
    function_starts.insert(0);
    function_starts.extend(handlers.keys().copied());
    function_starts.extend(call_targets.iter().map(|(_, target)| *target));
    let function_starts = function_starts.into_iter().collect::<Vec<_>>();
    let function_start_set = function_starts.iter().copied().collect::<HashSet<_>>();

    let mut edges = function_starts
        .iter()
        .map(|start| (*start, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (call_site, target) in call_targets {
        if !function_start_set.contains(&target) {
            continue;
        }
        if let Some(function_start) = containing_function_start(&function_starts, call_site) {
            edges.entry(function_start).or_default().insert(target);
        }
    }

    let mut reachable = BTreeMap::<usize, BTreeSet<String>>::new();
    for (handler_start, handler_name) in handlers {
        let mut stack = vec![*handler_start];
        let mut seen = HashSet::new();

        while let Some(function_start) = stack.pop() {
            if !seen.insert(function_start) || seen.len() > 10_000 {
                continue;
            }

            reachable
                .entry(function_start)
                .or_default()
                .insert(handler_name.clone());

            if let Some(targets) = edges.get(&function_start) {
                for target in targets.iter().rev() {
                    stack.push(*target);
                }
            }
        }
    }

    reachable
        .into_iter()
        .filter_map(|(function_start, names)| {
            if handlers.contains_key(&function_start) || names.len() < 2 {
                return None;
            }
            focused_reachable_names(names).map(|names| (function_start, names))
        })
        .collect()
}

fn focused_reachable_names(names: BTreeSet<String>) -> Option<Vec<String>> {
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort();

    let trade_names = names
        .iter()
        .filter(|name| is_trade_instruction_name(name))
        .cloned()
        .collect::<Vec<_>>();
    if trade_names.len() >= 2 {
        return Some(trade_names);
    }

    if names.len() <= 6 {
        return Some(names);
    }

    None
}

fn is_trade_instruction_name(name: &str) -> bool {
    name == "buy"
        || name == "sell"
        || name.starts_with("buy_")
        || name.starts_with("sell_")
        || name.contains("_swap")
        || name.contains("swap_")
}

fn containing_function_start(function_starts: &[usize], instruction_index: usize) -> Option<usize> {
    let index = function_starts
        .binary_search(&instruction_index)
        .unwrap_or_else(|index| index.saturating_sub(1));
    function_starts.get(index).copied()
}

fn first_call_target(lines: &[String], start: usize, search_len: usize) -> Option<usize> {
    let end = start.saturating_add(search_len).min(lines.len());
    (start..end).find_map(|index| parse_call_target(index, &lines[index]))
}

fn parse_call_target(instruction_index: usize, line: &str) -> Option<usize> {
    let immediate = line.strip_prefix("call ")?;
    let offset = parse_signed_immediate(immediate)?;
    relative_target(instruction_index, offset)
}

fn parse_goto_target(instruction_index: usize, line: &str) -> Option<usize> {
    let (_, immediate) = line.rsplit_once(" goto ")?;
    let offset = parse_signed_immediate(immediate)?;
    relative_target(instruction_index, offset)
}

fn relative_target(instruction_index: usize, offset: i64) -> Option<usize> {
    let target = instruction_index as i64 + 1 + offset;
    if target < 0 {
        return None;
    }
    Some(target as usize)
}

fn parse_immediate_assignment(line: &str) -> Option<u64> {
    let (_, immediate) = line.split_once(" = ")?;
    let immediate = immediate.strip_suffix(" ll")?;
    Some(parse_signed_immediate(immediate)? as u64)
}

fn parse_signed_immediate(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if let Some(hex) = raw.strip_prefix("-0x") {
        return i64::from_str_radix(hex, 16).ok().map(|value| -value);
    }
    if let Some(hex) = raw.strip_prefix("+0x") {
        return i64::from_str_radix(hex, 16).ok();
    }
    if let Some(hex) = raw.strip_prefix("0x") {
        return i64::from_str_radix(hex, 16).ok();
    }
    raw.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(name: &str, discriminator: [u8; 8]) -> KnownProgramInstruction {
        KnownProgramInstruction {
            name: name.to_owned(),
            discriminator,
        }
    }

    #[test]
    fn labels_anchor_handlers_and_shared_helpers() {
        let lines = vec![
            "r7 = 0x807060504030201 ll".to_owned(),
            "if r0 == r7 goto +0x1".to_owned(),
            "exit".to_owned(),
            "call 0x6".to_owned(),
            "exit".to_owned(),
            "r7 = -0x7f8f9fafbfcfdff0 ll".to_owned(),
            "if r0 == r7 goto +0x1".to_owned(),
            "exit".to_owned(),
            "call 0x4".to_owned(),
            "exit".to_owned(),
            "call 0x4".to_owned(),
            "exit".to_owned(),
            "exit".to_owned(),
            "call 0x1".to_owned(),
            "exit".to_owned(),
            "exit".to_owned(),
        ];

        let labels = infer_anchor_labels(
            &lines,
            &[
                known("first", [1, 2, 3, 4, 5, 6, 7, 8]),
                known("second", [16, 32, 48, 64, 80, 96, 112, 128]),
            ],
        );

        assert_eq!(labels.get(&10), Some(&vec!["handler: first".to_owned()]));
        assert_eq!(labels.get(&13), Some(&vec!["handler: second".to_owned()]));
        assert_eq!(
            labels.get(&15),
            Some(&vec!["helper reachable from: first, second".to_owned()])
        );
    }
}
