use std::{fs::File, path::Path, sync::Arc};

use symphonia::core::{
    audio::SampleBuffer,
    codecs::{CODEC_TYPE_NULL, DecoderOptions},
    errors::Error as SymError,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

pub(crate) struct Audio {
    pub samples: Arc<[f32]>,
    pub sample_rate: u32,
}

impl Audio {
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f64 / f64::from(self.sample_rate)
        }
    }
}

pub(crate) fn load(path: &Path) -> Result<Audio, String> {
    let file = File::open(path).map_err(|error| format!("ファイルを開けません: {error}"))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| format!("対応していない音声形式です: {error}"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("音声トラックがありません")?;
    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or("サンプルレートが不明です")?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|error| format!("デコーダーを作成できません: {error}"))?;
    let mut samples = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymError::ResetRequired) => break,
            Err(SymError::IoError(_)) => break,
            Err(error) => return Err(format!("音声の読み取りに失敗しました: {error}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
            Err(error) => return Err(format!("デコードに失敗しました: {error}")),
        };
        let spec = *decoded.spec();
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        let channels = spec.channels.count().max(1);
        if channels == 1 {
            samples.extend_from_slice(buffer.samples());
        } else {
            for frame in buffer.samples().chunks(channels) {
                let sum: f32 = frame.iter().copied().sum();
                samples.push(sum / channels as f32);
            }
        }
    }
    if samples.is_empty() {
        return Err("音声サンプルを読み取れませんでした".into());
    }
    Ok(Audio {
        samples: samples.into(),
        sample_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_cbr_and_vbr_mp3_files() {
        for (file, sample_rate) in [("tone-cbr.mp3", 44100), ("tone-vbr.mp3", 48000)] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(file);
            let audio = load(&path).unwrap_or_else(|error| panic!("{file}: {error}"));
            assert_eq!(audio.sample_rate, sample_rate);
            assert!((0.2..0.4).contains(&audio.duration_secs()));
            assert!(audio.samples.iter().all(|sample| sample.is_finite()));
            assert!(audio.samples.iter().any(|sample| sample.abs() > 0.01));
        }
    }
}
