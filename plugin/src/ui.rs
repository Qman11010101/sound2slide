use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::{
    audio::{self, Audio},
    audit::Audit,
    generate::{self, Generated, Settings, SilentZone, Slide, TICKS_PER_BEAT, ticks_per_second},
    i18n::{Lang, Msg},
    mgxc,
    modal::ModalHost,
    playback::{Playback, preview_range},
    rainbone::{self, Output},
    timing::{ChartPosition, PositionScan},
    update::{self, Release, UpdateCheck},
};

const QUANTIZE: &[(i32, &str)] = &[
    (480, "1/4 (480 tick)"),
    (240, "1/8 (240 tick)"),
    (160, "1/12 (160 tick)"),
    (120, "1/16 (120 tick)"),
    (80, "1/24 (80 tick)"),
    (60, "1/32 (60 tick)"),
    (40, "1/48 (40 tick)"),
    (30, "1/64 (30 tick)"),
    (20, "1/96 (20 tick)"),
    (15, "1/128 (15 tick)"),
    (10, "1/192 (10 tick)"),
    (8, "1/240 (8 tick)"),
    (6, "1/320 (6 tick)"),
    (5, "1/384 (5 tick)"),
    (4, "1/480 (4 tick)"),
    (3, "1/640 (3 tick)"),
    (2, "1/960 (2 tick)"),
    (1, "1/1920 (1 tick)"),
];

pub(crate) struct Commit {
    pub slides: Vec<Slide>,
    pub transparent_steps: bool,
    pub output: Output,
}

pub(crate) fn open(
    host: &ModalHost,
    start_tick: i32,
    lookup_bpm: &dyn Fn(i32) -> Result<Option<f64>, Msg>,
    lookup_signature: &dyn Fn(i32) -> Result<Option<[i32; 2]>, Msg>,
    chart_uses_extended_field: bool,
) -> Result<Option<Commit>, String> {
    let output = Arc::new(Mutex::new(None));
    let app_output = Arc::clone(&output);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("sound2slide")
            // eframe targets the active HWND, which can still belong to Margrete.
            .with_icon(egui::IconData::default())
            .with_inner_size([780.0, 820.0])
            .with_min_inner_size([620.0, 520.0]),
        centered: true,
        persist_window: false,
        run_and_return: true,
        ..Default::default()
    };
    eframe::run_native(
        "sound2slide",
        options,
        Box::new(move |creation| {
            host.attach(creation).map_err(std::io::Error::other)?;
            apply_theme(&creation.egui_ctx);
            let mut app = Sound2SlideApp::new(start_tick, app_output, lookup_bpm, lookup_signature);
            app.host_extended_field = chart_uses_extended_field;
            app.select_detected_field();
            app.host = Some(host);
            app.update_check = UpdateCheck::start();
            app.fonts = UiFonts::load();
            app.fonts.apply(&creation.egui_ctx, app.lang);
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| error.to_string())?;
    let mut guard = output.lock().unwrap_or_else(|error| error.into_inner());
    Ok(guard.take())
}

const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(18, 21, 30);
const BG_CARD: egui::Color32 = egui::Color32::from_rgb(25, 29, 41);
const BG_INPUT: egui::Color32 = egui::Color32::from_rgb(14, 17, 25);
const BORDER_SUBTLE: egui::Color32 = egui::Color32::from_rgb(43, 50, 68);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(232, 237, 246);
const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(148, 158, 178);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(46, 152, 240);
const ACCENT_BRIGHT: egui::Color32 = egui::Color32::from_rgb(96, 196, 255);
const WARN_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 184, 77);
const WARN_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(44, 34, 22, 255);
const OK_COLOR: egui::Color32 = egui::Color32::from_rgb(88, 214, 168);
const OK_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(20, 38, 36, 255);

fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_CARD;
    visuals.extreme_bg_color = BG_INPUT;
    visuals.faint_bg_color = egui::Color32::from_rgb(22, 26, 37);
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.window_corner_radius = egui::CornerRadius::same(8);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);
    visuals.window_stroke = egui::Stroke::new(1.0, BORDER_SUBTLE);

    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.weak_bg_fill = BG_CARD;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(5);

    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(33, 39, 54);
    visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(31, 37, 52);
    visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(52, 60, 82));
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(5);

    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(44, 53, 74);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(44, 53, 74);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT_BRIGHT);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.2, egui::Color32::WHITE);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(5);

    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(52, 64, 90);
    visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(52, 64, 90);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.2, ACCENT_BRIGHT);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5, egui::Color32::WHITE);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(5);

    visuals.widgets.open.bg_fill = egui::Color32::from_rgb(39, 47, 66);
    visuals.widgets.open.weak_bg_fill = egui::Color32::from_rgb(39, 47, 66);
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(5);

    visuals.selection.bg_fill = egui::Color32::from_rgb(31, 86, 138);
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_BRIGHT);
    visuals.hyperlink_color = ACCENT_BRIGHT;
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.combo_height = 240.0;
        style.spacing.interact_size.y = 24.0;
    });
}

fn section_card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(BG_CARD)
        .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
}

fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(11.5)
            .strong()
            .color(ACCENT_BRIGHT),
    );
}

fn primary_button(text: impl Into<egui::WidgetText>, enabled: bool) -> egui::Button<'static> {
    let mut button = egui::Button::new(text).corner_radius(egui::CornerRadius::same(5));
    if enabled {
        button = button
            .fill(egui::Color32::from_rgb(36, 118, 196))
            .stroke(egui::Stroke::new(1.0, ACCENT_BRIGHT));
    }
    button
}

fn segmented_choice(ui: &mut egui::Ui, selected: bool, text: &str, width: f32) -> egui::Response {
    let mut button = egui::Button::new(egui::RichText::new(text).color(if selected {
        egui::Color32::WHITE
    } else {
        TEXT_PRIMARY
    }))
    .min_size(egui::vec2(width, 24.0))
    .corner_radius(egui::CornerRadius::same(4));
    if selected {
        button = button
            .fill(egui::Color32::from_rgb(34, 96, 156))
            .stroke(egui::Stroke::new(1.0, ACCENT_BRIGHT));
    }
    ui.add(button)
}

#[derive(Clone, Copy)]
enum ZoneDrag {
    Create { index: usize, anchor: f64 },
    Resize { index: usize, end: bool },
}

fn zone_elapsed(inner: egui::Rect, y: f32, span: f64) -> f64 {
    f64::from(((inner.bottom() - y) / inner.height()).clamp(0.0, 1.0)) * span
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FieldMode {
    #[default]
    Normal,
    Extended,
}

impl FieldMode {
    fn label(self, lang: Lang) -> &'static str {
        match self {
            Self::Normal => lang.pick([
                "通常16レーン",
                "Normal (16 lanes)",
                "一般16軌",
                "일반 16레인",
            ]),
            Self::Extended => lang.pick([
                "拡張48レーン",
                "Extended (48 lanes)",
                "擴充48軌",
                "확장 48레인",
            ]),
        }
    }
}

struct OffsetSelection {
    position: ChartPosition,
    files: Vec<std::path::PathBuf>,
    selected: usize,
}

struct Sound2SlideApp<'a> {
    host: Option<&'a ModalHost>,
    start_tick: i32,
    audio: Option<Audio>,
    path: String,
    lang: Lang,
    fonts: UiFonts,
    status: Msg,
    settings: Settings,
    field_mode: FieldMode,
    field_manually_selected: bool,
    host_extended_field: bool,
    chart_files: Vec<std::path::PathBuf>,
    selected_chart: Option<std::path::PathBuf>,
    selected_chart_extended: Option<bool>,
    bpm_note: Vec<Msg>,
    transparent_steps: bool,
    rainbone: bool,
    slide_no_center_line: bool,
    rainbone_axis: rainbone::Axis,
    rainbone_color: i32,
    rainbone_height: i32,
    rainbone_fine: bool,
    lane_divisions: i32,
    generated: Generated,
    audit: Option<Audit>,
    audit_output: Output,
    silent_zones: Vec<SilentZone>,
    cached_silent_zones: Vec<SilentZone>,
    editing_silent_zones: bool,
    zone_drag: Option<ZoneDrag>,
    cached_settings: Settings,
    cached_generation: u64,
    generation: u64,
    output: Arc<Mutex<Option<Commit>>>,
    lookup_bpm: &'a dyn Fn(i32) -> Result<Option<f64>, Msg>,
    lookup_signature: &'a dyn Fn(i32) -> Result<Option<[i32; 2]>, Msg>,
    playback: Option<Playback>,
    playback_error: Option<Msg>,
    position_scan: Option<PositionScan>,
    offset_selection: Option<OffsetSelection>,
    show_open_source_libraries: bool,
    update_check: Option<UpdateCheck>,
    update_available: Option<Release>,
    update_dialog_open: bool,
}

impl<'a> Sound2SlideApp<'a> {
    fn new(
        start_tick: i32,
        output: Arc<Mutex<Option<Commit>>>,
        lookup_bpm: &'a dyn Fn(i32) -> Result<Option<f64>, Msg>,
        lookup_signature: &'a dyn Fn(i32) -> Result<Option<[i32; 2]>, Msg>,
    ) -> Self {
        let settings = Settings::default();
        Self {
            host: None,
            start_tick,
            audio: None,
            path: String::new(),
            lang: Lang::default(),
            fonts: UiFonts::default(),
            status: Msg::OpenAudioPrompt,
            settings,
            field_mode: FieldMode::Normal,
            field_manually_selected: false,
            host_extended_field: false,
            chart_files: Vec::new(),
            selected_chart: None,
            selected_chart_extended: None,
            bpm_note: Vec::new(),
            transparent_steps: true,
            rainbone: false,
            slide_no_center_line: false,
            rainbone_axis: rainbone::Axis::default(),
            rainbone_color: 0,
            rainbone_height: rainbone::DEFAULT_HEIGHT,
            rainbone_fine: false,
            lane_divisions: 16,
            generated: Generated::default(),
            audit: None,
            audit_output: Output::Slide,
            silent_zones: Vec::new(),
            cached_silent_zones: Vec::new(),
            editing_silent_zones: false,
            zone_drag: None,
            cached_settings: settings,
            cached_generation: 1,
            generation: 0,
            output,
            lookup_bpm,
            lookup_signature,
            playback: None,
            playback_error: None,
            position_scan: None,
            offset_selection: None,
            show_open_source_libraries: false,
            update_check: None,
            update_available: None,
            update_dialog_open: false,
        }
    }

    fn refresh(&mut self) {
        self.sync_field_mode();
        self.sync_silent_zones();
        let settings = self.generation_settings();
        if self.cached_generation == self.generation
            && self.cached_settings == settings
            && self.cached_silent_zones == self.silent_zones
        {
            if self.audit_output != self.output() {
                self.reset_audit();
            }
            return;
        }
        self.generated = match &self.audio {
            Some(audio) => generate::generate_with_silent_zones(
                &audio.samples,
                audio.sample_rate,
                self.start_tick,
                &settings,
                &self.silent_zones,
            ),
            None => Generated::default(),
        };
        self.cached_generation = self.generation;
        self.cached_settings = settings;
        self.cached_silent_zones.clone_from(&self.silent_zones);
        self.reset_audit();
    }

    fn reset_audit(&mut self) {
        self.audit_output = self.output();
        self.audit = self
            .settings
            .extended_field
            .then(|| Audit::new(&self.generated.slides, self.rainbone));
    }

