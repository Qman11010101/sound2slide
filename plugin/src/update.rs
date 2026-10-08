use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::{self, JoinHandle},
    time::Duration,
};

use eframe::egui;
use serde::{Deserialize, de::DeserializeOwned};
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

use crate::i18n::Lang;

const API: &str = "https://api.github.com/repos/Qman11010101/sound2slide";
const RELEASES_URL: &str = "https://github.com/Qman11010101/sound2slide/releases/latest";
const BUILD_SHA: &str = env!("SOUND2SLIDE_BUILD_SHA");
const ARCHIVE: &str = "sound2slide-windows-x64.zip";
const DOWNLOAD_PREFIX: &str = "https://github.com/Qman11010101/sound2slide/releases/download/";

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    state: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Release {
    tag_name: String,
    target_commitish: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

impl Release {
    fn archive_url(&self) -> Option<&str> {
        // Only link to this repository's release downloads, never to an arbitrary URL from the API.
        self.assets
            .iter()
            .find(|asset| {
                asset.name == ARCHIVE
                    && asset.state == "uploaded"
                    && asset.browser_download_url.starts_with(DOWNLOAD_PREFIX)
            })
            .map(|asset| asset.browser_download_url.as_str())
    }

    fn needs_comparison(&self, current_sha: &str) -> bool {
        !self.draft
            && !self.prerelease
            && valid_sha(current_sha)
            && valid_sha(&self.target_commitish)
            && !self.target_commitish.eq_ignore_ascii_case(current_sha)
            && self.archive_url().is_some()
    }
}

#[derive(Deserialize)]
struct Comparison {
    status: String,
}

fn valid_sha(sha: &str) -> bool {
    sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn get_json<T: DeserializeOwned>(url: &str) -> Result<T, ureq::Error> {
    // A global/DNS timeout detaches resolver threads that could outlive the DLL.
    let agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(5)))
        .timeout_send_request(Some(Duration::from_secs(5)))
        .timeout_recv_response(Some(Duration::from_secs(5)))
        .timeout_recv_body(Some(Duration::from_secs(5)))
        .https_only(true)
        .user_agent("sound2slide-update-check")
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .new_agent();
    agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()?
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_json()
}

fn check(current_sha: &str) -> Option<Release> {
    if !valid_sha(current_sha) {
        return None;
    }
    let release: Release = get_json(&format!("{API}/releases/latest")).ok()?;
    select_update(current_sha, release, |head| {
        get_json(&format!("{API}/compare/{current_sha}...{head}?per_page=1")).ok()
    })
}

fn select_update(
    current_sha: &str,
    release: Release,
    compare: impl FnOnce(&str) -> Option<Comparison>,
) -> Option<Release> {
    if !release.needs_comparison(current_sha) {
        return None;
    }
    let comparison = compare(&release.target_commitish)?;
    (comparison.status == "ahead").then_some(release)
}

pub(crate) struct UpdateCheck {
    receiver: Option<Receiver<Release>>,
    worker: Option<JoinHandle<()>>,
}

impl UpdateCheck {
    pub(crate) fn start() -> Option<Self> {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("sound2slide-update".into())
            .spawn(move || {
                if let Some(release) = check(BUILD_SHA) {
                    let _ = sender.send(release);
                }
            })
            .ok()?;
        Some(Self {
            receiver: Some(receiver),
            worker: Some(worker),
        })
    }

    pub(crate) fn poll(&mut self, ctx: &egui::Context) -> Option<Release> {
        let receiver = self.receiver.as_ref()?;
        match receiver.try_recv() {
            Ok(release) => {
                self.receiver = None;
                Some(release)
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
                None
            }
            Err(TryRecvError::Disconnected) => {
                self.receiver = None;
                None
            }
        }
    }
}

impl Drop for UpdateCheck {
    fn drop(&mut self) {
        // Margrete may unload the DLL after the command returns; its worker must finish first.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// eframe is built without egui-winit's `links` feature, so `Context::open_url` is a no-op.
fn open_in_browser(url: &str) {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

    let url: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    // SAFETY: the verb and URL are NUL-terminated UTF-16 strings that outlive the call.
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            windows_sys::w!("open"),
            url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

pub(crate) fn footer(ui: &mut egui::Ui, available: &Release, lang: Lang) {
    let Some(url) = available.archive_url() else {
        return;
    };
    ui.label(format!(
        "{} (sound2slide {})",
        lang.pick([
            "新しいバージョンがあります",
            "A new version is available",
            "有新版本可供下載",
            "새 버전이 있습니다",
        ]),
        available.tag_name
    ));
    if ui
        .link(lang.pick(["ダウンロード", "Download", "下載", "다운로드"]))
        .on_hover_text(url)
        .clicked()
    {
        open_in_browser(url);
    }
    ui.separator();
}

pub(crate) fn show(ctx: &egui::Context, available: &Release, open: &mut bool, lang: Lang) {
    if !*open {
        return;
    }
    let mut close = false;
    let response = egui::Modal::new(egui::Id::new("update_available")).show(ctx, |ui| {
        ui.set_max_width((ctx.content_rect().width() - 64.0).clamp(200.0, 420.0));
        ui.heading(lang.pick([
            "更新のお知らせ",
            "Update available",
            "更新通知",
            "업데이트 알림",
        ]));
        ui.add_space(8.0);
        ui.label(lang.pick([
            "sound2slideの新しいバージョンが公開されています。",
            "A new version of sound2slide is available.",
            "sound2slide 有新版本可供下載。",
            "sound2slide의 새 버전이 출시되었습니다.",
        ]));
        ui.label(format!("sound2slide {}", available.tag_name));
        ui.label(lang.pick([
            "更新する場合は、Margreteを終了してからファイルを差し替えてください。",
            "Close Margrete before replacing the plugin files to update.",
            "更新時，請先關閉 Margrete，再替換外掛檔案。",
            "업데이트하려면 Margrete를 종료한 후 플러그인 파일을 교체해 주세요.",
        ]));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .button(lang.pick([
                    "リリースページを開く",
                    "Open release page",
                    "開啟版本發布頁面",
                    "릴리스 페이지 열기",
                ]))
                .clicked()
            {
                open_in_browser(RELEASES_URL);
                close = true;
            }
            if ui
                .button(lang.pick(["閉じる", "Close", "關閉", "닫기"]))
                .clicked()
            {
                close = true;
            }
        });
    });
    if close || response.should_close() {
        *open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT: &str = "1111111111111111111111111111111111111111";
    const NEW: &str = "2222222222222222222222222222222222222222";

    fn release() -> Release {
        Release {
            tag_name: "2026-10-08-2".into(),
            target_commitish: NEW.into(),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                name: ARCHIVE.into(),
                state: "uploaded".into(),
                browser_download_url: format!("{DOWNLOAD_PREFIX}2026-10-08-2/{ARCHIVE}"),
            }],
        }
    }

