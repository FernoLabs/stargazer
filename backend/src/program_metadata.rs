use solana_sdk::pubkey::Pubkey;
use std::{str::FromStr, sync::OnceLock};

pub const PROGRAM_METADATA_PROGRAM_ID: &str = "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S";
pub const CANONICAL_IDL_SEED: &str = "idl";
pub const SEED_LEN: usize = 16;
#[allow(dead_code)]
const HEADER_LEN: usize = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramMetadataInstructionKind {
    Write,
    Initialize,
    SetAuthority,
    SetData,
    SetImmutable,
    Trim,
    Close,
    Allocate,
    Extend,
}

impl ProgramMetadataInstructionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Initialize => "initialize",
            Self::SetAuthority => "set_authority",
            Self::SetData => "set_data",
            Self::SetImmutable => "set_immutable",
            Self::Trim => "trim",
            Self::Close => "close",
            Self::Allocate => "allocate",
            Self::Extend => "extend",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataEncoding {
    None,
    Utf8,
    Base58,
    Base64,
}

impl MetadataEncoding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Utf8 => "utf8",
            Self::Base58 => "base58",
            Self::Base64 => "base64",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataCompression {
    None,
    Gzip,
    Zlib,
}

impl MetadataCompression {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Gzip => "gzip",
            Self::Zlib => "zlib",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataFormat {
    None,
    Json,
    Yaml,
    Toml,
}

impl MetadataFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataDataSource {
    Direct,
    Url,
    External,
}

impl MetadataDataSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Url => "url",
            Self::External => "external",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedProgramMetadataInstruction {
    pub kind: ProgramMetadataInstructionKind,
    pub seed: Option<String>,
    pub new_authority: Option<Option<String>>,
    pub encoding: Option<MetadataEncoding>,
    pub compression: Option<MetadataCompression>,
    pub format: Option<MetadataFormat>,
    pub data_source: Option<MetadataDataSource>,
    pub inline_data_len: Option<usize>,
    pub offset: Option<u32>,
    pub length: Option<u16>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedMetadataHeader {
    pub program: Pubkey,
    pub authority: Option<Pubkey>,
    pub mutable: bool,
    pub canonical: bool,
    pub seed: String,
    pub encoding: MetadataEncoding,
    pub compression: MetadataCompression,
    pub format: MetadataFormat,
    pub data_source: MetadataDataSource,
    pub data_length: u32,
}

pub fn program_id() -> Pubkey {
    static PROGRAM_ID: OnceLock<Pubkey> = OnceLock::new();
    *PROGRAM_ID.get_or_init(|| {
        Pubkey::from_str(PROGRAM_METADATA_PROGRAM_ID).expect("valid program metadata program id")
    })
}

pub fn seed_bytes(seed: &str) -> [u8; SEED_LEN] {
    let mut output = [0_u8; SEED_LEN];
    let bytes = seed.as_bytes();
    let length = bytes.len().min(SEED_LEN);
    output[..length].copy_from_slice(&bytes[..length]);
    output
}

pub fn seed_string(seed_bytes: &[u8]) -> String {
    let end = seed_bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(seed_bytes.len());
    String::from_utf8_lossy(&seed_bytes[..end]).to_string()
}

pub fn canonical_metadata_address(program: Pubkey, seed: &str) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref(), &seed_bytes(seed)], &program_id()).0
}

pub fn canonical_idl_metadata_address(program: Pubkey) -> Pubkey {
    canonical_metadata_address(program, CANONICAL_IDL_SEED)
}

pub fn canonical_idl_metadata_addresses(programs: &[Pubkey]) -> Vec<Pubkey> {
    programs
        .iter()
        .map(|program| canonical_idl_metadata_address(*program))
        .collect()
}