    fn audit_warnings(&mut self, ui: &mut egui::Ui) {
        let Some(audit) = &mut self.audit else {
            return;
        };
        if self.generated.slides.is_empty() {
            return;
        }
        audit.advance(&self.generated.slides, self.lookup_bpm);
        let lang = self.lang;
        let has_issue = audit.error.is_some() || audit.count > 0;
        let (fill, stroke) = if has_issue {
            (
                WARN_BG,
                egui::Stroke::new(1.0, WARN_COLOR.gamma_multiply(0.55)),
            )
        } else if audit.complete {
            (OK_BG, egui::Stroke::new(1.0, OK_COLOR.gamma_multiply(0.45)))
        } else {
            (BG_CARD, egui::Stroke::new(1.0, BORDER_SUBTLE))
        };
        egui::Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if !audit.complete {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(lang.pick([
                            "48レーン校閲中…",
                            "Checking the 48-lane field…",
                            "正在檢查48軌…",
                            "48레인 검사 중…",
                        ]));
                    });
                    ui.ctx().request_repaint();
                }
                if let Some(error) = &audit.error {
                    ui.colored_label(
                        WARN_COLOR,
                        lang.pick([
                            format!("校閲を完了できませんでした：{}", error.text(lang)),
                            format!("Could not complete the check: {}", error.text(lang)),
                            format!("無法完成檢查：{}", error.text(lang)),
                            format!("검사를 완료하지 못했습니다: {}", error.text(lang)),
                        ]),
                    );
                }
                if audit.count == 0 {
                    if audit.complete && audit.error.is_none() {
                        ui.label(
                            egui::RichText::new(lang.pick([
                                "48レーン校閲：警告なし",
                                "48-lane check: no warnings",
                                "48軌檢查：無警告",
                                "48레인 검사: 경고 없음",
                            ]))
                            .color(OK_COLOR)
                            .strong(),
                        );
                    }
                    return;
                }
                ui.label(
                    egui::RichText::new(lang.pick([
                        format!("48レーン校閲：警告 {} 件", audit.count),
                        format!("48-lane check: {} warnings", audit.count),
                        format!("48軌檢查：{}項警告", audit.count),
                        format!("48레인 검사: 경고 {}개", audit.count),
                    ]))
                    .color(WARN_COLOR)
                    .strong(),
                );
                ui.label(
                    egui::RichText::new(lang.pick([
                        "Margreteの譜面出力で弾かれる可能性があります。移動範囲を狭めるか、始点・終点を中央に寄せてください。警告があっても追加できます。",
                        "Margrete may reject chart export. Reduce the movement range or move the start/end toward the center. You can still add these notes.",
                        "Margrete可能拒絕匯出。請縮小移動範圍，或將起點、終點移向中央。有警告時仍可加入音符。",
                        "Margrete에서 채보 내보내기가 거부될 수 있습니다. 이동 범위를 줄이거나 시작점·끝점을 중앙으로 옮겨 주세요. 경고가 있어도 추가할 수 있습니다.",
                    ]))
                    .size(11.5)
                    .color(TEXT_MUTED),
                );
                ui.collapsing(
                    lang.pick(["警告の詳細", "Warning details", "警告詳情", "경고 상세"]),
                    |ui| {
                        ui.label(
                            egui::RichText::new(lang.pick([
                                "位置は譜面の絶対tickです。時間の境界は未確定のため、途中90 ms・終端40 msから警告します（AIRなし）。",
                                "Positions are absolute chart ticks. Boundaries are unverified, so warnings start at 90 ms mid-slide and 40 ms at the end (without AIR).",
                                "位置為譜面的絕對tick。邊界尚未確認，因此中途90 ms、末端40 ms起即顯示警告（無AIR）。",
                                "위치는 채보의 절대 tick입니다. 경계가 미확정이므로 중간 90 ms·끝 40 ms부터 경고합니다 (AIR 없음).",
                            ]))
                            .size(11.0)
                            .color(TEXT_MUTED),
                        );
                        egui::Frame::new()
                            .fill(BG_INPUT)
                            .corner_radius(egui::CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(6, 4))
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("audit_details")
                                    .max_height(160.0)
                                    .show(ui, |ui| {
                                        for warning in &audit.warnings {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "#{} / tick {}: {}",
                                                    warning.slide + 1,
                                                    warning.tick,
                                                    warning.reason.text(lang)
                                                ))
                                                .size(11.5),
                                            );
                                        }
                                        if audit.count > audit.warnings.len() {
                                            ui.label(
                                                egui::RichText::new(lang.pick([
                                                    "詳細は先頭100件まで表示します。",
                                                    "Showing the first 100 warnings.",
                                                    "僅顯示前100項警告。",
                                                    "처음 100개 경고를 표시합니다.",
                                                ]))
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                            );
                                        }
                                    });
                            });
                    },
                );
            });
    }

    fn open_file(&mut self, parent: &eframe::Frame) {
        let lang = self.lang;
        let Some(path) = rfd::FileDialog::new()
            .set_title(lang.pick([
                "音声ファイルを開く",
                "Open audio file",
                "開啟音訊檔案",
                "오디오 파일 열기",
            ]))
            .set_parent(parent)
            .add_filter(
                lang.pick(["音声", "Audio", "音訊", "오디오"]),
                &["wav", "mp3", "flac", "ogg", "oga"],
            )
            .pick_file()
        else {
            return;
        };
        match audio::load(&path) {
            Ok(audio) => {
                let offer_calculation = audio.duration_secs() >= 60.0;
                self.accept_audio(&path, audio);
                if offer_calculation
                    && rfd::MessageDialog::new()
                        .set_title("sound2slide")
                        .set_description(lang.pick([
                            "楽曲ファイルを検知しました。\n音声の開始位置を自動で計算しますか？",
                            "This looks like a song file.\nCalculate the audio start position automatically?",
                            "偵測到樂曲檔案。\n要自動計算音訊的起始位置嗎？",
                            "곡 파일이 감지되었습니다.\n오디오 시작 위치를 자동으로 계산할까요?",
                        ]))
                        .set_buttons(rfd::MessageButtons::YesNo)
                        .set_parent(parent)
                        .show()
                        == rfd::MessageDialogResult::Yes
                {
                    match PositionScan::new(self.start_tick) {
                        Ok(scan) => self.position_scan = Some(scan),
                        Err(error) => self.bpm_note = vec![error],
                    }
                }
            }
            Err(error) => self.status = error,
        }
    }

    fn accept_audio(&mut self, path: &std::path::Path, audio: Audio) {
        self.playback = None;
        self.silent_zones.clear();
        self.zone_drag = None;
        self.position_scan = None;
        self.offset_selection = None;
        self.playback_error = None;
        self.bpm_note.clear();
        self.status = Msg::AudioInfo {
            seconds: audio.duration_secs(),
            sample_rate: audio.sample_rate,
        };
        self.settings.offset_sec = 0.0;
        self.settings.length_sec = if audio.duration_secs() >= 60.0 {
            3.0
        } else {
            audio.duration_secs()
        };
        self.chart_files = mgxc::candidates(path);
        let selected = if self.chart_files.len() == 1 {
            self.chart_files.first().cloned()
        } else {
            None
        };
        self.select_chart(selected);
        self.path = path.display().to_string();
        self.audio = Some(audio);
        self.generation += 1;
    }

    fn advance_position_scan(&mut self, ctx: &egui::Context) {
        let Some(scan) = &mut self.position_scan else {
            return;
        };
        match scan.advance(self.lookup_bpm, self.lookup_signature) {
            Ok(Some(position)) => {
                self.position_scan = None;
                self.resolve_chart_offset(
                    position,
                    mgxc::candidates(std::path::Path::new(&self.path)),
                );
            }
            Ok(None) => ctx.request_repaint(),
            Err(error) => {
                self.position_scan = None;
                self.bpm_note = vec![Msg::PositionScanFailed(Box::new(error))];
            }
        }
    }

    fn resolve_chart_offset(&mut self, position: ChartPosition, files: Vec<std::path::PathBuf>) {
        match files.as_slice() {
            [] => self.apply_chart_position(position),
            [file] => {
                self.select_chart(Some(file.clone()));
                self.apply_chart_position(ChartPosition {
                    seconds: position.seconds - mgxc::read_offset(file),
                    ..position
                });
            }
            _ => {
                let selected = self
                    .selected_chart
                    .as_ref()
                    .and_then(|selected| files.iter().position(|file| file == selected))
                    .unwrap_or(0);
                self.offset_selection = Some(OffsetSelection {
                    position,
                    files,
                    selected,
                });
            }
        }
    }

    fn finish_offset_selection(&mut self, use_selected: bool) {
        let Some(selection) = self.offset_selection.take() else {
            return;
        };
        if use_selected {
            self.select_chart(selection.files.get(selection.selected).cloned());
        }
        let offset = if use_selected {
            selection
                .files
                .get(selection.selected)
                .map(|file| mgxc::read_offset(file))
                .unwrap_or(0.0)
        } else {
            0.0
        };
        self.apply_chart_position(ChartPosition {
            seconds: selection.position.seconds - offset,
            ..selection.position
        });
    }

    fn offset_selection_dialog(&mut self, ctx: &egui::Context) {
        let lang = self.lang;
        let Some(selection) = &mut self.offset_selection else {
            return;
        };
        let mut action = None;
        egui::Modal::new(egui::Id::new("mgxc_offset_selection")).show(ctx, |ui| {
            ui.set_max_width((ctx.content_rect().width() - 64.0).clamp(240.0, 440.0));
            ui.label(
                egui::RichText::new(lang.pick([
                    "どの譜面ファイルを元にオフセットを設定しますか？",
                    "Which chart file should the offset be taken from?",
                    "要依據哪個譜面檔案設定偏移？",
                    "어느 채보 파일을 기준으로 오프셋을 설정할까요?",
                ]))
                .strong(),
            );
            ui.add_space(8.0);
            egui::Frame::new()
                .fill(BG_INPUT)
                .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE))
                .corner_radius(egui::CornerRadius::same(5))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical()
                        .max_height((ctx.content_rect().height() * 0.5).min(300.0))
                        .show(ui, |ui| {
                            for (index, file) in selection.files.iter().enumerate() {
                                let name = file.file_name().unwrap_or_default().to_string_lossy();
                                ui.radio_value(&mut selection.selected, index, name.as_ref());
                            }
                        });
                });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add(primary_button(
                        lang.pick(["選択する", "Select", "選擇", "선택"]),
                        true,
                    ))
                    .clicked()
                {
                    action = Some(true);
                }
                if ui
                    .button(lang.pick([
                        "オフセットを計算しない",
                        "Don't apply an offset",
                        "不計算偏移",
                        "오프셋을 계산하지 않음",
                    ]))
                    .clicked()
                {
                    action = Some(false);
                }
            });
        });
        if let Some(use_selected) = action {
            self.finish_offset_selection(use_selected);
            ctx.request_repaint();
        }
    }

    fn apply_chart_position(&mut self, position: ChartPosition) {
        let Some(audio) = &self.audio else {
            return;
        };
        let duration = audio.duration_secs();
        if !position.seconds.is_finite() || position.seconds >= duration {
            self.bpm_note = vec![Msg::PositionOutOfRange(position.seconds)];
            return;
        }
        self.settings.offset_sec = position.seconds;
        self.settings.length_sec = self.settings.length_sec.min(duration - position.seconds);
        self.settings.bpm = position.bpm;
        self.settings.time_signature = position.signature;
        self.bpm_note.clear();
    }

    fn language_selector(&mut self, ui: &mut egui::Ui) {
        let previous = self.lang;
        egui::ComboBox::from_id_salt("language")
            .selected_text(self.lang.name())
            .show_ui(ui, |ui| {
                for lang in Lang::ALL {
                    ui.selectable_value(&mut self.lang, lang, lang.name());
                }
            })
            .response
            .on_hover_text(self.lang.pick(["言語", "Language", "語言", "언어"]));
        if self.lang != previous {
            self.fonts.apply(ui.ctx(), self.lang);
            ui.ctx().request_repaint();
        }
    }

    fn note_count(&self) -> usize {
        self.generated
            .slides
            .iter()
            .map(|slide| {
                if self.transparent_steps && !self.rainbone {
                    let start = slide.points.first().map(|point| point.tick).unwrap_or(0);
                    let end = slide.points.last().map(|point| point.tick).unwrap_or(start);
                    generate::transparent_slide_note_count(start, end)
                } else {
                    slide.points.len()
                }
            })
            .sum()
    }

    fn controls_header(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("sound2slide")
                    .size(18.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.language_selector(ui);
            });
        });
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            egui::Frame::new()
                .fill(BG_INPUT)
                .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE))
                .corner_radius(egui::CornerRadius::same(4))
                .inner_margin(egui::Margin::symmetric(6, 2))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            lang.pick(["開始 tick:", "Start tick:", "起始 tick：", "시작 tick:"]),
                            self.start_tick
                        ))
                        .size(11.5)
                        .color(ACCENT_BRIGHT),
                    );
                });
        });
        ui.label(
            egui::RichText::new(lang.pick([
                "このウィンドウを閉じるまで Margrete は待機します。",
                "Margrete waits until this window is closed.",
                "關閉此視窗前 Margrete 會保持等待。",
                "이 창을 닫을 때까지 Margrete는 대기합니다.",
            ]))
            .size(11.0)
            .color(TEXT_MUTED),
        );
        ui.add_space(2.0);
    }

    fn controls_footer(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let notes = self.note_count();
        ui.add_space(4.0);
        if self.generated.truncated {
            egui::Frame::new()
                .fill(WARN_BG)
                .stroke(egui::Stroke::new(1.0, WARN_COLOR.gamma_multiply(0.5)))
                .corner_radius(egui::CornerRadius::same(5))
                .inner_margin(egui::Margin::symmetric(8, 5))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(lang.pick([
                            "点数上限に達したため、後ろを切り捨てました。量子化を粗くするか長さを短くしてください。",
                            "The point limit was reached, so the end was cut off. Use a coarser quantize or a shorter length.",
                            "已達點數上限，後段已被截斷。請將量化調粗或縮短長度。",
                            "점 개수 상한에 도달하여 뒷부분을 잘라냈습니다. 퀀타이즈를 거칠게 하거나 길이를 줄여 주세요.",
                        ]))
                        .size(11.5)
                        .color(WARN_COLOR),
                    );
                });
            ui.add_space(4.0);
        }
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(lang.pick([
                    format!("ノート {notes} 個"),
                    format!("{notes} notes"),
                    format!("{notes} 個音符"),
                    format!("노트 {notes}개"),
                ]))
                .size(12.0)
                .color(if notes > 0 { TEXT_PRIMARY } else { TEXT_MUTED }),
            );
        });
        ui.add_space(2.0);
        let can_commit = self.position_scan.is_none() && notes > 0;
        let label = lang.pick([
            format!("譜面に追加（{notes} ノート）"),
            format!("Add to chart ({notes} notes)"),
            format!("加入譜面（{notes} 個音符）"),
            format!("채보에 추가 (노트 {notes}개)"),
        ]);
        let close_width = 76.0;
        let spacing = ui.spacing().item_spacing.x;
        let primary_width = (ui.available_width() - close_width - spacing).max(120.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    can_commit,
                    primary_button(egui::RichText::new(label).strong(), can_commit)
                        .min_size(egui::vec2(primary_width, 30.0)),
                )
                .clicked()
            {
                let commit = Commit {
                    slides: self.generated.slides.clone(),
                    transparent_steps: self.transparent_steps,
                    output: self.output(),
                };
                *self
                    .output
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(commit);
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if ui
                .add(
                    egui::Button::new(lang.pick(["閉じる", "Close", "關閉", "닫기"]))
                        .min_size(egui::vec2(close_width, 30.0)),
                )
                .clicked()
            {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.add_space(4.0);
        ui.separator();
        if let Some(available) = &self.update_available {
            update::footer(ui, available, self.lang);
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(crate::oss::title(self.lang))
                        .size(11.5)
                        .color(TEXT_MUTED),
                )
                .frame(false),
            )
            .clicked()
        {
            self.show_open_source_libraries = true;
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui, parent: &eframe::Frame) {
        let lang = self.lang;
        ui.add_space(4.0);
        section_card(ui, |ui| {
            let no_audio = self.audio.is_none();
            let open_label = lang.pick(["音声を開く", "Open audio", "開啟音訊", "오디오 열기"]);
            let open_button = if no_audio {
                primary_button(egui::RichText::new(open_label).strong(), true)
            } else {
                egui::Button::new(open_label)
            }
            .min_size(egui::vec2(ui.available_width(), 28.0));
            if ui
                .add_enabled(self.position_scan.is_none(), open_button)
                .clicked()
            {
                self.open_file(parent);
            }
            if !self.path.is_empty() {
                let display_name = std::path::Path::new(&self.path)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| self.path.clone());
                ui.add(egui::Label::new(egui::RichText::new(display_name).strong()).truncate())
                    .on_hover_text(&self.path);
            }
            let status_color = match &self.status {
                Msg::OpenAudioPrompt | Msg::AudioInfo { .. } => TEXT_MUTED,
                _ => egui::Color32::from_rgb(255, 118, 118),
            };
            ui.label(
                egui::RichText::new(self.status.text(lang))
                    .size(11.5)
                    .color(status_color),
            );
            if self.position_scan.is_some() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(lang.pick([
                        "音声の開始位置を計算中…",
                        "Calculating the audio start position…",
                        "正在計算音訊起始位置…",
                        "오디오 시작 위치를 계산하는 중…",
                    ]));
                });
                ui.ctx().request_repaint();
                return;
            }
            if let Some(audio) = &self.audio {
                let duration = audio.duration_secs();
                let seconds_suffix = lang.pick([" 秒", " s", " 秒", "초"]);
                ui.add_space(2.0);
                ui.separator();
                section_heading(
                    ui,
                    lang.pick(["取り込む範囲", "Import range", "匯入範圍", "가져올 범위"]),
                );
                egui::Grid::new("import_range_grid")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(lang.pick(["始点", "Start", "起點", "시작점"]));
                        ui.add(
                            egui::DragValue::new(&mut self.settings.offset_sec)
                                .range(-60.0..=duration)
                                .clamp_existing_to_range(false)
                                .speed(0.01)
                                .max_decimals(3)
                                .suffix(seconds_suffix),
                        );
                        ui.end_row();
                        let remaining = (duration - self.settings.offset_sec).max(0.0);
                        self.settings.length_sec = self.settings.length_sec.min(remaining);
                        ui.label(lang.pick(["長さ", "Length", "長度", "길이"]));
                        ui.add(
                            egui::DragValue::new(&mut self.settings.length_sec)
                                .range(remaining.min(0.05)..=remaining)
                                .speed(0.01)
                                .max_decimals(3)
                                .suffix(seconds_suffix),
                        );
                        ui.end_row();
                    });
            }
        });
        if self.position_scan.is_some() {
            return;
        }
        ui.add_space(6.0);
        section_card(ui, |ui| {
            egui::Grid::new("timing_grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.label("BPM");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.settings.bpm)
                                .range(1.0..=512.0)
                                .clamp_existing_to_range(false)
                                .speed(1.0),
                        );
                        if ui
                            .button(lang.pick([
                                "譜面から取得",
                                "Get from chart",
                                "從譜面取得",
                                "채보에서 가져오기",
                            ]))
                            .clicked()
                        {
                            self.take_chart_timing();
                        }
                    });
                    ui.end_row();

                    ui.label(lang.pick(["拍子", "Time signature", "拍號", "박자"]));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.settings.time_signature[0])
                                .range(1..=16)
                                .speed(1.0),
                        );
                        ui.label(egui::RichText::new("/").color(TEXT_MUTED));
                        ui.add(
                            egui::DragValue::new(&mut self.settings.time_signature[1])
                                .range(1..=480)
                                .speed(1.0),
                        );
                    });
                    ui.end_row();

                    ui.label(lang.pick(["量子化", "Quantize", "量化", "퀀타이즈"]));
                    self.settings.quantize_ticks = self.settings.quantize_ticks.clamp(1, 1920);
                    let selected = QUANTIZE
                        .iter()
                        .find(|(ticks, _)| *ticks == self.settings.quantize_ticks)
                        .map(|(_, name)| (*name).to_owned())
                        .unwrap_or_else(|| format!("{} tick", self.settings.quantize_ticks));
                    egui::ComboBox::from_id_salt("quantize")
                        .width(ui.available_width().max(140.0))
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            for (ticks, name) in QUANTIZE {
                                ui.selectable_value(
                                    &mut self.settings.quantize_ticks,
                                    *ticks,
                                    *name,
                                );
                            }
                        });
                    ui.end_row();
                });
            if !self.bpm_note.is_empty() {
                ui.add_space(2.0);
                let notes: Vec<String> = self.bpm_note.iter().map(|note| note.text(lang)).collect();
                ui.label(
                    egui::RichText::new(notes.join(" / "))
                        .size(11.5)
                        .color(WARN_COLOR),
                );
            }
        });
        ui.add_space(6.0);
        section_card(ui, |ui| {
            self.field_controls(ui);
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);
            self.output_controls(ui);
        });
        ui.add_space(6.0);
        section_card(ui, |ui| {
            let fine_rainbone = self.fine_rainbone();
            if !fine_rainbone {
                section_heading(
                    ui,
                    lang.pick([
                        "スライドの種類",
                        "Slide type",
                        "Slide 類型",
                        "슬라이드 종류",
                    ]),
                );
                let half = ((ui.available_width() - ui.spacing().item_spacing.x) * 0.5).max(80.0);
                ui.horizontal(|ui| {
                    if segmented_choice(
                        ui,
                        !self.settings.symmetric_width,
                        lang.pick(["普通の波形", "Normal waveform", "一般波形", "일반 파형"]),
                        half,
                    )
                    .clicked()
                    {
                        self.settings.symmetric_width = false;
                    }
                    if segmented_choice(
                        ui,
                        self.settings.symmetric_width,
                        lang.pick(["左右対称", "Symmetric", "左右對稱", "좌우 대칭"]),
                        half,
                    )
                    .clicked()
                    {
                        self.settings.symmetric_width = true;
                    }
                });
                ui.add_space(4.0);
            }
            if !self.settings.symmetric_width {
                self.settings.max_width = self.settings.max_width.max(self.settings.width);
            }
            let maximum_span = if self.settings.extended_field
                && (!self.settings.symmetric_width || fine_rainbone)
            {
                48
            } else {
                16
            };
            let slider_width = (ui.available_width() - 155.0).clamp(80.0, 220.0);
            egui::Grid::new("waveform_sliders_grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.spacing_mut().slider_width = slider_width;
                    // Fine reproduction picks widths 1 and 2 itself, replacing the slide type and the width.
                    if !fine_rainbone && !self.settings.symmetric_width {
                        ui.label(lang.pick(["幅", "Width", "寬度", "폭"]));
                        ui.add(egui::Slider::new(&mut self.settings.width, 1..=16));
                        ui.end_row();
                    }
                    let max_width_hover = lang.pick([
                        "波形全体の最大幅。普通の波形ではノート幅以上になります。",
                        "Maximum width of the whole waveform. A normal waveform is at least as wide as the note width.",
                        "整體波形的最大寬度。一般波形時不會小於音符寬度。",
                        "파형 전체의 최대 폭입니다. 일반 파형에서는 노트 폭 이상이 됩니다.",
                    ]);
                    ui.label(lang.pick(["最大幅", "Max width", "最大寬度", "최대 폭"]))
                        .on_hover_text(max_width_hover);
                    ui.add(egui::Slider::new(
                        &mut self.settings.max_width,
                        2..=maximum_span,
                    ))
                    .on_hover_text(max_width_hover);
                    ui.end_row();

                    ui.label(lang.pick([
                        "無音しきい値",
                        "Silence threshold",
                        "靜音閾值",
                        "무음 임계값",
                    ]));
                    ui.add(egui::Slider::new(
                        &mut self.settings.silence_threshold,
                        0.0..=0.5,
                    ));
                    ui.end_row();

                    ui.label(lang.pick(["平滑化", "Smoothing", "平滑化", "평활화"]));
                    ui.add(egui::Slider::new(&mut self.settings.smooth, 0..=8));
                    ui.end_row();
                });
            if !self.settings.symmetric_width && self.settings.max_width < self.settings.width {
                self.settings.max_width = self.settings.width;
                ui.ctx().request_repaint();
            }
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);
            if fine_rainbone || !self.settings.symmetric_width {
                ui.checkbox(
                    &mut self.settings.zigzag,
                    lang.pick([
                        "制御点を左右均等にする",
                        "Balance control points left and right",
                        "讓控制點左右均等",
                        "제어점을 좌우 균등하게",
                    ]),
                );
                ui.checkbox(
                    &mut self.settings.center_ends,
                    lang.pick([
                        "始点と終点を中央にする",
                        "Center the start and end",
                        "起點與終點置中",
                        "시작점과 끝점을 가운데로",
                    ]),
                );
            }
            ui.checkbox(
                &mut self.settings.remove_silent_points,
                lang.pick([
                    "無音部分の制御点・中継点をなくす",
                    "Remove control and step points in silent parts",
                    "移除靜音部分的控制點與中繼點",
                    "무음 구간의 제어점·중계점 없애기",
                ]),
            );
            if !self.rainbone {
                ui.checkbox(
                    &mut self.transparent_steps,
                    lang.pick([
                        "中継点を透明にする（節を置かない）",
                        "Make step points invisible (no joints)",
                        "中繼點設為透明（不放置節點）",
                        "중계점을 투명하게 (마디를 두지 않음)",
                    ]),
                );
            }
        });
        self.refresh();
        if self.audit.is_some() && !self.generated.slides.is_empty() {
            ui.add_space(6.0);
            self.audit_warnings(ui);
        }
        ui.add_space(6.0);
    }
}

