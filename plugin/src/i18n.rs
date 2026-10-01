use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Lang {
    #[default]
    Ja,
    En,
    ZhTw,
    Ko,
}

impl Lang {
    pub(crate) const ALL: [Self; 4] = [Self::Ja, Self::En, Self::ZhTw, Self::Ko];

    pub(crate) fn name(self) -> &'static str {
        self.pick(["日本語", "English", "繁體中文", "한국어"])
    }

    /// Chooses the entry for this language from `[ja, en, zh-TW, ko]`.
    pub(crate) fn pick<T>(self, [ja, en, zh_tw, ko]: [T; 4]) -> T {
        match self {
            Self::Ja => ja,
            Self::En => en,
            Self::ZhTw => zh_tw,
            Self::Ko => ko,
        }
    }
}

/// A message kept in UI state and rendered in whichever language is selected at display time.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Msg {
    Raw(String),
    OpenAudioPrompt,
    AudioInfo { seconds: f64, sample_rate: u32 },
    OpenFile(String),
    UnsupportedFormat(String),
    NoAudioTrack,
    UnknownSampleRate,
    CreateDecoder(String),
    ReadAudio(String),
    Decode(String),
    NoSamples,
    OpenOutput(String),
    NoPlaybackRange,
    InvalidSampleRate,
    PlaybackRangeTooLong,
    PlaybackPositionOutOfRange,
    BeforeTickZero,
    InvalidBpmAt(i32),
    InvalidSignatureAt(i32),
    NoChart,
    InvalidBpm,
    SignatureOutOfRange,
    BpmEventNotFound,
    SignatureEventNotFound,
    PositionScanFailed(Box<Msg>),
    PositionOutOfRange(f64),
}

