use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn candidates(audio_path: &Path) -> Vec<PathBuf> {
    let Some(directory) = audio_path.parent() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("mgxc"))
        })
        .collect();
    files.sort();
    files
}

pub(crate) fn read_offset(path: &Path) -> f64 {
    fs::read(path)
        .ok()
        .and_then(|bytes| parse_offset(&bytes))
        .unwrap_or(0.0)
}

pub(crate) fn read_extended_field(path: &Path) -> Option<bool> {
    fs::read(path)
        .ok()
        .and_then(|bytes| parse_extended_field(&bytes))
}

fn parse_offset(bytes: &[u8]) -> Option<f64> {
    if bytes.starts_with(b"MGXC") {
        let (_, metadata_bytes) = chart_metadata(bytes)?;
        parse_metadata(metadata_bytes)?.offset
    } else if bytes.starts_with(b"MGCF0") {
        bytes.split(|byte| *byte == b'\n').find_map(|line| {
            let mut fields = line.split(|byte| *byte == b'\t');
            if fields.next()? != b"BGMOFFSET" {
                return None;
            }
            let offset: f64 = std::str::from_utf8(fields.next()?)
                .ok()?
                .trim()
                .parse()
                .ok()?;
            offset.is_finite().then_some(offset)
        })
    } else {
        None
    }
}

fn parse_extended_field(bytes: &[u8]) -> Option<bool> {
    let (version, metadata_bytes) = chart_metadata(bytes)?;
    if version == 2 {
        return Some(false);
    }
    Some(
        parse_metadata(metadata_bytes)?
            .extended_field
            .unwrap_or(false),
    )
}

#[derive(Default)]
struct Metadata {
    offset: Option<f64>,
    extended_field: Option<bool>,
}

fn chart_metadata(bytes: &[u8]) -> Option<(u32, &[u8])> {
    if bytes.get(..4)? != b"MGXC" {
        return None;
    }
    let content_length =
        usize::try_from(u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?)).ok()?;
    let version = u32::from_le_bytes(bytes.get(8..12)?.try_into().ok()?);
    if !matches!(version, 2 | 4) || content_length != bytes.len().checked_sub(8)? {
        return None;
    }
    let mut position = 12usize;
    let mut metadata = None;
    while position < bytes.len() {
        let header_end = position.checked_add(8)?;
        let header = bytes.get(position..header_end)?;
        let tag = &header[..4];
        let length = usize::try_from(u32::from_le_bytes(header[4..8].try_into().ok()?)).ok()?;
        let end = header_end.checked_add(length)?;
        let chunk = bytes.get(header_end..end)?;
        if tag == b"meta" {
            if metadata.is_some() {
                return None;
            }
            metadata = Some(chunk);
        }
        position = end;
    }
    Some((version, metadata?))
}