impl Sound2SlideApp<'_> {
    fn output(&self) -> Output {
        if self.rainbone {
            Output::Rainbone {
                axis: self.rainbone_axis,
                color: self.rainbone_color,
                height: self.rainbone_height,
            }
        } else if self.slide_no_center_line {
            Output::SlideNoCenterLine
        } else {
            Output::Slide
        }
    }

    fn fine_rainbone(&self) -> bool {
        self.rainbone && self.rainbone_fine
    }

    /// Settings used for generation, with the rainbone's fine reproduction applied.
    fn generation_settings(&self) -> Settings {
        let mut settings = self.settings;
        if self.fine_rainbone() {
            settings.fine_center = true;
            settings.symmetric_width = false;
        }
        settings
    }

    fn select_chart(&mut self, path: Option<std::path::PathBuf>) {
        self.selected_chart_extended = path.as_deref().and_then(mgxc::read_extended_field);
        self.selected_chart = path;
        self.select_detected_field();
    }

    fn detected_extended_field(&self) -> Option<bool> {
        if self.host_extended_field {
            Some(true)
        } else {
            self.selected_chart_extended
        }
    }

    fn select_detected_field(&mut self) {
        if !self.field_manually_selected {
            self.field_mode = if self.detected_extended_field().unwrap_or(false) {
                FieldMode::Extended
            } else {
                FieldMode::Normal
            };
        }
        self.sync_field_mode();
    }

    fn select_field_mode(&mut self, mode: FieldMode) {
        self.field_manually_selected = true;
        self.field_mode = mode;
        self.sync_field_mode();
    }

    fn sync_field_mode(&mut self) {
        let previous_limit = if self.settings.extended_field { 48 } else { 16 };
        self.settings.extended_field = match self.field_mode {
            FieldMode::Normal => false,
            FieldMode::Extended => true,
        };
        let limit = if self.settings.extended_field { 48 } else { 16 };
        if limit != previous_limit && self.settings.max_width == previous_limit {
            self.settings.max_width = limit;
        }
        self.settings.width = self.settings.width.clamp(1, 16);
        self.settings.max_width = self.settings.max_width.clamp(2, limit);
    }

    fn field_controls(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        section_heading(ui, lang.pick(["フィールド", "Field", "遊戲區域", "필드"]));
        let half = ((ui.available_width() - ui.spacing().item_spacing.x) * 0.5).max(80.0);
        ui.horizontal(|ui| {
            for mode in [FieldMode::Normal, FieldMode::Extended] {
                if segmented_choice(ui, self.field_mode == mode, mode.label(lang), half).clicked() {
                    self.select_field_mode(mode);
                }
            }
        });
        if !self.field_manually_selected {
            if !self.chart_files.is_empty() {
                let mut selected = self.selected_chart.clone();
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(lang.pick([
                        "判定元の譜面",
                        "Chart for detection",
                        "判定來源譜面",
                        "판정할 채보",
                    ]))
                    .size(11.5)
                    .color(TEXT_MUTED),
                );
                egui::ComboBox::from_id_salt("field_chart")
                    .width(ui.available_width().max(140.0))
                    .selected_text(
                        selected
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| {
                                lang.pick(["未選択", "Not selected", "未選擇", "선택 안 함"])
                                    .into()
                            }),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut selected,
                            None,
                            lang.pick(["未選択", "Not selected", "未選擇", "선택 안 함"]),
                        );
                        for file in &self.chart_files {
                            ui.selectable_value(
                                &mut selected,
                                Some(file.clone()),
                                file.file_name().unwrap_or_default().to_string_lossy(),
                            );
                        }
                    });
                if selected != self.selected_chart {
                    self.select_chart(selected);
                }
            }
            if self.selected_chart.is_some() {
                ui.label(
                    egui::RichText::new(lang.pick([
                        "保存済みの設定を参照します。未保存の変更は反映されません。",
                        "Uses saved settings; unsaved changes are not reflected.",
                        "使用已儲存的設定，不反映尚未儲存的變更。",
                        "저장된 설정을 사용하며 저장하지 않은 변경은 반영되지 않습니다.",
                    ]))
                    .size(11.0)
                    .color(TEXT_MUTED),
                );
            }
        }
        self.sync_field_mode();
    }

    fn output_controls(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        section_heading(ui, lang.pick(["出力", "Output", "輸出", "출력"]));
        let half = ((ui.available_width() - ui.spacing().item_spacing.x) * 0.5).max(80.0);
        ui.horizontal(|ui| {
            if segmented_choice(
                ui,
                !self.rainbone,
                lang.pick(["スライド", "Slide", "Slide", "슬라이드"]),
                half,
            )
            .clicked()
            {
                self.rainbone = false;
            }
            if segmented_choice(
                ui,
                self.rainbone,
                lang.pick(["レインボーン", "Rainbone", "Rainbone", "레인본"]),
                half,
            )
            .on_hover_text(lang.pick([
                "AIR-CRUSH のノーツを置かず、空中の線だけを追加します。",
                "Adds only the air line of an AIR-CRUSH, without crush notes.",
                "不放置 AIR-CRUSH 音符，只加入空中的線。",
                "AIR-CRUSH 노트 없이 공중의 선만 추가합니다.",
            ]))
            .clicked()
            {
                self.rainbone = true;
            }
        });
        ui.add_space(2.0);
        if !self.rainbone {
            ui.checkbox(
                &mut self.slide_no_center_line,
                lang.pick([
                    "中央線を表示しない",
                    "Hide center line",
                    "不顯示中央線",
                    "중앙선 숨기기",
                ]),
            );
            return;
        }
        egui::Grid::new("rainbone_grid")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label(lang.pick(["線の軸", "Line axis", "線的軸向", "선 축"]));
                egui::ComboBox::from_id_salt("rainbone_axis")
                    .width(ui.available_width().max(140.0))
                    .selected_text(self.rainbone_axis.label(lang))
                    .show_ui(ui, |ui| {
                        for axis in [rainbone::Axis::X, rainbone::Axis::Y, rainbone::Axis::Z] {
                            ui.selectable_value(&mut self.rainbone_axis, axis, axis.label(lang));
                        }
                    })
                    .response
                    .on_hover_text(lang.pick([
                        "線の描画軸を指定します。プレビューは配置経路を表示し、軸による立体的な描画の違いは再現しません。",
                        "Sets the line rendering axis. The preview shows the path, without reproducing its 3D orientation.",
                        "指定線的繪製軸向。預覽顯示配置路徑，不重現軸向造成的立體繪製差異。",
                        "선의 렌더링 축을 지정합니다. 미리보기는 배치 경로를 표시하며 축에 따른 입체 표현 차이는 재현하지 않습니다.",
                    ]));
                ui.end_row();

                ui.label(lang.pick(["線の色", "Line color", "線的顏色", "선 색"]));
                let selected = rainbone::color(self.rainbone_color);
                egui::ComboBox::from_id_salt("rainbone_color")
                    .width(ui.available_width().max(140.0))
                    .selected_text(line_color_text(selected, lang))
                    .show_ui(ui, |ui| {
                        for color in rainbone::COLORS {
                            ui.selectable_value(
                                &mut self.rainbone_color,
                                color.id,
                                line_color_text(color, lang),
                            );
                        }
                    });
                ui.end_row();

                ui.label(lang.pick(["高さ", "Height", "高度", "높이"]));
                ui.add(
                    egui::DragValue::new(&mut self.rainbone_height)
                        .range(0..=rainbone::MAX_HEIGHT)
                        .speed(1.0),
                );
                ui.end_row();
            });
        ui.add_space(2.0);
        ui.checkbox(
            &mut self.rainbone_fine,
            lang.pick([
                "さらに細かく波形を再現する",
                "Reproduce the waveform more finely",
                "更細緻地重現波形",
                "파형을 더 세밀하게 재현",
            ]),
        )
        .on_hover_text(lang.pick([
            "線はノートの中央に引かれます。1幅と2幅を使い分けて、線をレーンの中央と境目の両方に乗せます。",
            "The line runs through the center of each note. Mixing widths 1 and 2 puts it both on lane centers and on lane borders.",
            "線會畫在音符的中央。交替使用 1 寬與 2 寬，讓線同時落在軌道中央與軌道交界。",
            "선은 노트의 가운데를 지납니다. 1폭과 2폭을 섞어 선을 레인 가운데와 경계 양쪽에 올립니다.",
        ]));
    }
}

