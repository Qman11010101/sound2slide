use eframe::egui;

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

pub(crate) fn show(ctx: &egui::Context, open: &mut bool) {
    egui::Window::new("オープンソースライブラリ")
        .id(egui::Id::new("open_source_libraries"))
        .open(open)
        .collapsible(false)
        .default_width(520.0)
        .hscroll(true)
        .vscroll(true)
        .show(ctx, |ui| {
            ui.label("sound2slide が直接利用している主要ライブラリです。");
            ui.label("OR は、いずれかのライセンスを選択できることを示します。");
            ui.separator();
            egui::Grid::new("open_source_libraries_table")
                .striped(true)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.strong("ライブラリ");
                    ui.strong("バージョン");
                    ui.strong("ライセンス");
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
