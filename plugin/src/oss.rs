use eframe::egui;

use crate::i18n::Lang;

struct Library {
    name: &'static str,
    version: &'static str,
    license: &'static str,
}

const LIBRARIES: &[Library] = &[
    Library {
        name: "eframe",
        version: "0.36.2",
        license: "MIT OR Apache-2.0",
    },
    Library {
        name: "marmkmt",
        version: "0.1.0",
        license: "MIT",
    },
    Library {
        name: "raw-window-handle",
        version: "0.6.2",
        license: "MIT OR Apache-2.0 OR Zlib",
    },
    Library {
        name: "rfd",
        version: "0.15.4",
        license: "MIT",
    },
    Library {
        name: "rodio",
        version: "0.22.2",
        license: "MIT OR Apache-2.0",
    },
    Library {
        name: "symphonia",
        version: "0.5.5",
        license: "MPL-2.0",
    },
    Library {
        name: "windows-sys",
        version: "0.61.2",
        license: "MIT OR Apache-2.0",
    },
];

pub(crate) fn title(lang: Lang) -> &'static str {
    lang.pick([
        "オープンソースライブラリ",
        "Open source libraries",
        "開放原始碼程式庫",
        "오픈 소스 라이브러리",
    ])
}

pub(crate) fn show(ctx: &egui::Context, open: &mut bool, lang: Lang) {
    egui::Window::new(title(lang))
        .id(egui::Id::new("open_source_libraries"))
        .open(open)
        .collapsible(false)
        .default_width(520.0)
        .hscroll(true)
        .vscroll(true)
        .show(ctx, |ui| {
            ui.label(lang.pick([
                "sound2slide が直接利用している主要ライブラリです。",
                "The main libraries that sound2slide uses directly.",
                "sound2slide 直接使用的主要程式庫。",
                "sound2slide가 직접 사용하는 주요 라이브러리입니다.",
            ]));
            ui.label(lang.pick([
                "OR は、いずれかのライセンスを選択できることを示します。",
                "OR means that either license may be chosen.",
                "OR 表示可任選其中一種授權。",
                "OR은 어느 쪽 라이선스든 선택할 수 있음을 뜻합니다.",
            ]));
            ui.separator();
            egui::Grid::new("open_source_libraries_table")
                .striped(true)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.strong(lang.pick(["ライブラリ", "Library", "程式庫", "라이브러리"]));
                    ui.strong(lang.pick(["バージョン", "Version", "版本", "버전"]));
                    ui.strong(lang.pick(["ライセンス", "License", "授權", "라이선스"]));
                    ui.end_row();
                    for library in LIBRARIES {
                        ui.label(library.name);
                        ui.label(library.version);
                        ui.label(library.license);
                        ui.end_row();
                    }
                });
        });
}