fn line_color_text(color: &rainbone::LineColor, lang: Lang) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let font_id = egui::FontId::proportional(13.0);
    match color.rgb {
        Some([r, g, b]) => {
            job.append(
                "● ",
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: egui::Color32::from_rgb(r, g, b),
                    ..Default::default()
                },
            );
        }
        None => {
            job.append(
                "○ ",
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: TEXT_MUTED,
                    ..Default::default()
                },
            );
        }
    }
    job.append(
        &format!("({}) {}", color.key, lang.pick(color.names)),
        0.0,
        egui::TextFormat {
            font_id,
            color: TEXT_PRIMARY,
            ..Default::default()
        },
    );
    job
}

fn event_walking_back<T>(
    start_position: i32,
    mut lookup: impl FnMut(i32) -> Result<Option<T>, Msg>,
) -> Result<Option<T>, Msg> {
    let mut position = start_position;
    loop {
        if let Some(value) = lookup(position)? {
            return Ok(Some(value));
        }
        if position <= 0 {
            return Ok(None);
        }
        position -= 1;
    }
}

impl Sound2SlideApp<'_> {
    fn sync_silent_zones(&mut self) {
        let span = self
            .audio
            .as_ref()
            .and_then(|audio| {
                preview_range(audio, self.settings.offset_sec, self.settings.length_sec)
            })
            .map(|(start, end)| end - start)
            .unwrap_or(0.0);
        let mut changed = false;
        self.silent_zones.retain_mut(|zone| {
            if zone.start_sec >= span {
                changed = true;
                return false;
            }
            if zone.end_sec > span {
                zone.end_sec = span;
                changed = true;
            }
            true
        });
        if changed {
            self.zone_drag = None;
        }
    }

    fn preview_header(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let response = ui
            .horizontal(|ui| {
                if self.position_scan.is_none() {
                    self.playback_controls(ui);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.silent_zone_button(ui)
                })
                .inner
            })
            .inner;
        let lang = self.lang;
        if let Some(error) = &self.playback_error {
            ui.colored_label(egui::Color32::from_rgb(255, 118, 118), error.text(lang));
        }
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(lang.pick(["表示設定", "Display", "顯示設定", "표시 설정"]))
                    .color(TEXT_MUTED),
            );
            egui::ComboBox::from_id_salt("lane_divisions")
                .selected_text(lane_division_label(self.lane_divisions, lang))
                .show_ui(ui, |ui| {
                    for divisions in [16, 8, 4, 2, 0] {
                        ui.selectable_value(
                            &mut self.lane_divisions,
                            divisions,
                            lane_division_label(divisions, lang),
                        );
                    }
                });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.editing_silent_zones {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(lang.pick([
                                "サイレントゾーン編集中",
                                "Editing silent zones",
                                "正在編輯靜音區",
                                "무음 구간 편집 중",
                            ]))
                            .size(11.5)
                            .color(ACCENT_BRIGHT),
                        )
                        .truncate(),
                    );
                } else if self.audio.is_some() {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(lang.pick([
                                "Space: 再生/一時停止",
                                "Space: Play/Pause",
                                "Space：播放/暫停",
                                "Space: 재생/일시 정지",
                            ]))
                            .size(11.5)
                            .color(TEXT_MUTED),
                        )
                        .truncate(),
                    );
                }
            });
        });
        ui.separator();
        response
    }

    fn silent_zone_button(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let lang = self.lang;
        let response = ui
            .add_enabled(
                self.audio.is_some(),
                egui::Button::new("")
                    .min_size(egui::vec2(28.0, 24.0))
                    .selected(self.editing_silent_zones),
            )
            .on_hover_text(lang.pick([
                "サイレントゾーン設定：縦ドラッグで追加・上下の境界や秒数で調整",
                "Silent zones: drag vertically to add; adjust with the top and bottom edges or the seconds",
                "靜音區設定：縱向拖曳以新增，可用上下邊界或秒數調整",
                "무음 구간 설정: 세로로 드래그하여 추가, 위아래 경계나 초 값으로 조정",
            ]));
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                self.audio.is_some(),
                self.editing_silent_zones,
                lang.pick([
                    "サイレントゾーン設定",
                    "Silent zones",
                    "靜音區設定",
                    "무음 구간 설정",
                ]),
            )
        });
        let center = response.rect.center();
        let color = if self.editing_silent_zones {
            ui.visuals().selection.stroke.color
        } else {
            ui.style().interact(&response).fg_stroke.color
        };
        let painter = ui.painter();
        painter.rect_filled(
            egui::Rect::from_min_max(
                center + egui::vec2(-9.0, -3.0),
                center + egui::vec2(-5.0, 3.0),
            ),
            egui::CornerRadius::ZERO,
            color,
        );
        painter.add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(-5.0, -3.0),
                center + egui::vec2(0.0, -7.0),
                center + egui::vec2(0.0, 7.0),
                center + egui::vec2(-5.0, 3.0),
            ],
            color,
            egui::Stroke::NONE,
        ));
        for (a, b) in [((3.0, -3.0), (9.0, 3.0)), ((3.0, 3.0), (9.0, -3.0))] {
            painter.line_segment(
                [center + egui::vec2(a.0, a.1), center + egui::vec2(b.0, b.1)],
                egui::Stroke::new(1.8, color),
            );
        }
        if response.clicked() {
            self.editing_silent_zones = !self.editing_silent_zones;
            self.zone_drag = None;
            ui.ctx().request_repaint();
        }
        response
    }

    fn silent_zone_controls(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        let Some((start, end)) = self.audio.as_ref().and_then(|audio| {
            preview_range(audio, self.settings.offset_sec, self.settings.length_sec)
        }) else {
            self.zone_drag = None;
            return;
        };
        let inner = response.rect.shrink(16.0);
        if inner.width() <= 0.0 || inner.height() <= 0.0 {
            return;
        }
        let span = end - start;
        let minimum = 1.0 / f64::from(self.audio.as_ref().unwrap().sample_rate);
        let lang = self.lang;
        let suffix = lang.pick([" 秒", " s", " 秒", "초"]);
        let time_y = |elapsed: f64| {
            inner.bottom() - (elapsed / span).clamp(0.0, 1.0) as f32 * inner.height()
        };
        let mut changed = false;
        let mut remove = None;
        for (index, zone) in self.silent_zones.iter_mut().enumerate() {
            if zone.end_sec <= 0.0 || zone.start_sec >= span {
                continue;
            }
            let top = time_y(zone.end_sec);
            let bottom = time_y(zone.start_sec);
            for (is_end, y) in [(true, top), (false, bottom)] {
                let handle = egui::Rect::from_min_max(
                    egui::pos2(inner.left(), y - 5.0),
                    egui::pos2(inner.right(), y + 5.0),
                )
                .intersect(response.rect);
                let drag = ui
                    .interact(
                        handle,
                        response.id.with(("silent_zone_boundary", index, is_end)),
                        egui::Sense::drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::ResizeVertical);
                if drag.drag_started_by(egui::PointerButton::Primary) {
                    self.zone_drag = Some(ZoneDrag::Resize { index, end: is_end });
                }
            }
            // Put short-zone labels outside the zone so both bounds remain editable.
            let short = bottom - top < 48.0;
            let top_label_y = if short { top - 24.0 } else { top + 6.0 };
            let bottom_label_y = if short { bottom + 6.0 } else { bottom - 28.0 };
            let label_rect = |y: f32| {
                egui::Rect::from_min_size(
                    egui::pos2(
                        inner.left(),
                        y.clamp(response.rect.top(), response.rect.bottom() - 22.0),
                    ),
                    egui::vec2(88.0, 22.0),
                )
            };
            let max_start = (zone.end_sec.min(span) - minimum).max(0.0);
            changed |= ui
                .push_id(("silent_zone_start", index), |ui| {
                    ui.put(
                        label_rect(bottom_label_y),
                        egui::DragValue::new(&mut zone.start_sec)
                            .range(0.0..=max_start)
                            .clamp_existing_to_range(false)
                            .speed(0.001)
                            .max_decimals(3)
                            .suffix(suffix),
                    )
                    .on_hover_text(lang.pick([
                        "サイレントゾーン開始（取り込み開始からの経過秒数）",
                        "Silent zone start (seconds from the import start)",
                        "靜音區開始（自匯入起點起算的秒數）",
                        "무음 구간 시작 (가져오기 시작부터의 경과 초)",
                    ]))
                    .changed()
                })
                .inner;
            changed |= ui
                .push_id(("silent_zone_end", index), |ui| {
                    ui.put(
                        label_rect(top_label_y),
                        egui::DragValue::new(&mut zone.end_sec)
                            .range((zone.start_sec + minimum).min(span)..=span)
                            .clamp_existing_to_range(false)
                            .speed(0.001)
                            .max_decimals(3)
                            .suffix(suffix),
                    )
                    .on_hover_text(lang.pick([
                        "サイレントゾーン終了（取り込み開始からの経過秒数）",
                        "Silent zone end (seconds from the import start)",
                        "靜音區結束（自匯入起點起算的秒數）",
                        "무음 구간 끝 (가져오기 시작부터의 경과 초)",
                    ]))
                    .changed()
                })
                .inner;
            let delete_rect = egui::Rect::from_min_size(
                egui::pos2(
                    inner.right() - 22.0,
                    (top + 6.0).min(response.rect.bottom() - 22.0),
                ),
                egui::vec2(22.0, 22.0),
            );
            if ui
                .push_id(("delete_silent_zone", index), |ui| {
                    ui.put(delete_rect, egui::Button::new("×"))
                        .on_hover_text(lang.pick([
                            "サイレントゾーンを削除",
                            "Delete silent zone",
                            "刪除靜音區",
                            "무음 구간 삭제",
                        ]))
                        .clicked()
                })
                .inner
            {
                remove = Some(index);
            }
        }
        if let Some(index) = remove {
            self.silent_zones.remove(index);
            self.zone_drag = None;
            ui.ctx().request_repaint();
            return;
        }
        if self.zone_drag.is_none()
            && response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|input| input.pointer.press_origin())
            && inner.contains(origin)
        {
            let anchor = zone_elapsed(inner, origin.y, span);
            let index = self.silent_zones.len();
            self.silent_zones.push(SilentZone {
                start_sec: anchor,
                end_sec: anchor,
            });
            self.zone_drag = Some(ZoneDrag::Create { index, anchor });
        }
        if let Some(drag) = self.zone_drag {
            if let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) {
                let elapsed = zone_elapsed(inner, pointer.y, span);
                match drag {
                    ZoneDrag::Create { index, anchor } => {
                        self.silent_zones[index] = SilentZone {
                            start_sec: anchor.min(elapsed),
                            end_sec: anchor.max(elapsed),
                        };
                    }
                    ZoneDrag::Resize { index, end } => {
                        let zone = &mut self.silent_zones[index];
                        if end && elapsed < zone.start_sec {
                            zone.end_sec = zone.start_sec;
                            zone.start_sec = elapsed;
                            self.zone_drag = Some(ZoneDrag::Resize { index, end: false });
                        } else if !end && elapsed > zone.end_sec {
                            zone.start_sec = zone.end_sec;
                            zone.end_sec = elapsed;
                            self.zone_drag = Some(ZoneDrag::Resize { index, end: true });
                        } else if end {
                            zone.end_sec = elapsed.max((zone.start_sec + minimum).min(span));
                        } else {
                            zone.start_sec = elapsed.min((zone.end_sec - minimum).max(0.0));
                        }
                    }
                }
                changed = true;
            }
            if !ui.input(|input| input.pointer.primary_down()) {
                self.zone_drag = None;
                self.silent_zones
                    .retain(|zone| zone.end_sec > zone.start_sec);
            }
        }
        if changed {
            ui.ctx().request_repaint();
        }
    }

    fn take_chart_timing(&mut self) {
        let mut notes = Vec::new();
        match event_walking_back(self.start_tick, self.lookup_bpm) {
            Ok(Some(bpm)) => {
                self.settings.bpm = bpm;
            }
            Ok(None) => notes.push(Msg::BpmEventNotFound),
            Err(message) => notes.push(message),
        }
        // Margrete's event API indexes beat changes by bar; its tick coordinates use 1920 per bar.
        let start_bar = self.start_tick.div_euclid(TICKS_PER_BEAT * 4).max(0);
        match event_walking_back(start_bar, self.lookup_signature) {
            Ok(Some(signature)) => {
                self.settings.time_signature = signature;
            }
            Ok(None) => notes.push(Msg::SignatureEventNotFound),
            Err(message) => notes.push(message),
        }
        self.bpm_note = notes;
    }

    fn sync_playback(&mut self) {
        if self.playback.as_ref().is_some_and(|playback| {
            playback.is_finished()
                || playback.offset_sec != self.settings.offset_sec
                || playback.length_sec != self.settings.length_sec
        }) {
            self.playback = None;
        }
    }

    fn can_play(&self) -> bool {
        self.position_scan.is_none()
            && self.offset_selection.is_none()
            && self.audio.as_ref().is_some_and(|audio| {
                preview_range(audio, self.settings.offset_sec, self.settings.length_sec).is_some()
            })
    }

    fn toggle_playback(&mut self) {
        self.sync_playback();
        if !self.can_play() {
            return;
        }
        if let Some(playback) = &self.playback {
            playback.toggle_pause();
        } else if let Some(audio) = &self.audio {
            match Playback::start(audio, self.settings.offset_sec, self.settings.length_sec) {
                Ok(playback) => {
                    self.playback = Some(playback);
                    self.playback_error = None;
                }
                Err(error) => self.playback_error = Some(error),
            }
        }
    }

    fn seek_playback(&mut self, position_sec: f64) {
        if !self.can_play() {
            return;
        }
        self.playback = None;
        if let Some(audio) = &self.audio {
            match Playback::start_at(
                audio,
                self.settings.offset_sec,
                self.settings.length_sec,
                position_sec,
                true,
            ) {
                Ok(playback) => {
                    self.playback = Some(playback);
                    self.playback_error = None;
                }
                Err(error) => self.playback_error = Some(error),
            }
        }
    }

    fn playback_controls(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let playing = self
            .playback
            .as_ref()
            .is_some_and(|playback| !playback.is_finished() && !playback.is_paused());
        let label = if playing {
            lang.pick(["一時停止", "Pause", "暫停", "일시 정지"])
        } else {
            lang.pick(["再生", "Play", "播放", "재생"])
        };
        let play_button = if playing {
            primary_button(label, true)
        } else {
            egui::Button::new(label)
        };
        if ui.add_enabled(self.can_play(), play_button).clicked() {
            self.toggle_playback();
        }
        if ui
            .add_enabled(
                self.playback.is_some(),
                egui::Button::new(lang.pick(["停止", "Stop", "停止", "정지"])),
            )
            .clicked()
        {
            self.playback = None;
        }
        let elapsed = self
            .playback
            .as_ref()
            .map(|playback| (playback.position_sec() - playback.offset_sec).max(0.0))
            .unwrap_or(0.0);
        egui::Frame::new()
            .fill(BG_INPUT)
            .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE))
            .corner_radius(egui::CornerRadius::same(4))
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(lang.pick([
                        format!("{elapsed:.3} 秒"),
                        format!("{elapsed:.3} s"),
                        format!("{elapsed:.3} 秒"),
                        format!("{elapsed:.3}초"),
                    ]))
                    .monospace()
                    .size(12.0)
                    .color(if self.playback.is_some() {
                        TEXT_PRIMARY
                    } else {
                        TEXT_MUTED
                    }),
                );
            });
        if let Some(playback) = &self.playback {
            let interval_ms = if playback.is_paused() { 100 } else { 16 };
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(interval_ms));
        }
    }
}

