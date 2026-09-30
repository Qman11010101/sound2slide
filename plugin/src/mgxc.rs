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

fn parse_offset(bytes: &[u8]) -> Option<f64> {
    if bytes.starts_with(b"MGCF0") {
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
    } else if bytes.starts_with(b"MGXC-") {
        bytes
            .windows(4)
            .enumerate()
            .filter(|(_, tag)| *tag == b"wvof")
            .find_map(|(index, _)| {
                let value: [u8; 8] = bytes.get(index + 8..index + 16)?.try_into().ok()?;
                let offset = f64::from_le_bytes(value);
                (offset.is_finite() && (-60.0..=60.0).contains(&offset)).then_some(offset)
            })
    } else {
        None
    }
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

    fn chunk(offset: f64) -> Vec<u8> {
        let mut bytes = b"wvof\x08\x00\x00\x00".to_vec();
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes
    }

    #[test]
    fn reads_binary_offsets_and_skips_invalid_earlier_matches() {
        let mut bytes = b"MGXC-".to_vec();
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            60.01,
            -60.01,
            -1.55,
        ] {
            bytes.extend(chunk(value));
        }
        assert_eq!(parse_offset(&bytes), Some(-1.55));
        for value in [-60.0, 60.0, 0.0, 1.55] {
            let mut bytes = b"MGXC-".to_vec();
            bytes.extend(chunk(value));
            assert_eq!(parse_offset(&bytes), Some(value));
        }
    }

    #[test]
    fn rejects_truncated_binary_and_invalid_only_files() {
        for bytes in [
            b"MGXC-".as_slice(),
            b"MGXC-wvof",
            b"MGXC-wvof\x08\x00\x00\x00\x01\x02",
        ] {
            assert_eq!(parse_offset(bytes), None);
        }
        let mut bytes = b"MGXC-".to_vec();
        bytes.extend(chunk(61.0));
        assert_eq!(parse_offset(&bytes), None);
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