fn parse_metadata(bytes: &[u8]) -> Option<Metadata> {
    let mut metadata = Metadata::default();
    let mut position = 0usize;
    while position < bytes.len() {
        let remaining = bytes.get(position..)?;
        if remaining.len() == 8 && remaining[..6].iter().all(|byte| *byte == 0) {
            break;
        }
        let key_end = position.checked_add(4)?;
        let key: [u8; 4] = bytes.get(position..key_end)?.try_into().ok()?;
        let type_end = key_end.checked_add(2)?;
        let value_type = u16::from_le_bytes(bytes.get(key_end..type_end)?.try_into().ok()?);
        position = type_end;

        match value_type {
            1 => {
                let end = position.checked_add(2)?;
                let value = u16::from_le_bytes(bytes.get(position..end)?.try_into().ok()?);
                position = end;
                if &key == b"xfld" {
                    if metadata.extended_field.is_some() || value > 1 {
                        return None;
                    }
                    metadata.extended_field = Some(value == 1);
                }
            }
            2 => {
                let end = position.checked_add(6)?;
                bytes.get(position..end)?;
                position = end;
                if &key == b"xfld" {
                    return None;
                }
            }
            3 => {
                if &key == b"xfld" {
                    return None;
                }
                let end = position.checked_add(10)?;
                let value_bytes = bytes.get(position..end)?;
                let value = f64::from_le_bytes(value_bytes[2..10].try_into().ok()?);
                position = end;
                if &key == b"wvof" {
                    if metadata.offset.is_some() {
                        return None;
                    }
                    if value.is_finite() && (-60.0..=60.0).contains(&value) {
                        metadata.offset = Some(value);
                    }
                }
            }
            4 => {
                if &key == b"xfld" {
                    return None;
                }
                let length_end = position.checked_add(2)?;
                let length = usize::from(u16::from_le_bytes(
                    bytes.get(position..length_end)?.try_into().ok()?,
                ));
                let end = length_end.checked_add(length)?;
                bytes.get(length_end..end)?;
                position = end;
            }
            _ => return None,
        }
    }
    Some(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tab_separated_text_offsets_without_decoding_other_fields() {
        assert_eq!(
            parse_offset(b"MGCF0\r\nTITLE\t\xff\xfe\r\nBGMOFFSET\t-1.55000\r\n"),
            Some(-1.55)
        );
        assert_eq!(parse_offset(b"MGCF0\nBGMOFFSET\t2.50000\n"), Some(2.5));
    }

    #[test]
    fn rejects_unknown_headers_missing_values_and_non_finite_text() {
        for bytes in [
            b"OTHER\nBGMOFFSET\t1.0".as_slice(),
            b"MGCF0\nBGMOFFSET 1.0",
            b"MGCF0\nBGMOFFSET\tbad",
            b"MGCF0\nBGMOFFSET\tNaN",
            b"MGCF0\nBGMOFFSET\tinf",
        ] {
            assert_eq!(parse_offset(bytes), None);
        }
    }

    #[test]
    fn reads_offsets_from_valid_binary_charts_at_different_header_lengths() {
        for (low_byte, expected_magic) in [(0x2d, b"MGXC-".as_slice()), (0x53, b"MGXCS")] {
            let bytes = chart_v2_with_length_low_byte(&offset_property(-1.55), low_byte);
            assert_eq!(&bytes[..5], expected_magic);
            assert_eq!(parse_offset(&bytes), Some(-1.55));
        }
        for value in [-60.0, 60.0, 0.0, 1.55] {
            assert_eq!(
                parse_offset(&chart_v2(&offset_property(value))),
                Some(value)
            );
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 60.01, -60.01] {
            assert_eq!(parse_offset(&chart_v2(&offset_property(value))), None);
        }
    }

    #[test]
    fn rejects_truncated_binary_and_invalid_only_files() {
        for bytes in [
            b"MGXC".as_slice(),
            b"MGXC-wvof",
            b"MGXC-wvof\x08\x00\x00\x00\x01\x02",
        ] {
            assert_eq!(parse_offset(bytes), None);
        }
        let mut malformed = chart_v2(&offset_property(1.5));
        malformed.pop();
        assert_eq!(parse_offset(&malformed), None);
    }

    fn tagged_chunk(tag: &[u8; 4], value: &[u8]) -> Vec<u8> {
        let mut bytes = tag.to_vec();
        bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
        bytes.extend_from_slice(value);
        bytes
    }

    fn integer_property(tag: &[u8; 4], value: u16) -> Vec<u8> {
        let mut bytes = tag.to_vec();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    }

    fn string_property(tag: &[u8; 4], value: &[u8]) -> Vec<u8> {
        let mut bytes = tag.to_vec();
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
        bytes.extend_from_slice(value);
        bytes
    }

    fn offset_property(value: f64) -> Vec<u8> {
        let mut bytes = b"wvof".to_vec();
        bytes.extend_from_slice(&3u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    }

    fn chart_v4(metadata: &[u8]) -> Vec<u8> {
        let mut chunks = tagged_chunk(b"meta", metadata);
        chunks.extend(tagged_chunk(b"junk", b"xfld\x01\x00\x01\x00"));
        let content_length = 4 + chunks.len();
        let mut bytes = b"MGXC".to_vec();
        bytes.extend_from_slice(&(content_length as u32).to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend(chunks);
        bytes
    }

    fn chart_v2(metadata: &[u8]) -> Vec<u8> {
        let mut chunks = tagged_chunk(b"meta", metadata);
        chunks.extend(tagged_chunk(b"dat2", b"not metadata"));
        let content_length = 4 + chunks.len();
        let mut bytes = b"MGXC".to_vec();
        bytes.extend_from_slice(&(content_length as u32).to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend(chunks);
        bytes
    }

    fn chart_v2_with_length_low_byte(metadata: &[u8], low_byte: u8) -> Vec<u8> {
        let mut chunks = tagged_chunk(b"meta", metadata);
        let minimum = 4 + chunks.len() + 8;
        let desired = minimum + (usize::from(low_byte).wrapping_sub(minimum) % 256);
        let padding_length = desired - minimum;
        chunks.extend(tagged_chunk(b"flll", &vec![0; padding_length]));
        let content_length = 4 + chunks.len();
        assert_eq!(content_length as u8, low_byte);
        let mut bytes = b"MGXC".to_vec();
        bytes.extend_from_slice(&(content_length as u32).to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend(chunks);
        bytes
    }

    #[test]
    fn reads_extended_field_and_offset_from_v4_metadata_chunks() {
        let mut metadata = string_property(b"titl", b"xfld");
        metadata.extend(integer_property(b"xfld", 1));
        metadata.extend(offset_property(-1.25));
        let bytes = chart_v4(&metadata);
        assert_eq!(parse_extended_field(&bytes), Some(true));
        assert_eq!(parse_offset(&bytes), Some(-1.25));

        let disabled = chart_v4(&integer_property(b"xfld", 0));
        assert_eq!(parse_extended_field(&disabled), Some(false));
        assert_eq!(parse_offset(&disabled), None);
    }

    #[test]
    fn defaults_missing_extension_flag_to_normal_for_valid_charts() {
        let mut metadata = string_property(b"titl", b"no extension flag");
        metadata.extend(integer_property(b"diff", 5));
        assert_eq!(parse_extended_field(&chart_v4(&metadata)), Some(false));
        assert_eq!(parse_extended_field(&chart_v2(&metadata)), Some(false));
        let legacy_unknown_property = chart_v2(b"CTCK\x02\x00\x00\x00\x00\x00\xb3\x01");
        assert_eq!(parse_extended_field(&legacy_unknown_property), Some(false));
    }

    #[test]
    fn rejects_mistyped_duplicate_and_out_of_range_extension_flags() {
        assert_eq!(
            parse_extended_field(&chart_v4(&string_property(b"xfld", b"true"))),
            None
        );
        let mut duplicate = integer_property(b"xfld", 1);
        duplicate.extend(integer_property(b"xfld", 0));
        assert_eq!(parse_extended_field(&chart_v4(&duplicate)), None);
        assert_eq!(
            parse_extended_field(&chart_v4(&integer_property(b"xfld", 2))),
            None
        );
    }

    #[test]
    fn reads_legacy_v2_offset_from_its_metadata_chunk() {
        let bytes = chart_v2(&offset_property(1.5));
        assert_eq!(parse_offset(&bytes), Some(1.5));
    }

    #[test]
    fn reads_offset_before_a_legacy_reserved_padding_property() {
        let mut metadata = offset_property(0.85);
        metadata.extend_from_slice(&[0; 6]);
        metadata.extend_from_slice(&93u16.to_le_bytes());
        assert_eq!(parse_offset(&chart_v2(&metadata)), Some(0.85));
        metadata.extend(integer_property(b"xfld", 1));
        assert_eq!(parse_offset(&chart_v2(&metadata)), None);
    }

    #[test]
    fn rejects_unsupported_or_malformed_chart_metadata() {
        let malformed = b"MGXC\x04\x00\x00\x00\x04\x00\x00\x00meta\xff\xff\xff\xff";

        assert_eq!(parse_extended_field(malformed), None);
        assert_eq!(parse_offset(malformed), None);

        let unknown_type = chart_v4(b"xfld\x09\x00\x01\x00");
        assert_eq!(parse_extended_field(&unknown_type), None);
        assert_eq!(
            parse_extended_field(b"MGXC\x00\x00\x00\x00\x03\x00\x00\x00"),
            None
        );
        assert_eq!(parse_extended_field(b"unrecognized"), None);
    }

    #[test]
    fn finds_only_neighboring_chart_files_and_defaults_on_read_failure() {
        let directory = std::env::temp_dir().join(format!(
            "sound2slide-mgxc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let audio_path = directory.join("song.mp3");
        assert!(candidates(&audio_path).is_empty());
        assert_eq!(read_offset(&directory.join("missing.mgxc")), 0.0);
        let first = directory.join("a.mgxc");
        let second = directory.join("b.MGXC");
        fs::write(&first, b"MGCF0\nBGMOFFSET\t-1.55000\n").unwrap();
        fs::write(&second, b"invalid").unwrap();
        fs::write(directory.join("notes.txt"), b"MGCF0").unwrap();
        fs::create_dir(directory.join("folder.mgxc")).unwrap();
        assert_eq!(candidates(&audio_path), vec![first.clone(), second.clone()]);
        assert_eq!(read_offset(&first), -1.55);
        assert_eq!(read_offset(&second), 0.0);
        fs::remove_dir_all(&directory).unwrap();
    }
}