impl eframe::App for Sound2SlideApp<'_> {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(host) = self.host {
            host.restore();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if let Some(check) = &mut self.update_check
            && let Some(release) = check.poll(ui.ctx())
        {
            self.update_available = Some(release);
            self.update_dialog_open = true;
        }
        let toggle_playback = !self.update_dialog_open && consume_playback_shortcut(ui.ctx());
        self.advance_position_scan(ui.ctx());
        self.sync_playback();
        self.refresh();
        egui::Panel::left("controls")
            .resizable(true)
            .min_size(300.0)
            .default_size(348.0)
            .show(ui, |ui| {
                ui.add_enabled_ui(
                    self.offset_selection.is_none() && !self.update_dialog_open,
                    |ui| {
                        egui::Panel::top("controls_header")
                            .resizable(false)
                            .show(ui, |ui| self.controls_header(ui));
                        egui::Panel::bottom("controls_footer")
                            .resizable(false)
                            .show(ui, |ui| self.controls_footer(ui));
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.controls(ui, frame));
                    },
                );
            });
        self.refresh();
        self.sync_playback();
        if toggle_playback {
            self.toggle_playback();
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if self.offset_selection.is_some() || self.update_dialog_open {
                ui.disable();
            }
            self.preview_header(ui);
            let available = ui.available_size();
            let sense = if self.editing_silent_zones {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::click()
            };
            let (response, painter) = ui.allocate_painter(available, sense);
            if !self.editing_silent_zones
                && response.clicked()
                && let Some(pointer) = response.interact_pointer_pos()
                && let Some(audio) = &self.audio
                && let Some(position) =
                    preview_click_position(response.rect, pointer, audio, &self.settings)
            {
                self.seek_playback(position);
                ui.ctx().request_repaint();
            }
            paint_preview(
                &painter,
                response.rect,
                self.audio.as_ref(),
                &self.settings,
                &self.generated.slides,
                self.output(),
                self.start_tick,
                self.settings.time_signature,
                self.lane_divisions,
                self.playback.as_ref().map(Playback::position_sec),
                &self.silent_zones,
                self.editing_silent_zones,
                self.lang,
            );
            if self.editing_silent_zones {
                self.silent_zone_controls(ui, &response);
            }
        });
        self.refresh();
        self.offset_selection_dialog(ui.ctx());
        crate::oss::show(ui.ctx(), &mut self.show_open_source_libraries, self.lang);
        if self.offset_selection.is_none()
            && let Some(available) = &self.update_available
        {
            update::show(ui.ctx(), available, &mut self.update_dialog_open, self.lang);
        }
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
}

fn consume_playback_shortcut(ctx: &egui::Context) -> bool {
    // Consume before widgets so Space cannot also activate a focused button or repeat the toggle.
    ctx.input_mut(|input| {
        let mut toggle = false;
        input.events.retain(|event| {
            if let egui::Event::Key {
                key: egui::Key::Space,
                pressed,
                repeat,
                ..
            } = event
            {
                toggle |= *pressed && !*repeat;
                false
            } else {
                !matches!(event, egui::Event::Text(text) if text == " ")
            }
        });
        toggle
    })
}

fn lane_division_label(divisions: i32, lang: Lang) -> String {
    if divisions == 0 {
        lang.pick(["分割なし", "No lanes", "不分割", "분할 없음"])
            .into()
    } else {
        lang.pick([
            format!("{divisions}分割"),
            format!("{divisions} lanes"),
            format!("{divisions} 等分"),
            format!("{divisions}분할"),
        ])
    }
}

fn preview_beats(
    start_tick: i32,
    span_sec: f64,
    bpm: f64,
    time_signature: [i32; 2],
    height: f32,
) -> Vec<(f64, bool)> {
    if !span_sec.is_finite() || span_sec <= 0.0 || !bpm.is_finite() || bpm <= 0.0 || height <= 0.0 {
        return Vec::new();
    }
    let [numerator, denominator] = time_signature;
    let denominator = f64::from(denominator.clamp(1, 480));
    let seconds_per_beat = 60.0 / bpm * 4.0 / denominator;
    let start_beat =
        f64::from(start_tick) * f64::from(numerator.clamp(1, 16)) / f64::from(TICKS_PER_BEAT * 4);
    let end_beat = start_beat + span_sec / seconds_per_beat;
    let beats_per_bar = i64::from(numerator.clamp(1, 16));
    let pixels_per_beat = f64::from(height) * seconds_per_beat / span_sec;
    // Dense previews keep aligned bar lines instead of painting overlapping beats.
    let stride = if pixels_per_beat < 3.0 {
        ((3.0 / pixels_per_beat / beats_per_bar as f64).ceil() as i64)
            .max(1)
            .saturating_mul(beats_per_bar)
    } else {
        1
    };
    let first = (start_beat / stride as f64).ceil() as i64;
    let last = (end_beat / stride as f64).floor() as i64;
    (first..=last)
        .map(|index| {
            let beat = index.saturating_mul(stride);
            (
                (beat as f64 - start_beat) * seconds_per_beat,
                beat.rem_euclid(beats_per_bar) == 0,
            )
        })
        .collect()
}

fn preview_click_position(
    rect: egui::Rect,
    pointer: egui::Pos2,
    audio: &Audio,
    settings: &Settings,
) -> Option<f64> {
    let inner = rect.shrink(16.0);
    if !inner.contains(pointer) || inner.height() <= 0.0 || inner.width() <= 0.0 {
        return None;
    }
    let (start, end) = preview_range(audio, settings.offset_sec, settings.length_sec)?;
    let fraction = f64::from((inner.bottom() - pointer.y) / inner.height());
    // The top edge is the exclusive end; keep one sample available for playback.
    let last_sample = (end - 1.0 / f64::from(audio.sample_rate)).max(start);
    Some((start + fraction * (end - start)).clamp(start, last_sample))
}

fn lane_fraction(lane: f32, extended: bool) -> f32 {
    let (left, span) = if extended { (-16.0, 48.0) } else { (0.0, 16.0) };
    ((lane - left) / span).clamp(0.0, 1.0)
}

#[allow(clippy::too_many_arguments)]
fn paint_preview(
    painter: &egui::Painter,
    rect: egui::Rect,
    audio: Option<&Audio>,
    settings: &Settings,
    slides: &[Slide],
    output: Output,
    start_tick: i32,
    time_signature: [i32; 2],
    lane_divisions: i32,
    playback_position_sec: Option<f64>,
    silent_zones: &[SilentZone],
    editing_silent_zones: bool,
    lang: Lang,
) {
    painter.rect_filled(
        rect,
        egui::CornerRadius::same(6),
        egui::Color32::from_rgb(13, 15, 22),
    );
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(6),
        egui::Stroke::new(1.0, BORDER_SUBTLE),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(16.0);
    let Some(audio) = audio else {
        painter.rect_filled(
            inner,
            egui::CornerRadius::same(4),
            egui::Color32::from_rgb(18, 21, 30),
        );
        painter.rect_stroke(
            inner,
            egui::CornerRadius::same(4),
            egui::Stroke::new(1.0, BORDER_SUBTLE),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            Msg::OpenAudioPrompt.text(lang),
            egui::FontId::proportional(16.0),
            TEXT_MUTED,
        );
        return;
    };
    if audio.sample_rate == 0 || audio.samples.is_empty() {
        return;
    }
    let Some((view_start, view_end)) =
        preview_range(audio, settings.offset_sec, settings.length_sec)
    else {
        return;
    };
    painter.rect_filled(
        inner,
        egui::CornerRadius::ZERO,
        egui::Color32::from_rgb(19, 23, 33),
    );
    let span = view_end - view_start;
    let time_y = |time: f64| -> f32 {
        let t = ((time - view_start) / span) as f32;
        inner.bottom() - t.clamp(0.0, 1.0) * inner.height()
    };
    let lane_x = |lane: f32| -> f32 {
        let t = lane_fraction(lane, settings.extended_field);
        inner.left() + t * inner.width()
    };
    if settings.extended_field {
        let wing_fill = egui::Color32::from_black_alpha(52);
        painter.rect_filled(
            egui::Rect::from_min_max(inner.left_top(), egui::pos2(lane_x(0.0), inner.bottom())),
            egui::CornerRadius::ZERO,
            wing_fill,
        );
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(lane_x(16.0), inner.top()), inner.right_bottom()),
            egui::CornerRadius::ZERO,
            wing_fill,
        );
    }
    for zone in silent_zones {
        if zone.end_sec <= 0.0 || zone.start_sec >= span {
            continue;
        }
        let top = time_y(view_start + zone.end_sec);
        let bottom = time_y(view_start + zone.start_sec);
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(inner.left(), top),
                egui::pos2(inner.right(), bottom),
            ),
            egui::CornerRadius::ZERO,
            egui::Color32::from_rgba_unmultiplied(80, 155, 220, 55),
        );
        for y in [top, bottom] {
            painter.line_segment(
                [egui::pos2(inner.left(), y), egui::pos2(inner.right(), y)],
                egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 190, 245)),
            );
            if editing_silent_zones {
                let cx = inner.center().x;
                painter.line_segment(
                    [egui::pos2(cx - 14.0, y), egui::pos2(cx + 14.0, y)],
                    egui::Stroke::new(3.0, egui::Color32::from_rgb(160, 222, 255)),
                );
            }
        }
    }
    if lane_divisions > 0 {
        let panels = if settings.extended_field { 3 } else { 1 };
        let left = if settings.extended_field { -16.0 } else { 0.0 };
        for division in 0..=lane_divisions * panels {
            let x = lane_x(left + division as f32 * 16.0 / lane_divisions as f32);
            painter.line_segment(
                [egui::pos2(x, inner.top()), egui::pos2(x, inner.bottom())],
                egui::Stroke::new(1.0, egui::Color32::from_white_alpha(18)),
            );
        }
    }
    let center_x = lane_x(8.0);
    painter.line_segment(
        [
            egui::pos2(center_x, inner.top()),
            egui::pos2(center_x, inner.bottom()),
        ],
        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(30)),
    );
    if settings.extended_field {
        for lane in [0.0, 16.0] {
            let x = lane_x(lane);
            painter.line_segment(
                [egui::pos2(x, inner.top()), egui::pos2(x, inner.bottom())],
                egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 165, 220)),
            );
        }
    }
    for (seconds, is_bar) in preview_beats(
        start_tick,
        span,
        settings.bpm,
        time_signature,
        inner.height(),
    ) {
        let y = time_y(view_start + seconds);
        let stroke = if is_bar {
            egui::Stroke::new(1.5, egui::Color32::from_white_alpha(96))
        } else {
            egui::Stroke::new(1.0, egui::Color32::from_white_alpha(30))
        };
        painter.line_segment(
            [egui::pos2(inner.left(), y), egui::pos2(inner.right(), y)],
            stroke,
        );
    }
    let ticks_per_second = ticks_per_second(settings.bpm.max(1.0), settings.time_signature);
    for slide in slides {
        if let Output::Rainbone { color, .. } = output {
            paint_rainbone(
                painter,
                slide,
                rainbone::color(color),
                start_tick,
                settings.offset_sec,
                ticks_per_second,
                &lane_x,
                &time_y,
            );
            continue;
        }
        paint_slider(
            painter,
            slide,
            output != Output::SlideNoCenterLine,
            start_tick,
            settings.offset_sec,
            ticks_per_second,
            &lane_x,
            &time_y,
        );
    }
    let field_border = if editing_silent_zones {
        egui::Stroke::new(1.5, ACCENT_BRIGHT)
    } else {
        egui::Stroke::new(1.0, BORDER_SUBTLE)
    };
    painter.rect_stroke(
        inner,
        egui::CornerRadius::ZERO,
        field_border,
        egui::StrokeKind::Outside,
    );
    if let Some(position) = playback_position_sec {
        let y = time_y(position);
        let playhead = egui::Color32::from_rgb(255, 64, 64);
        painter.line_segment(
            [egui::pos2(inner.left(), y), egui::pos2(inner.right(), y)],
            egui::Stroke::new(2.0, playhead),
        );
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(inner.left() - 7.0, y - 4.0),
                egui::pos2(inner.left(), y),
                egui::pos2(inner.left() - 7.0, y + 4.0),
            ],
            playhead,
            egui::Stroke::NONE,
        ));
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(inner.right() + 7.0, y - 4.0),
                egui::pos2(inner.right() + 7.0, y + 4.0),
                egui::pos2(inner.right(), y),
            ],
            playhead,
            egui::Stroke::NONE,
        ));
    }
}

