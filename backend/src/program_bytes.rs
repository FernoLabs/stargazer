use crate::models::ProgramByteRange;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub(crate) struct ProgramDataChunk {
    pub(crate) offset: u32,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct ProgramByteAssembly {
    pub(crate) bytes: Option<Vec<u8>>,
    pub(crate) program_len: Option<i64>,
    pub(crate) program_sha256: Option<String>,
    pub(crate) complete: bool,
    pub(crate) chunk_count: usize,
    pub(crate) missing_ranges: Vec<ProgramByteRange>,
}

pub(crate) fn assemble_program_bytes(
    base: Option<Vec<u8>>,
    chunks: Vec<ProgramDataChunk>,
) -> ProgramByteAssembly {
    let chunk_count = chunks.len();
    let max_chunk_end = chunks
        .iter()
        .map(|chunk| chunk.offset as usize + chunk.bytes.len())
        .max()
        .unwrap_or(0);
    let base_len = base.as_ref().map(Vec::len).unwrap_or(0);
    let program_len = base_len.max(max_chunk_end);

    if program_len == 0 {
        return ProgramByteAssembly {
            bytes: None,
            program_len: None,
            program_sha256: None,
            complete: false,
            chunk_count,
            missing_ranges: Vec::new(),
        };
    }

    if let Some(mut bytes) = base {
        bytes.resize(program_len, 0);
        apply_chunks(&mut bytes, &chunks);
        let program_sha256 = sha256_hex(&bytes);
        return ProgramByteAssembly {
            bytes: Some(bytes),
            program_len: Some(program_len as i64),
            program_sha256: Some(program_sha256),
            complete: true,
            chunk_count,
            missing_ranges: Vec::new(),
        };
    }

    let missing_ranges = missing_ranges(program_len, &chunks);
    if !missing_ranges.is_empty() {
        return ProgramByteAssembly {
            bytes: None,
            program_len: Some(program_len as i64),
            program_sha256: None,
            complete: false,
            chunk_count,
            missing_ranges,
        };
    }

    let mut bytes = vec![0; program_len];
    apply_chunks(&mut bytes, &chunks);
    let program_sha256 = sha256_hex(&bytes);
    ProgramByteAssembly {
        bytes: Some(bytes),
        program_len: Some(program_len as i64),
        program_sha256: Some(program_sha256),
        complete: true,
        chunk_count,
        missing_ranges: Vec::new(),
    }
}

pub(crate) fn instruction_order_key(path: &str) -> String {
    path.split('.')
        .map(|part| {
            part.parse::<u32>()
                .map(|value| format!("{value:08}"))
                .unwrap_or_else(|_| part.to_owned())
        })
        .collect::<Vec<_>>()
        .join(".")
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn apply_chunks(bytes: &mut [u8], chunks: &[ProgramDataChunk]) {
    for chunk in chunks {
        let start = chunk.offset as usize;
        let end = start.saturating_add(chunk.bytes.len());
        if let Some(destination) = bytes.get_mut(start..end) {
            destination.copy_from_slice(&chunk.bytes);
        }
    }
}

fn missing_ranges(program_len: usize, chunks: &[ProgramDataChunk]) -> Vec<ProgramByteRange> {
    let mut ranges = chunks
        .iter()
        .filter_map(|chunk| {
            let start = chunk.offset as usize;
            let end = start.saturating_add(chunk.bytes.len()).min(program_len);
            (start < end).then_some((start, end))
        })
        .collect::<Vec<_>>();
    ranges.sort_by_key(|(start, end)| (*start, *end));

    let mut covered_until = 0;
    let mut missing = Vec::new();
    for (start, end) in ranges {
        if start > covered_until {
            missing.push(ProgramByteRange {
                start: covered_until as i64,
                end: start as i64,
            });
        }
        covered_until = covered_until.max(end);
    }

    if covered_until < program_len {
        missing.push(ProgramByteRange {
            start: covered_until as i64,
            end: program_len as i64,
        });
    }

    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_contiguous_chunks() {
        let assembly = assemble_program_bytes(
            None,
            vec![
                ProgramDataChunk {
                    offset: 3,
                    bytes: b"def".to_vec(),
                },
                ProgramDataChunk {
                    offset: 0,
                    bytes: b"abc".to_vec(),
                },
            ],
        );

        assert!(assembly.complete);
        assert_eq!(assembly.bytes.as_deref(), Some(&b"abcdef"[..]));
        assert_eq!(assembly.program_len, Some(6));
        assert!(assembly.missing_ranges.is_empty());
    }

    #[test]
    fn marks_sparse_chunks_incomplete_without_base() {
        let assembly = assemble_program_bytes(
            None,
            vec![ProgramDataChunk {
                offset: 2,
                bytes: b"cd".to_vec(),
            }],
        );

        assert!(!assembly.complete);
        assert!(assembly.bytes.is_none());
        assert_eq!(
            assembly.missing_ranges,
            vec![ProgramByteRange { start: 0, end: 2 }]
        );
    }

    #[test]
    fn marks_sparse_non_header_ranges_incomplete_without_base() {
        let assembly = assemble_program_bytes(
            None,
            vec![
                ProgramDataChunk {
                    offset: 0,
                    bytes: b"ab".to_vec(),
                },
                ProgramDataChunk {
                    offset: 4,
                    bytes: b"ef".to_vec(),
                },
            ],
        );

        assert!(!assembly.complete);
        assert!(assembly.bytes.is_none());
        assert_eq!(
            assembly.missing_ranges,
            vec![ProgramByteRange { start: 2, end: 4 }]
        );
    }

    #[test]
    fn overlays_chunks_on_previous_program_bytes() {
        let assembly = assemble_program_bytes(
            Some(b"abcdef".to_vec()),
            vec![ProgramDataChunk {
                offset: 2,
                bytes: b"XY".to_vec(),
            }],
        );

        assert!(assembly.complete);
        assert_eq!(assembly.bytes.as_deref(), Some(&b"abXYef"[..]));
    }
}