    #[test]
    fn published_release_with_windows_archive_is_checked_even_on_same_release_date() {
        assert!(release().needs_comparison(CURRENT));
    }

    #[test]
    fn installed_release_does_not_need_comparison() {
        assert!(!release().needs_comparison(NEW));
    }

    #[test]
    fn drafts_and_prereleases_are_not_offered() {
        let mut candidate = release();
        candidate.draft = true;
        assert!(!candidate.needs_comparison(CURRENT));
        candidate.draft = false;
        candidate.prerelease = true;
        assert!(!candidate.needs_comparison(CURRENT));
    }

    #[test]
    fn missing_or_incomplete_archive_is_not_offered() {
        let mut candidate = release();
        candidate.assets[0].state = "open".into();
        assert!(!candidate.needs_comparison(CURRENT));
        candidate.assets[0].state = "uploaded".into();
        candidate.assets[0].name = "source.zip".into();
        assert!(!candidate.needs_comparison(CURRENT));
        candidate.assets.clear();
        assert!(!candidate.needs_comparison(CURRENT));
    }

    #[test]
    fn archive_link_points_only_to_this_repository_downloads() {
        let mut candidate = release();
        assert_eq!(
            candidate.archive_url(),
            Some(
                "https://github.com/Qman11010101/sound2slide/releases/download/2026-10-08-2/sound2slide-windows-x64.zip"
            )
        );
        candidate.assets[0].browser_download_url = format!("https://example.com/{ARCHIVE}");
        assert!(candidate.archive_url().is_none());
        assert!(!candidate.needs_comparison(CURRENT));
    }

    #[test]
    fn unknown_build_or_unresolved_branch_is_not_offered() {
        assert!(!release().needs_comparison(""));
        let mut candidate = release();
        candidate.target_commitish = "main".into();
        assert!(!candidate.needs_comparison(CURRENT));
        assert!(!valid_sha("../../main"));
    }

    #[test]
    fn https_request_uses_an_enabled_tls_provider() {
        // ureq panics instead of erroring when the selected TLS provider is not compiled in.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || drop(listener.accept()));
        let result = std::panic::catch_unwind(|| {
            get_json::<Comparison>(&format!("https://127.0.0.1:{port}/")).is_err()
        });
        server.join().unwrap();
        assert!(
            result.unwrap(),
            "a closed TLS handshake must fail without panicking"
        );
    }

    #[test]
    fn failed_check_finishes_without_a_notification() {
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        let mut check = UpdateCheck {
            receiver: Some(receiver),
            worker: None,
        };
        assert!(check.poll(&egui::Context::default()).is_none());
        assert!(check.receiver.is_none());
    }

    #[test]
    fn available_update_is_delivered_only_once_per_check() {
        let (sender, receiver) = mpsc::channel();
        sender.send(release()).unwrap();
        let mut check = UpdateCheck {
            receiver: Some(receiver),
            worker: None,
        };
        let ctx = egui::Context::default();
        assert_eq!(check.poll(&ctx).unwrap().tag_name, "2026-10-08-2");
        assert!(check.poll(&ctx).is_none());
    }

    #[test]
    fn only_a_release_ahead_of_the_installed_build_is_offered() {
        for status in ["ahead", "identical", "behind", "diverged", "unknown"] {
            let offered = select_update(CURRENT, release(), |head| {
                assert_eq!(head, NEW);
                Some(Comparison {
                    status: status.into(),
                })
            });
            assert_eq!(offered.is_some(), status == "ahead", "{status}");
        }
    }

    #[test]
    fn failed_comparison_does_not_offer_an_update() {
        assert!(select_update(CURRENT, release(), |_| None).is_none());
    }

    #[test]
    fn installed_release_does_not_make_a_comparison_request() {
        assert!(select_update(NEW, release(), |_| panic!("unnecessary request")).is_none());
    }

    #[test]
    fn escape_dismisses_the_notification_in_every_language() {
        for lang in Lang::ALL {
            let ctx = egui::Context::default();
            let available = release();
            let mut open = true;
            ctx.begin_pass(egui::RawInput::default());
            show(&ctx, &available, &mut open, lang);
            let _ = ctx.end_pass();
            assert!(open);
            ctx.begin_pass(egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: Some(egui::Key::Escape),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            });
            show(&ctx, &available, &mut open, lang);
            let output = ctx.end_pass();
            assert!(!open, "{lang:?}");
            assert!(output.platform_output.commands.is_empty());
        }
    }
}