fn paint_slider(
    painter: &egui::Painter,
    slide: &Slide,
    show_center_line: bool,
    start_tick: i32,
    offset_sec: f64,
    ticks_per_second: f64,
    lane_x: &impl Fn(f32) -> f32,
    time_y: &impl Fn(f64) -> f32,
) {
    if slide.points.len() < 2 {
        return;
    }
    let placed: Vec<(egui::Pos2, egui::Pos2, egui::Pos2, f32)> = slide
        .points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let time =
                offset_sec + f64::from(point.tick.saturating_sub(start_tick)) / ticks_per_second;
            let y = time_y(time);
            let left = lane_x(point.x as f32);
            let right = lane_x(point.x as f32 + point.width as f32);
            let center = (left + right) * 0.5;
            let t = index as f32 / (slide.points.len() - 1) as f32;
            (
                egui::pos2(left, y),
                egui::pos2(center, y),
                egui::pos2(right, y),
                t,
            )
        })
        .collect();
    let mut mesh = egui::epaint::Mesh::default();
    for pair in placed.windows(2) {
        let (left0, center0, right0, t0) = pair[0];
        let (left1, center1, right1, t1) = pair[1];
        add_gradient_quad(
            &mut mesh,
            left0,
            center0,
            left1,
            center1,
            edge_color(t0),
            center_color(t0),
            edge_color(t1),
            center_color(t1),
        );
        add_gradient_quad(
            &mut mesh,
            center0,
            right0,
            center1,
            right1,
            center_color(t0),
            edge_color(t0),
            center_color(t1),
            edge_color(t1),
        );
    }
    if !mesh.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }
    let center_line = egui::Color32::from_rgb(90, 255, 236);
    // The center line keeps a constant horizontal width, so it gets thinner the more a
    // segment leans sideways, just like the in-game slide.
    let half = egui::vec2(1.2, 0.0);
    for pair in placed.windows(2).filter(|_| show_center_line) {
        let (mut top, mut bottom) = (pair[0].1, pair[1].1);
        if top.y > bottom.y {
            std::mem::swap(&mut top, &mut bottom);
        }
        // egui feathers convex polygons correctly only when they wind clockwise.
        painter.add(egui::Shape::convex_polygon(
            vec![top - half, top + half, bottom + half, bottom - half],
            center_line,
            egui::Stroke::NONE,
        ));
    }
    let cap_height = ((placed[0].2.x - placed[0].0.x).abs() * 0.14).clamp(5.0, 10.0);
    let last = placed.len() - 1;
    let start_width = placed[0].2.x - placed[0].0.x;
    paint_cap(painter, placed[0].1, start_width, cap_height);
    let mark = start_width.abs().max(8.0) * 0.28;
    painter.line_segment(
        [
            egui::pos2(placed[0].1.x - mark, placed[0].1.y),
            egui::pos2(placed[0].1.x + mark, placed[0].1.y),
        ],
        egui::Stroke::new(1.0, egui::Color32::WHITE),
    );
    paint_cap(
        painter,
        placed[last].1,
        placed[last].2.x - placed[last].0.x,
        cap_height,
    );
}

fn paint_rainbone(
    painter: &egui::Painter,
    slide: &Slide,
    color: &rainbone::LineColor,
    start_tick: i32,
    offset_sec: f64,
    ticks_per_second: f64,
    lane_x: &impl Fn(f32) -> f32,
    time_y: &impl Fn(f64) -> f32,
) {
    let points: Vec<egui::Pos2> = slide
        .points
        .iter()
        .map(|point| {
            let time =
                offset_sec + f64::from(point.tick.saturating_sub(start_tick)) / ticks_per_second;
            egui::pos2(
                lane_x(point.x as f32 + point.width as f32 * 0.5),
                time_y(time),
            )
        })
        .collect();
    // A transparent line is invisible in game, so show it faintly to keep it editable.
    let (fill, half) = match color.rgb {
        Some([r, g, b]) => (egui::Color32::from_rgb(r, g, b), egui::vec2(1.5, 0.0)),
        None => (egui::Color32::from_white_alpha(80), egui::vec2(0.6, 0.0)),
    };
    // Like the slide's center line, the width stays constant horizontally, so the line
    // gets thinner the more a segment leans sideways.
    for pair in points.windows(2) {
        let (mut top, mut bottom) = (pair[0], pair[1]);
        if top.y > bottom.y {
            std::mem::swap(&mut top, &mut bottom);
        }
        // egui feathers convex polygons correctly only when they wind clockwise.
        painter.add(egui::Shape::convex_polygon(
            vec![top - half, top + half, bottom + half, bottom - half],
            fill,
            egui::Stroke::NONE,
        ));
    }
}

fn add_gradient_quad(
    mesh: &mut egui::epaint::Mesh,
    a: egui::Pos2,
    b: egui::Pos2,
    c: egui::Pos2,
    d: egui::Pos2,
    color_a: egui::Color32,
    color_b: egui::Color32,
    color_c: egui::Color32,
    color_d: egui::Color32,
) {
    let index = mesh.vertices.len() as u32;
    for (pos, color) in [(a, color_a), (b, color_b), (d, color_d), (c, color_c)] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv: egui::epaint::WHITE_UV,
            color,
        });
    }
    mesh.add_triangle(index, index + 1, index + 2);
    mesh.add_triangle(index, index + 2, index + 3);
}

fn edge_color(t: f32) -> egui::Color32 {
    slide_gradient(t).gamma_multiply(0.82)
}

fn center_color(t: f32) -> egui::Color32 {
    lerp_color(
        slide_gradient(t),
        egui::Color32::from_rgb(70, 235, 220),
        0.45,
    )
}

fn slide_gradient(t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let bottom = egui::Color32::from_rgb(176, 62, 196);
    let mid = egui::Color32::from_rgb(24, 196, 188);
    if t < 0.5 {
        lerp_color(bottom, mid, t * 2.0)
    } else {
        lerp_color(mid, bottom, (t - 0.5) * 2.0)
    }
}

fn lerp_color(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from_rgb(
        lerp_u8(from.r(), to.r(), t),
        lerp_u8(from.g(), to.g(), t),
        lerp_u8(from.b(), to.b(), t),
    )
}

fn lerp_u8(from: u8, to: u8, t: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * t).round() as u8
}

fn paint_cap(painter: &egui::Painter, center: egui::Pos2, width: f32, height: f32) {
    let rect = egui::Rect::from_center_size(center, egui::vec2(width.abs().max(8.0), height));
    let radius = (height * 0.5).round() as u8;
    painter.rect_filled(rect, radius, egui::Color32::from_rgb(46, 96, 255));
}

/// CJK system fonts, loaded once so switching languages only reorders the fallback chain.
#[derive(Default)]
struct UiFonts {
    ja: Option<Arc<egui::FontData>>,
    zh_tw: Option<Arc<egui::FontData>>,
    ko: Option<Arc<egui::FontData>>,
}

impl UiFonts {
    fn load() -> Self {
        Self {
            ja: load_font(&[
                r"C:\Windows\Fonts\YuGothM.ttc",
                r"C:\Windows\Fonts\yugothm.ttc",
                r"C:\Windows\Fonts\meiryo.ttc",
                r"C:\Windows\Fonts\msgothic.ttc",
            ]),
            zh_tw: load_font(&[
                r"C:\Windows\Fonts\msjh.ttc",
                r"C:\Windows\Fonts\mingliu.ttc",
            ]),
            ko: load_font(&[
                r"C:\Windows\Fonts\malgun.ttf",
                r"C:\Windows\Fonts\gulim.ttc",
            ]),
        }
    }

    fn apply(&self, ctx: &egui::Context, lang: Lang) {
        let ja = ("ja", &self.ja);
        let zh_tw = ("zh_tw", &self.zh_tw);
        let ko = ("ko", &self.ko);
        // Han characters take the selected language's glyph forms; the other fonts only fill
        // gaps such as the language names in the selector.
        let order = match lang {
            Lang::Ja | Lang::En => [ja, zh_tw, ko],
            Lang::ZhTw => [zh_tw, ja, ko],
            Lang::Ko => [ko, ja, zh_tw],
        };
        let mut fonts = egui::FontDefinitions::default();
        let mut position = 0;
        for (name, data) in order {
            let Some(data) = data else {
                continue;
            };
            fonts.font_data.insert(name.to_owned(), Arc::clone(data));
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                if let Some(family) = fonts.families.get_mut(&family) {
                    family.insert(position, name.to_owned());
                }
            }
            position += 1;
        }
        if position > 0 {
            ctx.set_fonts(fonts);
        }
    }
}

fn load_font(paths: &[&str]) -> Option<Arc<egui::FontData>> {
    let bytes = paths.iter().find_map(|path| std::fs::read(path).ok())?;
    let y_offset_factor = line_gap_center_offset(&bytes);
    Some(Arc::new(egui::FontData::from_owned(bytes).tweak(
        egui::FontTweak {
            y_offset_factor,
            ..Default::default()
        },
    )))
}

fn line_gap_center_offset(font: &[u8]) -> f32 {
    let face = font_face_offset(font).unwrap_or(0);
    let Some((head, hhea, os2)) = face_metric_tables(font, face) else {
        return 0.0;
    };
    let Some(upem) = read_u16(font, head.saturating_add(18)) else {
        return 0.0;
    };
    if upem == 0 {
        return 0.0;
    }
    let use_typo = os2
        .and_then(|offset| read_u16(font, offset.saturating_add(62)))
        .is_some_and(|flags| flags & 0x80 != 0);
    let line_gap = if use_typo {
        os2.and_then(|offset| read_i16(font, offset.saturating_add(72)))
    } else {
        read_i16(font, hhea.saturating_add(8))
    };
    f32::from(line_gap.unwrap_or(0).max(0)) / (2.0 * f32::from(upem))
}

fn font_face_offset(font: &[u8]) -> Option<usize> {
    if font.len() >= 12 && &font[0..4] == b"ttcf" {
        let count = read_u32(font, 8)?;
        if count == 0 {
            return None;
        }
        return Some(read_u32(font, 12)? as usize);
    }
    Some(0)
}

fn face_metric_tables(font: &[u8], face: usize) -> Option<(usize, usize, Option<usize>)> {
    let count = read_u16(font, face.saturating_add(4))? as usize;
    let mut head = None;
    let mut hhea = None;
    let mut os2 = None;
    for index in 0..count {
        let entry = face
            .saturating_add(12)
            .saturating_add(index.saturating_mul(16));
        let tag = font.get(entry..entry.saturating_add(4))?;
        let offset = read_u32(font, entry.saturating_add(8))? as usize;
        match tag {
            b"head" => head = Some(offset),
            b"hhea" => hhea = Some(offset),
            b"OS/2" => os2 = Some(offset),
            _ => {}
        }
    }
    Some((head?, hhea?, os2))
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let chunk = bytes.get(offset..offset.saturating_add(2))?;
    Some(u16::from_be_bytes([chunk[0], chunk[1]]))
}

