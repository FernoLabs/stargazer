use solana_sdk::pubkey::Pubkey;

pub const LEGACY_ANCHOR_IDL_SEED: &str = "anchor:idl";
pub const LEGACY_ANCHOR_IDL_ORIGIN: &str = "anchor_legacy";
pub const PROGRAM_METADATA_IDL_ORIGIN: &str = "program_metadata";
pub const IDL_IX_TAG: u64 = 0x0a69e9a778bcf440;
pub const IDL_IX_TAG_LE: [u8; 8] = IDL_IX_TAG.to_le_bytes();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorIdlInstructionKind {
    Create,
    CreateBuffer,
    Write,
    SetBuffer,
    SetAuthority,
    Close,
    Resize,
}

impl AnchorIdlInstructionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::CreateBuffer => "create_buffer",
            Self::Write => "write",
            Self::SetBuffer => "set_buffer",
            Self::SetAuthority => "set_authority",
            Self::Close => "close",
            Self::Resize => "resize",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedAnchorIdlInstruction {
    pub kind: AnchorIdlInstructionKind,
    pub data_len: Option<u64>,
    pub write_data: Option<Vec<u8>>,
    pub new_authority: Option<String>,
}

pub fn legacy_idl_address(program: Pubkey) -> Option<Pubkey> {
    let program_signer = Pubkey::find_program_address(&[], &program).0;
    Pubkey::create_with_seed(&program_signer, LEGACY_ANCHOR_IDL_SEED, &program).ok()
}

pub fn legacy_idl_addresses(programs: &[Pubkey]) -> Vec<Pubkey> {
    programs
        .iter()
        .filter_map(|program| legacy_idl_address(*program))
        .collect()
}

pub fn parse_instruction(data: &[u8]) -> Option<ParsedAnchorIdlInstruction> {
    if data.get(..8)? != IDL_IX_TAG_LE {
        return None;
    }

    let variant = *data.get(8)?;
    match variant {
        0 => Some(ParsedAnchorIdlInstruction {
            kind: AnchorIdlInstructionKind::Create,
            data_len: Some(read_u64(data.get(9..17)?)?),
            write_data: None,
            new_authority: None,
        }),
        1 => Some(ParsedAnchorIdlInstruction {
            kind: AnchorIdlInstructionKind::CreateBuffer,
            data_len: None,
            write_data: None,
            new_authority: None,
        }),
        2 => {
            let len = read_u32(data.get(9..13)?)? as usize;
            let bytes = data.get(13..13 + len)?.to_vec();
            Some(ParsedAnchorIdlInstruction {
                kind: AnchorIdlInstructionKind::Write,
                data_len: Some(len as u64),
                write_data: Some(bytes),
                new_authority: None,
            })
        }
        3 => Some(ParsedAnchorIdlInstruction {
            kind: AnchorIdlInstructionKind::SetBuffer,
            data_len: None,
            write_data: None,
            new_authority: None,
        }),
        4 => Some(ParsedAnchorIdlInstruction {
            kind: AnchorIdlInstructionKind::SetAuthority,
            data_len: None,
            write_data: None,
            new_authority: Some(Pubkey::try_from(data.get(9..41)?).ok()?.to_string()),
        }),
        5 => Some(ParsedAnchorIdlInstruction {
            kind: AnchorIdlInstructionKind::Close,
            data_len: None,
            write_data: None,
            new_authority: None,
        }),
        6 => Some(ParsedAnchorIdlInstruction {
            kind: AnchorIdlInstructionKind::Resize,
            data_len: Some(read_u64(data.get(9..17)?)?),
            write_data: None,
            new_authority: None,
        }),
        _ => None,
    }
}

fn read_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u64(bytes: &[u8]) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_anchor_write() {
        let mut data = IDL_IX_TAG_LE.to_vec();
        data.push(2);
        data.extend_from_slice(&3_u32.to_le_bytes());
        data.extend_from_slice(b"abc");

        let parsed = parse_instruction(&data).expect("legacy IDL instruction");
        assert_eq!(parsed.kind, AnchorIdlInstructionKind::Write);
        assert_eq!(parsed.write_data.as_deref(), Some(&b"abc"[..]));
    }

    #[test]
    fn derives_legacy_anchor_idl_address() {
        let program = Pubkey::new_unique();
        let address = legacy_idl_address(program).expect("legacy IDL address");
        assert_ne!(address, program);
    }
}
