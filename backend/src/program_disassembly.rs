use crate::program_bytes::sha256_hex;
use anyhow::{Context, Result};
use either::Either;
use sbpf_common::{
    inst_param::Number,
    instruction::{AsmFormat, Instruction},
    opcode::Opcode,
};
use sbpf_disassembler::program::Program;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write as _,
};

pub(crate) const DISASSEMBLY_FORMAT: &str = "llvm+anchor-handler-labels-v2";

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
        let asm = format_instruction_asm(instruction)
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

fn format_instruction_asm(instruction: &Instruction) -> Result<String> {
    if has_min_immediate(instruction) {
        return Ok(fallback_instruction_asm(instruction));
    }
    instruction
        .to_asm(AsmFormat::Llvm)
        .context("format LLVM assembly")
}

fn has_min_immediate(instruction: &Instruction) -> bool {
    matches!(
        instruction.imm,
        Some(Either::Right(
            Number::Int(i64::MIN) | Number::Addr(i64::MIN)
        ))
    )
}

fn fallback_instruction_asm(instruction: &Instruction) -> String {
    if instruction.opcode == Opcode::Lddw {
        let dst = instruction
            .dst
            .as_ref()
            .map(|register| register.n)
            .unwrap_or(0);
        let imm = instruction
            .imm
            .as_ref()
            .map(format_immediate)
            .unwrap_or_else(|| "0x0".to_owned());
        return format!("r{dst} = {imm} ll");
    }

    format!("{instruction:?}")
}

fn format_immediate(imm: &Either<String, Number>) -> String {
    match imm {
        Either::Left(label) => label.clone(),
        Either::Right(Number::Int(value) | Number::Addr(value)) if *value < 0 => {
            format!("-0x{:x}", value.unsigned_abs())
        }
        Either::Right(Number::Int(value) | Number::Addr(value)) => format!("0x{value:x}"),
    }
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

    labels
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
        let value = u64::from_str_radix(hex, 16).ok()?;
        if value == 1_u64 << 63 {
            return Some(i64::MIN);
        }
        return i64::try_from(value).ok().map(|value| -value);
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
    use sbpf_common::inst_param::Register;

    fn known(name: &str, discriminator: [u8; 8]) -> KnownProgramInstruction {
        KnownProgramInstruction {
            name: name.to_owned(),
            discriminator,
        }
    }

    #[test]
    fn labels_anchor_handlers() {
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
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn parses_minimum_negative_hex_immediate() {
        assert_eq!(
            parse_signed_immediate("-0x8000000000000000"),
            Some(i64::MIN)
        );
    }

    #[test]
    fn formats_minimum_negative_load_immediate() {
        let instruction = Instruction {
            opcode: Opcode::Lddw,
            dst: Some(Register { n: 7 }),
            src: None,
            off: None,
            imm: Some(Either::Right(Number::Int(i64::MIN))),
            span: 0..16,
        };

        assert_eq!(
            format_instruction_asm(&instruction).expect("format fallback"),
            "r7 = -0x8000000000000000 ll"
        );
    }
}