fn read_i16(bytes: &[u8], offset: usize) -> Option<i16> {
    read_u16(bytes, offset).map(|value| value as i16)
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let chunk = bytes.get(offset..offset.saturating_add(4))?;
    Some(u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}

#[cfg(test)]
mod tests {
    use super::{
        Sound2SlideApp, consume_playback_shortcut, event_walking_back, preview_beats,
        preview_click_position,
    };
    use crate::{audio::Audio, generate::SilentZone, i18n::Msg, timing::ChartPosition};
    use eframe::egui;
    use std::sync::{Arc, Mutex};

    #[test]
    fn field_and_output_changes_replace_warnings_without_changing_generated_notes() {
        let mut app = zone_test_app();
        app.select_field_mode(super::FieldMode::Extended);
        app.refresh();
        let original = app.generated.clone();
        let audit = app.audit.as_mut().unwrap();
        audit.advance(&app.generated.slides, app.lookup_bpm);
        assert!(audit.count > 0);
        app.refresh();
        assert!(app.audit.as_ref().unwrap().count > 0);
        app.rainbone = true;
        app.refresh();
        assert_eq!(app.generated, original);
        assert_eq!(app.audit.as_ref().unwrap().count, 0);
        assert!(app.audit.as_ref().unwrap().complete);
        app.rainbone = false;
        app.refresh();
        assert!(app.audit.as_ref().unwrap().count > 0);
        app.settings.max_width = 16;
        app.refresh();
        assert_eq!(app.audit.as_ref().unwrap().count, 0);
        assert!(app.audit.as_ref().unwrap().complete);
        app.select_field_mode(super::FieldMode::Normal);
        app.refresh();
        assert!(app.audit.is_none());
    }

    fn saved_chart(extended: bool, offset: f64) -> Vec<u8> {
        let mut metadata = b"xfld\x01\x00".to_vec();
        metadata.extend_from_slice(&u16::from(extended).to_le_bytes());
        metadata.extend_from_slice(b"wvof\x03\x00\x00\x00");
        metadata.extend_from_slice(&offset.to_le_bytes());
        let mut bytes = b"MGXC".to_vec();
        bytes.extend_from_slice(&(12 + metadata.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(b"meta");
        bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
        bytes.extend(metadata);
        bytes
    }

    #[test]
    fn audio_loading_detects_a_single_chart_without_timing_scan_and_clears_old_sources() {
        let directory = std::env::temp_dir().join(format!(
            "sound2slide-field-choice-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("extended.mgxc");
        std::fs::write(&path, saved_chart(true, 1.25)).unwrap();
        let audio = || Audio {
            samples: vec![0.1; 10].into(),
            sample_rate: 10,
        };
        let mut app = zone_test_app();
        app.accept_audio(&directory.join("song.wav"), audio());
        assert!(app.position_scan.is_none());
        assert_eq!(app.selected_chart.as_ref(), Some(&path));
        assert!(app.settings.extended_field);
        assert_eq!(app.settings.offset_sec, 0.0);
        std::fs::create_dir(directory.join("other")).unwrap();
        app.accept_audio(&directory.join("other/song.wav"), audio());
        assert!(app.selected_chart.is_none());
        assert_eq!(app.detected_extended_field(), None);
        assert!(!app.settings.extended_field);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn hidden_center_line_applies_only_to_slides_and_preserves_geometry() {
        use crate::rainbone::Output;
        let mut app = zone_test_app();
        assert_eq!(app.output(), Output::Slide);
        let settings = app.generation_settings();
        app.slide_no_center_line = true;
        assert_eq!(app.output(), Output::SlideNoCenterLine);
        assert_eq!(app.generation_settings(), settings);
        app.rainbone = true;
        assert!(matches!(app.output(), Output::Rainbone { .. }));
        app.rainbone = false;
        assert_eq!(app.output(), Output::SlideNoCenterLine);
    }

    #[test]
    fn rainbone_axis_selection_preserves_waveform_geometry_and_slide_output() {
        use crate::rainbone::{Axis, Output};
        let mut app = zone_test_app();
        assert_eq!(app.rainbone_axis, Axis::X);
        app.rainbone = true;
        app.refresh();
        let original = app.generated.clone();
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            app.rainbone_axis = axis;
            app.refresh();
            assert_eq!(app.generated, original);
            assert_eq!(
                app.output(),
                Output::Rainbone {
                    axis,
                    color: 0,
                    height: 80
                }
            );
        }
        app.rainbone = false;
        assert_eq!(app.output(), Output::Slide);
    }

    #[test]
    fn detected_fields_select_a_concrete_mode_and_manual_choices_are_preserved() {
        use super::FieldMode;
        let mut app = zone_test_app();
        assert_eq!(app.field_mode, FieldMode::Normal);
        app.selected_chart_extended = Some(true);
        app.select_detected_field();
        app.refresh();
        assert_eq!(app.field_mode, FieldMode::Extended);
        assert!(app.settings.extended_field);
        assert_eq!(app.settings.max_width, 48);
        app.selected_chart_extended = Some(false);
        app.select_detected_field();
        assert_eq!(app.field_mode, FieldMode::Normal);
        assert_eq!(app.settings.max_width, 16);
        app.host_extended_field = true;
        app.select_detected_field();
        assert_eq!(app.field_mode, FieldMode::Extended);
        app.select_field_mode(FieldMode::Normal);
        app.select_detected_field();
        app.refresh();
        assert_eq!(app.field_mode, FieldMode::Normal);
        assert!(!app.settings.extended_field);
        assert_eq!(app.settings.max_width, 16);
        app.select_chart(None);
        assert_eq!(app.field_mode, FieldMode::Normal);
        app.host_extended_field = false;
        app.select_field_mode(FieldMode::Extended);
        app.select_chart(None);
        app.refresh();
        assert_eq!(app.field_mode, FieldMode::Extended);
        assert!(app.settings.extended_field);
    }

    #[test]
    fn choosing_the_current_field_also_prevents_later_detection_from_overriding_it() {
        use super::FieldMode;
        let mut app = zone_test_app();
        app.select_field_mode(FieldMode::Normal);
        app.host_extended_field = true;
        app.select_detected_field();
        assert_eq!(app.field_mode, FieldMode::Normal);
        assert!(!app.settings.extended_field);
    }

    #[test]
    fn extended_preview_keeps_the_normal_field_in_the_middle_third() {
        use super::lane_fraction;
        assert_eq!(lane_fraction(-16.0, true), 0.0);
        assert_eq!(lane_fraction(0.0, true), 1.0 / 3.0);
        assert_eq!(lane_fraction(8.0, true), 0.5);
        assert_eq!(lane_fraction(16.0, true), 2.0 / 3.0);
        assert_eq!(lane_fraction(32.0, true), 1.0);
        assert_eq!(lane_fraction(0.0, false), 0.0);
        assert_eq!(lane_fraction(8.0, false), 0.5);
        assert_eq!(lane_fraction(16.0, false), 1.0);
    }

    #[test]
    fn fine_reproduction_applies_only_to_rainbones() {
        let mut app = zone_test_app();
        app.editing_silent_zones = false;
        app.settings.symmetric_width = true;
        app.settings.width = 6;
        app.rainbone_fine = true;
        assert_eq!(app.generation_settings(), app.settings);
        app.rainbone = true;
        app.refresh();
        let settings = app.generation_settings();
        assert!(settings.fine_center && !settings.symmetric_width);
        assert!(
            app.generated
                .slides
                .iter()
                .flat_map(|slide| &slide.points)
                .all(|point| matches!(point.width, 1 | 2))
        );
        assert_eq!(
            (app.settings.width, app.settings.symmetric_width),
            (6, true)
        );
    }

    fn zone_test_app() -> Sound2SlideApp<'static> {
        let mut app =
            Sound2SlideApp::new(0, Arc::new(Mutex::new(None)), &|_| Ok(None), &|_| Ok(None));
        app.accept_audio(
            std::path::Path::new("test.wav"),
            Audio {
                samples: vec![1.0; 1000].into(),
                sample_rate: 100,
            },
        );
        app.editing_silent_zones = true;
        app.settings.center_ends = false;
        app
    }

    fn zone_frame(
        app: &mut Sound2SlideApp<'_>,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::Rect {
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 600.0),
            )),
            events,
            ..Default::default()
        });
        let mut rect = egui::Rect::NOTHING;
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("test_root"),
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 600.0),
            )),
        );
        egui::CentralPanel::default().show(&mut root, |ui| {
            let (response, _) =
                ui.allocate_painter(egui::vec2(300.0, 500.0), egui::Sense::click_and_drag());
            rect = response.rect;
            app.silent_zone_controls(ui, &response);
        });
        ctx.end_pass().textures_delta.clear();
        rect
    }

    fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn zone_drag_test(
        app: &mut Sound2SlideApp<'_>,
        ctx: &egui::Context,
        from: egui::Pos2,
        to: egui::Pos2,
    ) {
        zone_frame(app, ctx, vec![egui::Event::PointerMoved(from)]);
        zone_frame(app, ctx, vec![pointer_button(from, true)]);
        zone_frame(app, ctx, vec![egui::Event::PointerMoved(to)]);
        zone_frame(app, ctx, vec![pointer_button(to, false)]);
        zone_frame(app, ctx, vec![]);
    }

    #[test]
    fn zones_can_be_drawn_both_ways_resized_and_deleted() {
        let ctx = egui::Context::default();
        let mut app = zone_test_app();
        let inner = zone_frame(&mut app, &ctx, vec![]).shrink(16.0);
        let position = |elapsed: f64| {
            egui::pos2(
                inner.center().x,
                inner.bottom() - (elapsed / 10.0) as f32 * inner.height(),
            )
        };
        zone_drag_test(&mut app, &ctx, position(2.0), position(5.0));
        assert_eq!(app.silent_zones.len(), 1);
        assert!((app.silent_zones[0].start_sec - 2.0).abs() < 1.0e-5);
        assert!((app.silent_zones[0].end_sec - 5.0).abs() < 1.0e-5);
        zone_drag_test(&mut app, &ctx, position(5.0), position(6.0));
        assert_eq!(app.silent_zones.len(), 1);
        assert!((app.silent_zones[0].end_sec - 6.0).abs() < 1.0e-5);
        zone_drag_test(&mut app, &ctx, position(2.0), position(-2.0));
        assert_eq!(app.silent_zones[0].start_sec, 0.0);
        zone_drag_test(&mut app, &ctx, position(9.0), position(8.0));
        assert_eq!(app.silent_zones.len(), 2);
        assert!((app.silent_zones[1].start_sec - 8.0).abs() < 1.0e-5);
        let delete = egui::pos2(inner.right() - 11.0, position(6.0).y + 17.0);
        zone_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(delete)]);
        zone_frame(&mut app, &ctx, vec![pointer_button(delete, true)]);
        zone_frame(&mut app, &ctx, vec![pointer_button(delete, false)]);
        assert_eq!(app.silent_zones.len(), 1);
        assert!((app.silent_zones[0].end_sec - 9.0).abs() < 1.0e-5);
        assert!(app.playback.is_none());
    }

    #[test]
    fn mute_button_stays_at_top_right_and_toggling_keeps_preview_height() {
        let mut app = zone_test_app();
        let ctx = egui::Context::default();
        let frame = |app: &mut Sound2SlideApp<'_>, events| {
            ctx.begin_pass(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(260.0, 600.0),
                )),
                events,
                ..Default::default()
            });
            let mut root = egui::Ui::new(
                ctx.clone(),
                egui::Id::new("header_test"),
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(260.0, 600.0),
                )),
            );
            let mut bounds = (egui::Rect::NOTHING, 0.0);
            egui::CentralPanel::default().show(&mut root, |ui| {
                let right = ui.max_rect().right();
                let top = ui.cursor().top();
                let response = app.preview_header(ui);
                assert!((response.rect.right() - right).abs() < 1.0);
                assert!((response.rect.top() - top).abs() < 1.0);
                bounds = (response.rect, ui.available_height());
            });
            ctx.end_pass().textures_delta.clear();
            bounds
        };
        app.editing_silent_zones = false;
        let (button, height) = frame(&mut app, vec![]);
        let position = button.center();
        frame(&mut app, vec![egui::Event::PointerMoved(position)]);
        frame(&mut app, vec![pointer_button(position, true)]);
        frame(&mut app, vec![pointer_button(position, false)]);
        assert!(app.editing_silent_zones);
        let (_, enabled_height) = frame(&mut app, vec![]);
        assert_eq!(enabled_height, height);
        assert!(height > 520.0);
        frame(&mut app, vec![pointer_button(position, true)]);
        frame(&mut app, vec![pointer_button(position, false)]);
        assert!(!app.editing_silent_zones);
    }

    #[test]
    fn dragging_a_boundary_past_the_other_swaps_roles_and_continues_the_drag() {
        let ctx = egui::Context::default();
        let mut app = zone_test_app();
        app.silent_zones.push(SilentZone {
            start_sec: 2.0,
            end_sec: 5.0,
        });
        let inner = zone_frame(&mut app, &ctx, vec![]).shrink(16.0);
        let position = |elapsed: f64| {
            egui::pos2(
                inner.center().x,
                inner.bottom() - (elapsed / 10.0) as f32 * inner.height(),
            )
        };
        zone_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(position(5.0))],
        );
        zone_frame(&mut app, &ctx, vec![pointer_button(position(5.0), true)]);
        zone_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(position(1.0))],
        );
        zone_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(position(0.5))],
        );
        zone_frame(&mut app, &ctx, vec![pointer_button(position(0.5), false)]);
        assert_eq!(app.silent_zones.len(), 1);
        assert!((app.silent_zones[0].start_sec - 0.5).abs() < 1.0e-5);
        assert_eq!(app.silent_zones[0].end_sec, 2.0);
        zone_drag_test(&mut app, &ctx, position(0.5), position(7.0));
        assert_eq!(app.silent_zones[0].start_sec, 2.0);
        assert!((app.silent_zones[0].end_sec - 7.0).abs() < 1.0e-5);
        assert!(app.zone_drag.is_none());
    }

    #[test]
    fn zone_seconds_can_be_edited_without_creating_another_zone() {
        let ctx = egui::Context::default();
        let mut app = zone_test_app();
        app.silent_zones.push(SilentZone {
            start_sec: 2.0,
            end_sec: 5.0,
        });
        let inner = zone_frame(&mut app, &ctx, vec![]).shrink(16.0);
        let value = egui::pos2(
            inner.left() + 35.0,
            inner.bottom() - 0.2 * inner.height() - 17.0,
        );
        zone_drag_test(&mut app, &ctx, value, value + egui::vec2(30.0, 0.0));
        assert_eq!(app.silent_zones.len(), 1);
        assert!(app.silent_zones[0].start_sec > 2.0);
        assert_eq!(app.silent_zones[0].end_sec, 5.0);
        assert!(app.zone_drag.is_none());
    }

    #[test]
    fn zone_seconds_accept_keyboard_input() {
        let ctx = egui::Context::default();
        let mut app = zone_test_app();
        app.silent_zones.push(SilentZone {
            start_sec: 2.0,
            end_sec: 5.0,
        });
        let inner = zone_frame(&mut app, &ctx, vec![]).shrink(16.0);
        let value = egui::pos2(
            inner.left() + 35.0,
            inner.bottom() - 0.2 * inner.height() - 17.0,
        );
        zone_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(value)]);
        for _ in 0..2 {
            zone_frame(&mut app, &ctx, vec![pointer_button(value, true)]);
            zone_frame(&mut app, &ctx, vec![pointer_button(value, false)]);
        }
        zone_frame(&mut app, &ctx, vec![egui::Event::Text("1.25".into())]);
        zone_frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: Some(egui::Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(app.silent_zones.len(), 1);
        assert_eq!(app.silent_zones[0].start_sec, 1.25);
    }

    #[test]
    fn shrinking_the_import_range_clips_zone_ends_and_removes_outside_starts() {
        let mut app = zone_test_app();
        app.silent_zones = vec![
            SilentZone {
                start_sec: 1.0,
                end_sec: 9.0,
            },
            SilentZone {
                start_sec: 4.8,
                end_sec: 8.0,
            },
            SilentZone {
                start_sec: 5.0,
                end_sec: 7.0,
            },
            SilentZone {
                start_sec: 8.0,
                end_sec: 9.0,
            },
        ];
        app.zone_drag = Some(super::ZoneDrag::Resize {
            index: 3,
            end: true,
        });
        app.settings.length_sec = 5.0;
        app.refresh();
        assert_eq!(
            app.silent_zones,
            vec![
                SilentZone {
                    start_sec: 1.0,
                    end_sec: 5.0
                },
                SilentZone {
                    start_sec: 4.8,
                    end_sec: 5.0
                },
            ]
        );
        assert!(app.zone_drag.is_none());
        app.settings.length_sec = 2.0;
        app.refresh();
        assert_eq!(
            app.silent_zones,
            vec![SilentZone {
                start_sec: 1.0,
                end_sec: 2.0
            }]
        );
        app.settings.length_sec = 10.0;
        app.refresh();
        assert_eq!(app.silent_zones[0].end_sec, 2.0);
        app.settings.offset_sec = 9.0;
        app.refresh();
        assert!(app.silent_zones.is_empty());
    }

    #[test]
    fn zone_edits_invalidate_generation_and_loading_audio_clears_zones() {
        let mut app = zone_test_app();
        app.refresh();
        let original = app.generated.clone();
        app.silent_zones.push(SilentZone {
            start_sec: 0.0,
            end_sec: 10.0,
        });
        app.refresh();
        assert_ne!(app.generated, original);
        app.silent_zones.clear();
        app.refresh();
        assert_eq!(app.generated, original);
        app.silent_zones.push(SilentZone {
            start_sec: 0.0,
            end_sec: 10.0,
        });
        app.accept_audio(
            std::path::Path::new("other.wav"),
            Audio {
                samples: vec![0.2; 100].into(),
                sample_rate: 100,
            },
        );
        assert!(app.silent_zones.is_empty());
        assert!(app.zone_drag.is_none());
    }

    #[test]
    fn preview_click_uses_the_drawn_timeline_and_preserves_the_range() {
        let audio = Audio {
            samples: vec![0.1; 1000].into(),
            sample_rate: 100,
        };
        let settings = crate::generate::Settings {
            offset_sec: -1.0,
            length_sec: 4.0,
            ..Default::default()
        };
        let rect = eframe::egui::Rect::from_min_max(
            eframe::egui::pos2(10.0, 20.0),
            eframe::egui::pos2(242.0, 452.0),
        );
        let inner = rect.shrink(16.0);
        let click = |y| {
            preview_click_position(
                rect,
                eframe::egui::pos2(inner.center().x, y),
                &audio,
                &settings,
            )
        };
        assert_eq!(click(inner.bottom()), Some(-1.0));
        assert_eq!(click(inner.center().y), Some(1.0));
        assert_eq!(click(inner.top()), Some(2.99));
        assert_eq!(click(rect.top()), None);
        assert_eq!(settings.offset_sec, -1.0);
        assert_eq!(settings.length_sec, 4.0);
    }

    #[test]
    fn calculated_position_sets_the_import_range_and_current_timing() {
        let lookup_bpm = |_| Ok(None);
        let lookup_signature = |_| Ok(None);
        let mut app = Sound2SlideApp::new(
            0,
            Arc::new(Mutex::new(None)),
            &lookup_bpm,
            &lookup_signature,
        );
        app.accept_audio(
            std::path::Path::new("song.wav"),
            Audio {
                samples: vec![0.1; 120].into(),
                sample_rate: 1,
            },
        );
        assert_eq!(app.settings.length_sec, 3.0);
        app.apply_chart_position(ChartPosition {
            seconds: 75.5,
            bpm: 150.0,
            signature: [3, 4],
        });
        assert_eq!(app.settings.offset_sec, 75.5);
        assert_eq!(app.settings.length_sec, 3.0);
        assert_eq!(app.settings.bpm, 150.0);
        assert_eq!(app.settings.time_signature, [3, 4]);
        app.apply_chart_position(ChartPosition {
            seconds: 121.0,
            bpm: 180.0,
            signature: [7, 8],
        });
        assert_eq!(app.settings.offset_sec, 75.5);
        assert_eq!(app.settings.length_sec, 3.0);
        assert_eq!(app.settings.bpm, 150.0);
        app.accept_audio(
            std::path::Path::new("short.wav"),
            Audio {
                samples: vec![0.1; 10].into(),
                sample_rate: 1,
            },
        );
        assert_eq!(app.settings.offset_sec, 0.0);
        assert_eq!(app.settings.length_sec, 10.0);
    }

    #[test]
    fn offset_dialog_applies_the_selected_chart_or_skips_correction() {
        let directory = std::env::temp_dir().join(format!(
            "sound2slide-offset-choice-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let first = directory.join("a.mgxc");
        let second = directory.join("b.mgxc");
        std::fs::write(&first, b"MGCF0\nBGMOFFSET\t1.0\n").unwrap();
        std::fs::write(&second, saved_chart(true, -1.55)).unwrap();
        let lookup_bpm = |_| Ok(None);
        let lookup_signature = |_| Ok(None);
        let mut app = Sound2SlideApp::new(
            0,
            Arc::new(Mutex::new(None)),
            &lookup_bpm,
            &lookup_signature,
        );
        app.accept_audio(
            &directory.join("song.mp3"),
            Audio {
                samples: vec![0.1; 120].into(),
                sample_rate: 1,
            },
        );
        let position = ChartPosition {
            seconds: 10.0,
            bpm: 120.0,
            signature: [4, 4],
        };
        app.resolve_chart_offset(position, vec![first.clone(), second.clone()]);
        assert!(app.offset_selection.is_some());
        assert!(!app.can_play());
        assert_eq!(app.settings.offset_sec, 0.0);
        app.offset_selection.as_mut().unwrap().selected = 1;
        app.finish_offset_selection(true);
        assert_eq!(app.settings.offset_sec, 11.55);
        assert_eq!(app.detected_extended_field(), Some(true));
        assert!(app.settings.extended_field);
        assert!(app.offset_selection.is_none());
        assert!(app.can_play());
        app.resolve_chart_offset(position, vec![first, second]);
        app.finish_offset_selection(false);
        assert_eq!(app.settings.offset_sec, 10.0);
        assert_eq!(app.settings.length_sec, 3.0);
        assert!(app.offset_selection.is_none());
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn chart_offset_moves_the_import_start_in_both_directions() {
        let lookup_bpm = |_| Ok(None);
        let lookup_signature = |_| Ok(None);
        let mut app = Sound2SlideApp::new(
            0,
            Arc::new(Mutex::new(None)),
            &lookup_bpm,
            &lookup_signature,
        );
        app.accept_audio(
            std::path::Path::new("song.wav"),
            Audio {
                samples: vec![0.1; 120].into(),
                sample_rate: 1,
            },
        );
        for (seconds, offset, expected) in
            [(10.0, -1.55, 11.55), (10.0, 1.55, 8.45), (0.0, 1.55, -1.55)]
        {
            app.apply_chart_position(ChartPosition {
                seconds: seconds - offset,
                bpm: 120.0,
                signature: [4, 4],
            });
            assert_eq!(app.settings.offset_sec, expected);
            assert_eq!(app.settings.length_sec, 3.0);
        }
    }

    #[test]
    fn long_audio_defaults_to_three_seconds_and_clips_near_the_end() {
        let lookup_bpm = |_| Ok(None);
        let lookup_signature = |_| Ok(None);
        let mut app = Sound2SlideApp::new(
            0,
            Arc::new(Mutex::new(None)),
            &lookup_bpm,
            &lookup_signature,
        );
        for (duration, expected) in [(59, 59.0), (60, 3.0), (120, 3.0)] {
            app.accept_audio(
                std::path::Path::new("song.wav"),
                Audio {
                    samples: vec![0.1; duration].into(),
                    sample_rate: 1,
                },
            );
            assert_eq!(app.settings.length_sec, expected);
        }
        app.apply_chart_position(ChartPosition {
            seconds: 119.0,
            bpm: 120.0,
            signature: [4, 4],
        });
        assert_eq!(app.settings.offset_sec, 119.0);
        assert_eq!(app.settings.length_sec, 1.0);
    }

    #[test]
    fn space_shortcut_consumes_key_events_and_ignores_repeats() {
        let ctx = eframe::egui::Context::default();
        let space = |pressed, repeat| eframe::egui::Event::Key {
            key: eframe::egui::Key::Space,
            physical_key: Some(eframe::egui::Key::Space),
            pressed,
            repeat,
            modifiers: eframe::egui::Modifiers::NONE,
        };
        ctx.begin_pass(eframe::egui::RawInput {
            events: vec![space(true, false)],
            ..Default::default()
        });
        assert!(consume_playback_shortcut(&ctx));
        assert!(!consume_playback_shortcut(&ctx));
        assert!(!ctx.input(|input| input.key_pressed(eframe::egui::Key::Space)));
        ctx.end_pass().textures_delta.clear();
        ctx.begin_pass(eframe::egui::RawInput {
            events: vec![space(true, true), space(false, false)],
            ..Default::default()
        });
        assert!(!consume_playback_shortcut(&ctx));
        assert!(ctx.input(|input| input.events.is_empty()));
        ctx.end_pass().textures_delta.clear();
    }

    #[test]
    fn chart_timing_finds_bpm_and_signature_at_independent_positions() {
        let lookup_bpm = |tick| Ok((tick == 4200).then_some(144.0));
        let lookup_signature = |bar| Ok((bar == 1).then_some([3, 8]));
        let mut app = Sound2SlideApp::new(
            5000,
            Arc::new(Mutex::new(None)),
            &lookup_bpm,
            &lookup_signature,
        );
        app.take_chart_timing();
        assert_eq!(app.settings.bpm, 144.0);
        assert_eq!(app.settings.time_signature, [3, 8]);
    }

    #[test]
    fn missing_signature_preserves_input_while_setting_bpm() {
        let lookup_bpm = |_| Ok(Some(180.0));
        let lookup_signature = |_| Ok(None);
        let mut app = Sound2SlideApp::new(
            0,
            Arc::new(Mutex::new(None)),
            &lookup_bpm,
            &lookup_signature,
        );
        app.settings.time_signature = [7, 8];
        app.take_chart_timing();
        assert_eq!(app.settings.bpm, 180.0);
        assert_eq!(app.settings.time_signature, [7, 8]);
    }

    #[test]
    fn bpm_error_does_not_block_signature_lookup() {
        let lookup_bpm = |_| Err("fail".into());
        let lookup_signature = |_| Ok(Some([3, 4]));
        let mut app = Sound2SlideApp::new(
            0,
            Arc::new(Mutex::new(None)),
            &lookup_bpm,
            &lookup_signature,
        );
        app.take_chart_timing();
        assert_eq!(app.settings.bpm, 120.0);
        assert_eq!(app.settings.time_signature, [3, 4]);
    }

    #[test]
    fn grid_aligns_to_chart_beats_from_a_partial_beat() {
        assert_eq!(
            preview_beats(240, 2.0, 120.0, [4, 4], 600.0),
            vec![(0.25, false), (0.75, false), (1.25, false), (1.75, true)],
        );
    }

    #[test]
    fn grid_uses_bpm_and_beats_per_bar() {
        assert_eq!(
            preview_beats(0, 3.0, 60.0, [3, 4], 600.0),
            vec![(0.0, true), (1.0, false), (2.0, false), (3.0, true)],
        );
        assert_eq!(
            preview_beats(1440, 1.0, 120.0, [4, 4], 600.0),
            vec![(0.0, false), (0.5, true), (1.0, false)],
        );
    }

    #[test]
    fn grid_uses_eighth_note_beats() {
        assert_eq!(
            preview_beats(240, 0.75, 120.0, [3, 8], 600.0),
            vec![(0.15625, false), (0.40625, false), (0.65625, true)],
        );
    }

    #[test]
    fn grid_supports_time_signature_limits() {
        assert_eq!(
            preview_beats(0, 4.0, 60.0, [1, 1], 600.0),
            vec![(0.0, true), (4.0, true)],
        );
        let lines = preview_beats(1920, 0.05, 120.0, [16, 480], 600.0);
        assert_eq!(lines[0], (0.0, true));
        assert_eq!(lines.len(), 13);
        assert!(lines[1..].iter().all(|(_, is_bar)| !is_bar));
        assert!((lines[1].0 - 1.0 / 240.0).abs() < 1.0e-10);
    }

    #[test]
    fn dense_grid_keeps_bar_alignment_and_limits_line_count() {
        let lines = preview_beats(240, 3600.0, 512.0, [3, 4], 600.0);
        assert!(lines.len() <= 201);
        assert!(!lines.is_empty());
        for (seconds, is_bar) in lines {
            assert!((0.0..=3600.0).contains(&seconds));
            assert!(is_bar);
            let beat = 0.375 + seconds * 512.0 / 60.0;
            assert!((beat / 3.0 - (beat / 3.0).round()).abs() < 1.0e-8);
        }
    }

    #[test]
    fn grid_rejects_invalid_time_ranges() {
        assert!(preview_beats(0, 0.0, 120.0, [4, 4], 600.0).is_empty());
        assert!(preview_beats(0, 1.0, f64::NAN, [4, 4], 600.0).is_empty());
        assert!(preview_beats(0, f64::INFINITY, 120.0, [4, 4], 600.0).is_empty());
    }

    #[test]
    fn uses_bpm_on_the_start_tick() {
        let mut seen = Vec::new();
        let found = event_walking_back(10, |tick| {
            seen.push(tick);
            Ok(Some(144.0))
        })
        .unwrap();
        assert_eq!(found, Some(144.0));
        assert_eq!(seen, vec![10]);
    }

    #[test]
    fn walks_back_to_the_nearest_event() {
        let found = event_walking_back(5, |tick| if tick == 2 { Ok(Some(90.0)) } else { Ok(None) })
            .unwrap();
        assert_eq!(found, Some(90.0));
    }

    #[test]
    fn leaves_missing_when_no_event_down_to_zero() {
        let mut seen = Vec::new();
        let found = event_walking_back::<f64>(3, |tick| {
            seen.push(tick);
            Ok(None)
        })
        .unwrap();
        assert_eq!(found, None);
        assert_eq!(seen, vec![3, 2, 1, 0]);
    }

    #[test]
    fn checks_a_non_positive_start_once() {
        let mut seen = Vec::new();
        let found = event_walking_back::<f64>(-2, |tick| {
            seen.push(tick);
            Ok(None)
        })
        .unwrap();
        assert_eq!(found, None);
        assert_eq!(seen, vec![-2]);
    }

    #[test]
    fn stops_on_lookup_error_without_a_bpm() {
        let error = event_walking_back::<f64>(4, |tick| {
            if tick == 2 {
                Err("fail".into())
            } else {
                Ok(None)
            }
        })
        .unwrap_err();
        assert_eq!(error, Msg::from("fail"));
    }
}