impl Msg {
    pub(crate) fn text(&self, lang: Lang) -> String {
        let detail = |texts: [&str; 4], error: &str| format!("{}: {error}", lang.pick(texts));
        match self {
            Self::Raw(text) => text.clone(),
            Self::OpenAudioPrompt => lang
                .pick([
                    "音声ファイルを開いてください",
                    "Open an audio file",
                    "請開啟音訊檔案",
                    "오디오 파일을 열어 주세요",
                ])
                .into(),
            Self::AudioInfo {
                seconds,
                sample_rate,
            } => lang.pick([
                format!("{seconds:.2} 秒 / {sample_rate} Hz"),
                format!("{seconds:.2} s / {sample_rate} Hz"),
                format!("{seconds:.2} 秒 / {sample_rate} Hz"),
                format!("{seconds:.2}초 / {sample_rate} Hz"),
            ]),
            Self::OpenFile(error) => detail(
                [
                    "ファイルを開けません",
                    "Cannot open the file",
                    "無法開啟檔案",
                    "파일을 열 수 없습니다",
                ],
                error,
            ),
            Self::UnsupportedFormat(error) => detail(
                [
                    "対応していない音声形式です",
                    "Unsupported audio format",
                    "不支援的音訊格式",
                    "지원하지 않는 오디오 형식입니다",
                ],
                error,
            ),
            Self::NoAudioTrack => lang
                .pick([
                    "音声トラックがありません",
                    "No audio track found",
                    "找不到音軌",
                    "오디오 트랙이 없습니다",
                ])
                .into(),
            Self::UnknownSampleRate => lang
                .pick([
                    "サンプルレートが不明です",
                    "Unknown sample rate",
                    "取樣率不明",
                    "샘플 레이트를 알 수 없습니다",
                ])
                .into(),
            Self::CreateDecoder(error) => detail(
                [
                    "デコーダーを作成できません",
                    "Cannot create a decoder",
                    "無法建立解碼器",
                    "디코더를 만들 수 없습니다",
                ],
                error,
            ),
            Self::ReadAudio(error) => detail(
                [
                    "音声の読み取りに失敗しました",
                    "Failed to read the audio",
                    "讀取音訊失敗",
                    "오디오를 읽지 못했습니다",
                ],
                error,
            ),
            Self::Decode(error) => detail(
                [
                    "デコードに失敗しました",
                    "Failed to decode",
                    "解碼失敗",
                    "디코딩하지 못했습니다",
                ],
                error,
            ),
            Self::NoSamples => lang
                .pick([
                    "音声サンプルを読み取れませんでした",
                    "No audio samples could be read",
                    "無法讀取音訊樣本",
                    "오디오 샘플을 읽을 수 없었습니다",
                ])
                .into(),
            Self::OpenOutput(error) => detail(
                [
                    "音声出力を開けません",
                    "Cannot open the audio output",
                    "無法開啟音訊輸出",
                    "오디오 출력을 열 수 없습니다",
                ],
                error,
            ),
            Self::NoPlaybackRange => lang
                .pick([
                    "再生できる範囲がありません",
                    "There is no range to play",
                    "沒有可播放的範圍",
                    "재생할 수 있는 범위가 없습니다",
                ])
                .into(),
            Self::InvalidSampleRate => lang
                .pick([
                    "サンプルレートが無効です",
                    "Invalid sample rate",
                    "取樣率無效",
                    "샘플 레이트가 올바르지 않습니다",
                ])
                .into(),
            Self::PlaybackRangeTooLong => lang
                .pick([
                    "再生範囲が長すぎます",
                    "The playback range is too long",
                    "播放範圍過長",
                    "재생 범위가 너무 깁니다",
                ])
                .into(),
            Self::PlaybackPositionOutOfRange => lang
                .pick([
                    "再生位置が取り込む範囲の外です",
                    "The playback position is outside the import range",
                    "播放位置超出匯入範圍",
                    "재생 위치가 가져올 범위 밖입니다",
                ])
                .into(),
            Self::BeforeTickZero => lang
                .pick([
                    "現在位置が tick 0 より前のため、自動計算できません",
                    "Cannot calculate automatically because the current position is before tick 0",
                    "目前位置在 tick 0 之前，無法自動計算",
                    "현재 위치가 tick 0보다 앞이므로 자동으로 계산할 수 없습니다",
                ])
                .into(),
            Self::InvalidBpmAt(tick) => lang.pick([
                format!("tick {tick} のBPMが無効です"),
                format!("The BPM at tick {tick} is invalid"),
                format!("tick {tick} 的 BPM 無效"),
                format!("tick {tick}의 BPM이 올바르지 않습니다"),
            ]),
            Self::InvalidSignatureAt(tick) => lang.pick([
                format!("tick {tick} の拍子が無効です"),
                format!("The time signature at tick {tick} is invalid"),
                format!("tick {tick} 的拍號無效"),
                format!("tick {tick}의 박자가 올바르지 않습니다"),
            ]),
            Self::NoChart => lang
                .pick([
                    "譜面を取得できません",
                    "Cannot get the chart",
                    "無法取得譜面",
                    "채보를 가져올 수 없습니다",
                ])
                .into(),
            Self::InvalidBpm => lang
                .pick([
                    "BPMが無効です",
                    "The BPM is invalid",
                    "BPM 無效",
                    "BPM이 올바르지 않습니다",
                ])
                .into(),
            Self::SignatureOutOfRange => lang
                .pick([
                    "拍子が入力範囲外です",
                    "The time signature is out of range",
                    "拍號超出輸入範圍",
                    "박자가 입력 범위를 벗어났습니다",
                ])
                .into(),
            Self::BpmEventNotFound => lang
                .pick([
                    "BPMイベントは見つかりませんでした",
                    "No BPM event was found",
                    "找不到 BPM 事件",
                    "BPM 이벤트를 찾지 못했습니다",
                ])
                .into(),
            Self::SignatureEventNotFound => lang
                .pick([
                    "拍子イベントは見つかりませんでした",
                    "No time signature event was found",
                    "找不到拍號事件",
                    "박자 이벤트를 찾지 못했습니다",
                ])
                .into(),
            Self::PositionScanFailed(error) => detail(
                [
                    "開始位置を計算できません",
                    "Cannot calculate the start position",
                    "無法計算起始位置",
                    "시작 위치를 계산할 수 없습니다",
                ],
                &error.text(lang),
            ),
            Self::PositionOutOfRange(seconds) => lang.pick([
                format!(
                    "計算した開始位置（{seconds:.3} 秒）が音声の範囲外です。始点を手動で設定してください。"
                ),
                format!(
                    "The calculated start position ({seconds:.3} s) is outside the audio. Set the start manually."
                ),
                format!("計算出的起始位置（{seconds:.3} 秒）超出音訊範圍。請手動設定起點。"),
                format!(
                    "계산한 시작 위치({seconds:.3}초)가 오디오 범위를 벗어났습니다. 시작점을 직접 설정해 주세요."
                ),
            ]),
        }
    }
}

impl From<String> for Msg {
    fn from(text: String) -> Self {
        Self::Raw(text)
    }
}

impl From<&str> for Msg {
    fn from(text: &str) -> Self {
        Self::Raw(text.into())
    }
}

impl fmt::Display for Msg {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text(Lang::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_japanese_and_renders_every_language() {
        assert_eq!(Lang::default(), Lang::Ja);
        assert_eq!(Msg::NoChart.to_string(), "譜面を取得できません");
        let nested = Msg::PositionScanFailed(Box::new(Msg::InvalidBpmAt(3)));
        assert_eq!(
            nested.text(Lang::En),
            "Cannot calculate the start position: The BPM at tick 3 is invalid"
        );
        for lang in Lang::ALL {
            assert!(!Msg::OpenAudioPrompt.text(lang).is_empty());
        }
        assert_eq!(Msg::from("raw").text(Lang::Ko), "raw");
    }
}