pub fn parse_instruction(data: &[u8]) -> Option<ParsedProgramMetadataInstruction> {
    let discriminator = *data.first()?;
    match discriminator {
        0 => Some(ParsedProgramMetadataInstruction {
            kind: ProgramMetadataInstructionKind::Write,
            seed: None,
            new_authority: None,
            encoding: None,
            compression: None,
            format: None,
            data_source: None,
            inline_data_len: data.len().checked_sub(5).filter(|len| *len > 0),
            offset: Some(read_u32(data.get(1..5)?)?),
            length: None,
        }),
        1 => Some(ParsedProgramMetadataInstruction {
            kind: ProgramMetadataInstructionKind::Initialize,
            seed: Some(seed_string(data.get(1..17)?)),
            new_authority: None,
            encoding: Some(parse_encoding(*data.get(17)?)?),
            compression: Some(parse_compression(*data.get(18)?)?),
            format: Some(parse_format(*data.get(19)?)?),
            data_source: Some(parse_data_source(*data.get(20)?)?),
            inline_data_len: data.len().checked_sub(21).filter(|len| *len > 0),
            offset: None,
            length: None,
        }),
        2 => Some(ParsedProgramMetadataInstruction {
            kind: ProgramMetadataInstructionKind::SetAuthority,
            seed: None,
            new_authority: Some(parse_option_pubkey(data.get(1..)?)?),
            encoding: None,
            compression: None,
            format: None,
            data_source: None,
            inline_data_len: None,
            offset: None,
            length: None,
        }),
        3 => Some(ParsedProgramMetadataInstruction {
            kind: ProgramMetadataInstructionKind::SetData,
            seed: None,
            new_authority: None,
            encoding: Some(parse_encoding(*data.get(1)?)?),
            compression: Some(parse_compression(*data.get(2)?)?),
            format: Some(parse_format(*data.get(3)?)?),
            data_source: Some(parse_data_source(*data.get(4)?)?),
            inline_data_len: data.len().checked_sub(5).filter(|len| *len > 0),
            offset: None,
            length: None,
        }),
        4 => Some(ParsedProgramMetadataInstruction {
            kind: ProgramMetadataInstructionKind::SetImmutable,
            seed: None,
            new_authority: None,
            encoding: None,
            compression: None,
            format: None,
            data_source: None,
            inline_data_len: None,
            offset: None,
            length: None,
        }),
        5 => Some(ParsedProgramMetadataInstruction {
            kind: ProgramMetadataInstructionKind::Trim,
            seed: None,
            new_authority: None,
            encoding: None,
            compression: None,
            format: None,
            data_source: None,
            inline_data_len: None,
            offset: None,
            length: None,
        }),
        6 => Some(ParsedProgramMetadataInstruction {
            kind: ProgramMetadataInstructionKind::Close,
            seed: None,
            new_authority: None,
            encoding: None,
            compression: None,
            format: None,
            data_source: None,
            inline_data_len: None,
            offset: None,
            length: None,
        }),
        7 => Some(ParsedProgramMetadataInstruction {
            kind: ProgramMetadataInstructionKind::Allocate,
            seed: data
                .get(1..)
                .filter(|bytes| !bytes.is_empty())
                .and_then(|bytes| bytes.get(..SEED_LEN))
                .map(seed_string)
                .filter(|seed| !seed.is_empty()),
            new_authority: None,
            encoding: None,
            compression: None,
            format: None,
            data_source: None,
            inline_data_len: None,
            offset: None,
            length: None,
        }),
        8 => Some(ParsedProgramMetadataInstruction {
            kind: ProgramMetadataInstructionKind::Extend,
            seed: None,
            new_authority: None,
            encoding: None,
            compression: None,
            format: None,
            data_source: None,
            inline_data_len: None,
            offset: None,
            length: Some(read_u16(data.get(1..3)?)?),
        }),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn parse_metadata_header(bytes: &[u8]) -> Option<ParsedMetadataHeader> {
    if bytes.len() < HEADER_LEN || bytes.first().copied()? != 2 {
        return None;
    }

    let program = Pubkey::try_from(bytes.get(1..33)?).ok()?;
    let authority_slice = bytes.get(33..65)?;
    let authority = if authority_slice.iter().all(|byte| *byte == 0) {
        None
    } else {
        Some(Pubkey::try_from(authority_slice).ok()?)
    };

    Some(ParsedMetadataHeader {
        program,
        authority,
        mutable: *bytes.get(65)? != 0,
        canonical: *bytes.get(66)? != 0,
        seed: seed_string(bytes.get(67..83)?),
        encoding: parse_encoding(*bytes.get(83)?)?,
        compression: parse_compression(*bytes.get(84)?)?,
        format: parse_format(*bytes.get(85)?)?,
        data_source: parse_data_source(*bytes.get(86)?)?,
        data_length: read_u32(bytes.get(87..91)?)?,
    })
}

fn read_u16(bytes: &[u8]) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn parse_encoding(value: u8) -> Option<MetadataEncoding> {
    match value {
        0 => Some(MetadataEncoding::None),
        1 => Some(MetadataEncoding::Utf8),
        2 => Some(MetadataEncoding::Base58),
        3 => Some(MetadataEncoding::Base64),
        _ => None,
    }
}

fn parse_compression(value: u8) -> Option<MetadataCompression> {
    match value {
        0 => Some(MetadataCompression::None),
        1 => Some(MetadataCompression::Gzip),
        2 => Some(MetadataCompression::Zlib),
        _ => None,
    }
}

fn parse_format(value: u8) -> Option<MetadataFormat> {
    match value {
        0 => Some(MetadataFormat::None),
        1 => Some(MetadataFormat::Json),
        2 => Some(MetadataFormat::Yaml),
        3 => Some(MetadataFormat::Toml),
        _ => None,
    }
}

fn parse_data_source(value: u8) -> Option<MetadataDataSource> {
    match value {
        0 => Some(MetadataDataSource::Direct),
        1 => Some(MetadataDataSource::Url),
        2 => Some(MetadataDataSource::External),
        _ => None,
    }
}

fn parse_option_pubkey(bytes: &[u8]) -> Option<Option<String>> {
    match bytes.first().copied()? {
        0 => Some(None),
        1 => Some(Some(Pubkey::try_from(bytes.get(1..33)?).ok()?.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_seed_bytes_to_program_metadata_length() {
        let seed = seed_bytes(CANONICAL_IDL_SEED);

        assert_eq!(seed[..3], *b"idl");
        assert!(seed[3..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn parses_initialize_instruction_shape() {
        let mut data = vec![1];
        data.extend_from_slice(&seed_bytes(CANONICAL_IDL_SEED));
        data.extend_from_slice(&[1, 2, 1, 0]);
        data.extend_from_slice(br#"{"name":"demo"}"#);

        let parsed = parse_instruction(&data).expect("initialize instruction");
        assert_eq!(parsed.kind, ProgramMetadataInstructionKind::Initialize);
        assert_eq!(parsed.seed.as_deref(), Some(CANONICAL_IDL_SEED));
        assert_eq!(parsed.encoding, Some(MetadataEncoding::Utf8));
        assert_eq!(parsed.compression, Some(MetadataCompression::Zlib));
        assert_eq!(parsed.format, Some(MetadataFormat::Json));
        assert_eq!(parsed.data_source, Some(MetadataDataSource::Direct));
        assert_eq!(parsed.inline_data_len, Some(br#"{"name":"demo"}"#.len()));
    }

    #[test]
    fn parses_set_authority_instruction_shape() {
        let authority = Pubkey::new_unique();
        let mut data = vec![2, 1];
        data.extend_from_slice(authority.as_ref());

        let parsed = parse_instruction(&data).expect("set authority instruction");
        assert_eq!(parsed.kind, ProgramMetadataInstructionKind::SetAuthority);
        assert_eq!(parsed.new_authority, Some(Some(authority.to_string())));
    }

    #[test]
    fn parses_metadata_header_bytes() {
        let program = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let mut header = [0_u8; HEADER_LEN];
        header[0] = 2;
        header[1..33].copy_from_slice(program.as_ref());
        header[33..65].copy_from_slice(authority.as_ref());
        header[65] = 1;
        header[66] = 1;
        header[67..83].copy_from_slice(&seed_bytes(CANONICAL_IDL_SEED));
        header[83] = 1;
        header[84] = 2;
        header[85] = 1;
        header[86] = 0;
        header[87..91].copy_from_slice(&1234_u32.to_le_bytes());

        let parsed = parse_metadata_header(&header).expect("metadata header");
        assert_eq!(parsed.program, program);
        assert_eq!(parsed.authority, Some(authority));
        assert!(parsed.mutable);
        assert!(parsed.canonical);
        assert_eq!(parsed.seed, CANONICAL_IDL_SEED);
        assert_eq!(parsed.encoding, MetadataEncoding::Utf8);
        assert_eq!(parsed.compression, MetadataCompression::Zlib);
        assert_eq!(parsed.format, MetadataFormat::Json);
        assert_eq!(parsed.data_source, MetadataDataSource::Direct);
        assert_eq!(parsed.data_length, 1234);
    }
}
