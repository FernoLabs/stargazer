use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const CATALOG_JSON: &str = include_str!("../data/idl_instruction_catalog.json");

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InstructionCatalog {
    pub version: u32,
    pub summary: CatalogSummary,
    pub programs: Vec<ProgramDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CatalogSummary {
    pub program_count: usize,
    pub instruction_count: usize,
    pub filterable_argument_count: usize,
    pub unresolved_discriminator_count: usize,
    pub confidence_counts: ConfidenceCounts,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ConfidenceCounts {
    pub high_confidence_admin: usize,
    pub likely_admin: usize,
    pub manual_review: usize,
    pub not_admin: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProgramDescriptor {
    pub program_id: String,
    pub protocol_slug: String,
    pub protocol_name: String,
    pub program_label: String,
    pub idl_name: String,
    pub idl_origin: String,
    pub idl_schema: String,
    pub idl_source: String,
    pub idl_sha256: String,
    pub instruction_count: usize,
    pub admin_candidate_count: usize,
    pub filterable_argument_count: usize,
    pub instructions: Vec<InstructionDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProgramSummary {
    pub program_id: String,
    pub protocol_slug: String,
    pub protocol_name: String,
    pub program_label: String,
    pub idl_name: String,
    pub idl_origin: String,
    pub idl_schema: String,
    pub idl_source: String,
    pub idl_sha256: String,
    pub instruction_count: usize,
    pub admin_candidate_count: usize,
    pub filterable_argument_count: usize,
}

impl From<&ProgramDescriptor> for ProgramSummary {
    fn from(program: &ProgramDescriptor) -> Self {
        Self {
            program_id: program.program_id.clone(),
            protocol_slug: program.protocol_slug.clone(),
            protocol_name: program.protocol_name.clone(),
            program_label: program.program_label.clone(),
            idl_name: program.idl_name.clone(),
            idl_origin: program.idl_origin.clone(),
            idl_schema: program.idl_schema.clone(),
            idl_source: program.idl_source.clone(),
            idl_sha256: program.idl_sha256.clone(),
            instruction_count: program.instruction_count,
            admin_candidate_count: program.admin_candidate_count,
            filterable_argument_count: program.filterable_argument_count,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InstructionDescriptor {
    pub name: String,
    pub raw_name: String,
    pub discriminator_hex: String,
    pub discriminator_offset: usize,
    pub discriminator_source: String,
    pub admin_confidence: AdminConfidence,
    pub admin_score: u32,
    pub recommended_severity: Option<RecommendedSeverity>,
    pub admin_reasons: Vec<String>,
    pub accounts: Vec<AccountDescriptor>,
    pub arguments: Vec<ArgumentDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AccountDescriptor {
    pub index: usize,
    pub name: String,
    pub is_signer: bool,
    pub is_writable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ArgumentDescriptor {
    pub index: usize,
    pub name: String,
    pub type_name: String,
    pub type_category: TypeCategory,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_kind: Option<ValueKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit_width: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endianness: Option<Endianness>,
    pub filterable: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdminConfidence {
    HighConfidenceAdmin,
    LikelyAdmin,
    ManualReview,
    NotAdmin,
}

impl AdminConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HighConfidenceAdmin => "high_confidence_admin",
            Self::LikelyAdmin => "likely_admin",
            Self::ManualReview => "manual_review",
            Self::NotAdmin => "not_admin",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedSeverity {
    High,
    Critical,
}

impl RecommendedSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TypeCategory {
    Integer,
    Float,
    Pubkey,
    Boolean,
    String,
    Optional,
    Collection,
    Defined,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Integer,
    Boolean,
    Pubkey,
    Float,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Endianness {
    Little,
    Big,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum DecodedValue {
    Integer(String),
    Boolean(bool),
    Pubkey(String),
    Float(String),
}

impl DecodedValue {
    pub fn as_integer_str(&self) -> Option<&str> {
        match self {
            Self::Integer(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_pubkey(&self) -> Option<&str> {
        match self {
            Self::Pubkey(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DecodedInstruction {
    pub program_id: String,
    pub protocol_slug: String,
    pub protocol_name: String,
    pub program_label: String,
    pub instruction_name: String,
    pub raw_instruction_name: String,
    pub discriminator_hex: String,
    pub admin_confidence: AdminConfidence,
    pub recommended_severity: Option<RecommendedSeverity>,
    pub admin_reasons: Vec<String>,
    pub arguments: BTreeMap<String, DecodedValue>,
    pub accounts: BTreeMap<String, String>,
}

pub fn instruction_catalog() -> &'static InstructionCatalog {
    static CATALOG: OnceLock<InstructionCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(CATALOG_JSON)
            .expect("the embedded IDL instruction catalog must be valid JSON")
    })
}

pub fn program_summaries() -> &'static [ProgramSummary] {
    static SUMMARIES: OnceLock<Vec<ProgramSummary>> = OnceLock::new();
    SUMMARIES
        .get_or_init(|| {
            instruction_catalog()
                .programs
                .iter()
                .map(ProgramSummary::from)
                .collect()
        })
        .as_slice()
}

pub fn program_descriptor(program_id: &str) -> Option<&'static ProgramDescriptor> {
    let catalog = instruction_catalog();
    let index = catalog
        .programs
        .binary_search_by(|program| program.program_id.as_str().cmp(program_id))
        .ok()?;
    catalog.programs.get(index)
}

pub fn instruction_descriptor(
    program_id: &str,
    instruction_name: &str,
) -> Option<&'static InstructionDescriptor> {
    program_descriptor(program_id)?
        .instructions
        .iter()
        .find(|instruction| instruction.name == instruction_name)
}

/// Decode one instruction using account keys in the instruction's local order.
///
/// The caller should resolve compiled account indices before calling this
/// function. Missing or malformed bytes never panic: a matching instruction is
/// returned with only the scalar fields that could be decoded safely.
pub fn decode_instruction<T: ToString>(
    program_id: &str,
    data: &[u8],
    instruction_accounts: &[T],
) -> Option<DecodedInstruction> {
    let program = program_descriptor(program_id)?;
    let instruction = program
        .instructions
        .iter()
        .filter_map(|instruction| {
            let discriminator = hex::decode(&instruction.discriminator_hex).ok()?;
            if discriminator.is_empty() {
                return None;
            }
            let end = instruction
                .discriminator_offset
                .checked_add(discriminator.len())?;
            (data.get(instruction.discriminator_offset..end) == Some(discriminator.as_slice()))
                .then_some((instruction, discriminator.len()))
        })
        .max_by_key(|(_, discriminator_length)| *discriminator_length)
        .map(|(instruction, _)| instruction)?;

    let accounts = instruction
        .accounts
        .iter()
        .filter_map(|account| {
            let value = instruction_accounts.get(account.index)?;
            (!account.name.is_empty()).then(|| (account.name.clone(), value.to_string()))
        })
        .collect();
    let arguments = instruction
        .arguments
        .iter()
        .filter_map(|argument| {
            decode_argument(argument, data).map(|value| (argument.name.clone(), value))
        })
        .collect();

    Some(DecodedInstruction {
        program_id: program.program_id.clone(),
        protocol_slug: program.protocol_slug.clone(),
        protocol_name: program.protocol_name.clone(),
        program_label: program.program_label.clone(),
        instruction_name: instruction.name.clone(),
        raw_instruction_name: instruction.raw_name.clone(),
        discriminator_hex: instruction.discriminator_hex.clone(),
        admin_confidence: instruction.admin_confidence,
        recommended_severity: instruction.recommended_severity,
        admin_reasons: instruction.admin_reasons.clone(),
        arguments,
        accounts,
    })
}

fn decode_argument(argument: &ArgumentDescriptor, data: &[u8]) -> Option<DecodedValue> {
    let value_kind = argument.value_kind?;
    let offset = argument.byte_offset?;
    let byte_length = argument.byte_length?;
    let end = offset.checked_add(byte_length)?;
    let bytes = data.get(offset..end)?;

    match value_kind {
        ValueKind::Integer => Some(DecodedValue::Integer(decode_integer(
            bytes,
            argument.signed?,
            argument.bit_width?,
            argument.endianness.unwrap_or(Endianness::Little),
        )?)),
        ValueKind::Boolean => match bytes {
            [0] => Some(DecodedValue::Boolean(false)),
            [1] => Some(DecodedValue::Boolean(true)),
            _ => None,
        },
        ValueKind::Pubkey if bytes.len() == 32 => {
            Some(DecodedValue::Pubkey(bs58::encode(bytes).into_string()))
        }
        ValueKind::Pubkey => None,
        ValueKind::Float => Some(DecodedValue::Float(decode_float(
            bytes,
            argument.bit_width?,
            argument.endianness.unwrap_or(Endianness::Little),
        )?)),
    }
}

fn decode_integer(
    bytes: &[u8],
    signed: bool,
    bit_width: u16,
    endianness: Endianness,
) -> Option<String> {
    macro_rules! decode_number {
        ($number_type:ty, $byte_count:literal) => {{
            let array: [u8; $byte_count] = bytes.try_into().ok()?;
            let value = match endianness {
                Endianness::Little => <$number_type>::from_le_bytes(array),
                Endianness::Big => <$number_type>::from_be_bytes(array),
            };
            Some(value.to_string())
        }};
    }

    match (signed, bit_width) {
        (false, 8) => decode_number!(u8, 1),
        (false, 16) => decode_number!(u16, 2),
        (false, 32) => decode_number!(u32, 4),
        (false, 64) => decode_number!(u64, 8),
        (false, 128) => decode_number!(u128, 16),
        (true, 8) => decode_number!(i8, 1),
        (true, 16) => decode_number!(i16, 2),
        (true, 32) => decode_number!(i32, 4),
        (true, 64) => decode_number!(i64, 8),
        (true, 128) => decode_number!(i128, 16),
        _ => None,
    }
}

fn decode_float(bytes: &[u8], bit_width: u16, endianness: Endianness) -> Option<String> {
    macro_rules! decode_number {
        ($number_type:ty, $byte_count:literal) => {{
            let array: [u8; $byte_count] = bytes.try_into().ok()?;
            let value = match endianness {
                Endianness::Little => <$number_type>::from_le_bytes(array),
                Endianness::Big => <$number_type>::from_be_bytes(array),
            };
            Some(value.to_string())
        }};
    }

    match bit_width {
        32 => decode_number!(f32, 4),
        64 => decode_number!(f64, 8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_has_expected_coverage() {
        let catalog = instruction_catalog();
        assert_eq!(catalog.version, 1);
        assert_eq!(catalog.summary.program_count, 259);
        assert_eq!(catalog.summary.instruction_count, 5_882);
        assert_eq!(catalog.summary.unresolved_discriminator_count, 0);
        assert_eq!(catalog.summary.confidence_counts.high_confidence_admin, 261);
        assert_eq!(program_summaries().len(), 259);
        assert!(
            catalog
                .programs
                .windows(2)
                .all(|pair| { pair[0].program_id.as_str() < pair[1].program_id.as_str() })
        );
        assert!(
            catalog
                .programs
                .iter()
                .flat_map(|program| &program.instructions)
                .flat_map(|instruction| &instruction.arguments)
                .all(|argument| {
                    !argument.filterable
                        || matches!(
                            argument.value_kind,
                            Some(ValueKind::Integer | ValueKind::Boolean | ValueKind::Pubkey)
                        ) && argument.byte_offset.is_some()
                })
        );
        for instruction in catalog
            .programs
            .iter()
            .flat_map(|program| &program.instructions)
        {
            let names = instruction
                .accounts
                .iter()
                .map(|account| account.name.as_str())
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(names.len(), instruction.accounts.len());
        }
    }

    #[test]
    fn decodes_anchor_integer_and_named_accounts() {
        let mut data = hex::decode("ed331203e6fe695c").unwrap();
        data.extend_from_slice(&42_u64.to_le_bytes());
        let accounts = vec!["admin-address", "state-address"];
        let decoded = decode_instruction(
            "1349iiGjWC7ZTbu6otFmJwztms122jEEnShKgpVnNewy",
            &data,
            &accounts,
        )
        .unwrap();

        assert_eq!(decoded.instruction_name, "buy_underlying");
        assert_eq!(
            decoded.arguments.get("limit_price"),
            Some(&DecodedValue::Integer("42".to_owned()))
        );
        assert_eq!(
            decoded.accounts.get("admin").map(String::as_str),
            Some("admin-address")
        );
    }

    #[test]
    fn decodes_codama_discriminator_and_integer() {
        let data = [0x1c, 0x34, 0x12];
        let accounts = vec!["authority-address", "central-state-address"];
        let decoded = decode_instruction(
            "6HW8dXjtiTGkD4jzXs7igdFmZExPpmwUrRN5195xGup",
            &data,
            &accounts,
        )
        .unwrap();

        assert_eq!(decoded.instruction_name, "admin_set_protocol_fee");
        assert_eq!(
            decoded.arguments.get("protocol_fee_basis_points"),
            Some(&DecodedValue::Integer("4660".to_owned()))
        );
        assert_eq!(
            decoded.recommended_severity,
            Some(RecommendedSeverity::Critical)
        );
    }

    #[test]
    fn malformed_data_never_requires_complete_arguments() {
        assert!(decode_instruction::<String>("not-a-program", &[], &[]).is_none());
        assert!(
            decode_instruction::<String>(
                "1349iiGjWC7ZTbu6otFmJwztms122jEEnShKgpVnNewy",
                &[0, 1, 2],
                &[],
            )
            .is_none()
        );

        let discriminator_only = hex::decode("ed331203e6fe695c").unwrap();
        let decoded = decode_instruction::<String>(
            "1349iiGjWC7ZTbu6otFmJwztms122jEEnShKgpVnNewy",
            &discriminator_only,
            &[],
        )
        .unwrap();
        assert!(decoded.arguments.is_empty());
        assert!(decoded.accounts.is_empty());
    }

    #[test]
    fn integer_decoder_preserves_128_bit_values_exactly() {
        assert_eq!(
            decode_integer(&u128::MAX.to_le_bytes(), false, 128, Endianness::Little,).as_deref(),
            Some("340282366920938463463374607431768211455")
        );
        assert_eq!(
            decode_integer(&i128::MIN.to_be_bytes(), true, 128, Endianness::Big,).as_deref(),
            Some("-170141183460469231731687303715884105728")
        );
    }
}
