#![windows_subsystem = "windows"] // no console window on Windows

use eframe::egui::{self, RichText};
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

// Install-or-update yt-dlp (plus ffmpeg and a JS runtime) with each OS's usual package manager.
#[cfg(target_os = "macos")]
const UPDATE: &[&str] = &["brew", "install", "yt-dlp", "ffmpeg"];
#[cfg(windows)]
const UPDATE: &[&str] = &[
    "winget",
    "install",
    "--id",
    "yt-dlp.yt-dlp",
    "-e",
    "--accept-source-agreements",
    "--accept-package-agreements",
];
#[cfg(not(any(target_os = "macos", windows)))]
const UPDATE: &[&str] = &["pipx", "install", "--force", "yt-dlp[default]"];

const REVEAL: &str = if cfg!(target_os = "macos") {
    "Show in Finder"
} else if cfg!(windows) {
    "Show in Explorer"
} else {
    "Show in folder"
};

/// Ionicons (MIT, see assets/icons/LICENSE), recoloured white so a tint can colour them.
mod icons {
    use eframe::egui::{self, ImageSource, include_image};

    pub const AUDIO: ImageSource = include_image!("../assets/icons/musical-notes-outline.svg");
    pub const VIDEO: ImageSource = include_image!("../assets/icons/film-outline.svg");
    pub const SETTINGS: ImageSource = include_image!("../assets/icons/settings-outline.svg");
    pub const REFRESH: ImageSource = include_image!("../assets/icons/refresh-outline.svg");
    pub const EMPTY: ImageSource = include_image!("../assets/icons/file-tray-outline.svg");
    pub const FOLDER: ImageSource = include_image!("../assets/icons/folder-open-outline.svg");

    pub fn sized(source: ImageSource<'static>, size: f32) -> egui::Image<'static> {
        egui::Image::new(source).fit_to_exact_size(egui::vec2(size, size))
    }

    /// An icon-only button whose icon follows the button's text colour (and hover state).
    pub fn button(source: ImageSource<'static>) -> egui::Button<'static> {
        egui::Button::image(sized(source, 16.0)).image_tint_follows_text_color(true)
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Download Anything")
            .with_app_id("download-anything")
            .with_inner_size([460.0, 320.0])
            .with_min_inner_size([380.0, 280.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).unwrap(),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "Download Anything",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx); // renders the SVG icons
            cc.egui_ctx.all_styles_mut(|s| {
                s.spacing.item_spacing = egui::vec2(8.0, 8.0);
                s.spacing.button_padding = egui::vec2(10.0, 5.0);
                s.interaction.selectable_labels = false; // so clicks reach the file rows
            });
            #[allow(unused_mut)]
            let mut app = App::new();
            #[cfg(any(target_os = "macos", windows))]
            if menu_bar::saved() {
                app.tray = menu_bar::Tray::new(&cc.egui_ctx);
            }
            Ok(Box::new(app))
        }),
    )
}

struct Job {
    child: Child,
    lines: Receiver<String>,
    update: bool,
    cancelled: bool,
}

struct App {
    url: String,
    audio: bool,
    job: Option<Job>,
    progress: f32,
    status: String,
    files: Vec<PathBuf>, // ponytail: this session only, no history saved
    last_file: Option<PathBuf>,
    last_download: Vec<String>, // what Retry runs again
    failed: bool,
    #[cfg(any(target_os = "macos", windows))]
    tray: Option<menu_bar::Tray>,
    #[cfg(any(target_os = "macos", windows))]
    hidden: bool,
}

impl App {
    fn new() -> Self {
        let clip = arboard::Clipboard::new()
            .and_then(|mut c| c.get_text())
            .unwrap_or_default();
        Self {
            url: if clip.trim().starts_with("http") {
                clip.trim().into()
            } else {
                String::new()
            },
            audio: true,
            job: None,
            progress: 0.0,
            status: "Paste a link, pick MP3 or MP4, hit Download.".into(),
            files: vec![],
            last_file: None,
            last_download: vec![],
            failed: false,
            #[cfg(any(target_os = "macos", windows))]
            tray: None,
            #[cfg(any(target_os = "macos", windows))]
            hidden: false,
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        let url = self.url.trim();
        if url.is_empty() {
            return;
        }
        let format: &[&str] = if self.audio {
            &["-t", "mp3", "--audio-quality", "0"]
        } else {
            &["-t", "mp4"]
        };
        let mut args = vec!["yt-dlp"];
        args.extend(format);
        args.extend([
            "--embed-metadata",
            "--embed-thumbnail",
            "-o",
            "%(title)s.%(ext)s",
            "--encoding",
            "utf-8",
        ]);
        args.extend([
            "--print",
            "after_move:filepath",
            "--progress",
            "--newline",
            url,
        ]);
        self.last_download = args.iter().map(ToString::to_string).collect();
        self.run(self.last_download.clone(), false, ctx);
    }

    fn run(&mut self, args: Vec<String>, update: bool, ctx: &egui::Context) {
        if self.job.is_some() {
            return;
        }
        self.progress = 0.0;
        self.last_file = None;
        match spawn(&args, ctx) {
            Ok((child, lines)) => {
                self.job = Some(Job {
                    child,
                    lines,
                    update,
                    cancelled: false,
                });
                self.failed = false;
                self.status = "Starting…".into();
            }
            Err(e) => {
                self.failed = true;
                self.status = match (e.kind(), args[0].as_str()) {
                    (std::io::ErrorKind::NotFound, "yt-dlp") => {
                        "yt-dlp is missing. Click Update yt-dlp to install it.".into()
                    }
                    (std::io::ErrorKind::NotFound, tool) => {
                        format!("{tool} is missing. See the setup steps on the GitHub page.")
                    }
                    _ => e.to_string(),
                };
            }
        }
    }

    fn poll(&mut self) {
        loop {
            let Some(job) = &mut self.job else { return };
            match job.lines.try_recv() {
                Ok(line) => self.show(&line),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => return self.finish(),
            }
        }
    }

    fn show(&mut self, line: &str) {
        let path = Path::new(line);
        if path.is_absolute() && path.is_file() {
            // from --print after_move:filepath; re-downloads jump back to the top
            self.files.retain(|f| f != path);
            self.files.push(path.into());
            self.last_file = Some(path.into());
            return;
        }
        self.status = line.replace("[download]", "").trim().into();
        if let Some(p) = percent(line) {
            self.progress = p / 100.0;
        }
    }

    fn finish(&mut self) {
        let Some(mut job) = self.job.take() else {
            return;
        };
        let ok = job.child.wait().is_ok_and(|s| s.success());
        self.failed = !ok && !job.cancelled;
        self.progress = if ok { 1.0 } else { 0.0 };
        if job.cancelled {
            self.status = "Cancelled.".into();
        } else if job.update && ok {
            self.status = "yt-dlp is up to date. Try your link again.".into();
        } else if job.update {
            // keep the package manager's last line as the error
        } else if self.failed {
            self.status += "\nSite down or changed? Retry, or update yt-dlp.";
        } else if let Some(f) = &self.last_file {
            self.status = format!("Done: {}", file_name(f));
        } else {
            self.status = "Done.".into();
        }
    }
}

impl eframe::App for App {
    #[cfg_attr(not(any(target_os = "macos", windows)), allow(unused_variables))]
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll(); // also runs while the window is hidden in the menu bar
        #[cfg(any(target_os = "macos", windows))]
        self.tray_actions(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(any(target_os = "macos", windows))]
        if self.tray.is_some() && ui.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.set_hidden(&ctx, true);
        }
        // Deleted or moved files drop off the list. Cheap: a few stat calls, only when egui repaints.
        self.files.retain(|f| f.is_file());
        let (mut start, mut retry, mut update) = (false, false, false);

        egui::Panel::top("controls")
            .frame(
                egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin {
                    left: 16,
                    right: 16,
                    top: 16,
                    bottom: 12,
                }),
            )
            .show(ui, |ui| {
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.url)
                        .hint_text("Paste a link…")
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(8.0, 6.0)),
                );
                start = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                egui::Sides::new().show(
                    ui,
                    |ui| {
                        egui::Frame::new()
                            .fill(ui.visuals().extreme_bg_color)
                            .corner_radius(8)
                            .inner_margin(2)
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                for (audio, icon, label) in
                                    [(true, icons::AUDIO, "MP3"), (false, icons::VIDEO, "MP4")]
                                {
                                    let atoms = (icons::sized(icon, 16.0), label);
                                    let button =
                                        egui::Button::selectable(self.audio == audio, atoms)
                                            .image_tint_follows_text_color(true);
                                    if ui.add(button).clicked() {
                                        self.audio = audio;
                                    }
                                }
                            });
                    },
                    |ui| {
                        if let Some(job) = &mut self.job {
                            if ui.button("Cancel").clicked() {
                                job.cancelled = true;
                                let _ = job.child.kill();
                            }
                        } else {
                            let download = egui::Button::new(RichText::new("Download").strong())
                                .fill(ui.visuals().selection.bg_fill)
                                .min_size(egui::vec2(96.0, 0.0));
                            start |= ui
                                .add_enabled(!self.url.trim().is_empty(), download)
                                .clicked();
                        }
                        #[cfg(any(target_os = "macos", windows))]
                        menu_bar::settings(ui, &mut self.tray);
                    },
                );
                if self.job.is_some() || self.progress > 0.0 {
                    ui.add(egui::ProgressBar::new(self.progress).desired_height(4.0));
                } else {
                    ui.add_space(4.0); // no stray dot when idle
                }
                let color = if self.failed {
                    ui.visuals().error_fg_color
                } else {
                    ui.visuals().weak_text_color()
                };
                ui.add(egui::Label::new(RichText::new(&self.status).color(color)).selectable(true));
                if self.failed {
                    ui.horizontal(|ui| {
                        retry = !self.last_download.is_empty() && ui.button("Retry").clicked();
                        // ponytail: manual button, not auto-update; sites break yt-dlp every few months
                        update = ui.button("Update yt-dlp").clicked();
                    });
                }
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(ui.style())
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ui, |ui| {
                egui::Sides::new().show(
                    ui,
                    |ui| ui.label(RichText::new("Recent downloads").small().weak()),
                    |ui| {
                        // The list re-checks its files on every repaint; clicking just triggers one.
                        ui.add(icons::button(icons::REFRESH).frame(false))
                            .on_hover_text("Refresh");
                    },
                );
                if self.files.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space((ui.available_height() / 2.0 - 48.0).max(0.0));
                        let weak = ui.visuals().weak_text_color();
                        ui.add(icons::sized(icons::EMPTY, 40.0).tint(weak));
                        ui.label(RichText::new("No downloads yet").weak());
                        ui.label(
                            RichText::new("Files you download show up here.")
                                .small()
                                .weak(),
                        );
                    });
                    return;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for f in self.files.iter().rev() {
                            file_row(ui, f);
                        }
                    });
            });

        if start {
            self.start(&ctx);
        }
        if retry {
            self.run(self.last_download.clone(), false, &ctx);
        }
        if update {
            self.run(UPDATE.iter().map(ToString::to_string).collect(), true, &ctx);
        }
    }
}

/// One downloaded file: click to open it, or the folder button to show it in its folder.
fn file_row(ui: &mut egui::Ui, path: &Path) {
    let background = ui.painter().add(egui::Shape::Noop);
    let row = ui
        .scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
            egui::Sides::new().shrink_left().show(
                ui,
                |ui| {
                    let icon = if is_audio(path) {
                        icons::AUDIO
                    } else {
                        icons::VIDEO
                    };
                    ui.add(icons::sized(icon, 22.0).tint(ui.visuals().weak_text_color()));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.add(egui::Label::new(file_name(path)).truncate());
                        ui.label(RichText::new(details(path)).small().weak());
                    });
                },
                |ui| {
                    if ui
                        .add(icons::button(icons::FOLDER))
                        .on_hover_text(REVEAL)
                        .clicked()
                    {
                        open(path, true);
                    }
                },
            );
        })
        .response;
    if row.hovered() {
        let fill = ui.visuals().widgets.hovered.weak_bg_fill;
        let rect = row.rect.expand2(egui::vec2(6.0, 3.0));
        ui.painter()
            .set(background, egui::Shape::rect_filled(rect, 6.0, fill));
    }
    if row
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Click to open")
        .clicked()
    {
        open(path, false);
    }
}

/// Menu bar icon (macOS) or system tray icon (Windows). Linux has none: its tray needs GTK.
#[cfg(any(target_os = "macos", windows))]
mod menu_bar {
    use super::App;
    use eframe::egui::{self, RichText};
    use std::path::PathBuf;
    use std::sync::mpsc::{Receiver, channel};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

    #[cfg(target_os = "macos")]
    const ICON: &[u8] = include_bytes!("../assets/tray.png"); // black glyph, macOS tints it
    #[cfg(windows)]
    const ICON: &[u8] = include_bytes!("../assets/icon.png");

    pub enum Action {
        Toggle,
        Show,
        Quit,
    }

    pub struct Tray {
        _icon: TrayIcon,
        actions: Receiver<Action>,
    }

    impl Tray {
        pub fn new(ctx: &egui::Context) -> Option<Self> {
            let (tx, actions) = channel();
            let (tx2, ctx2, ctx3) = (tx.clone(), ctx.clone(), ctx.clone());
            TrayIconEvent::set_event_handler(Some(move |e| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = e
                {
                    let _ = tx.send(Action::Toggle);
                    ctx2.request_repaint();
                }
            }));
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                let _ = tx2.send(if e.id == "quit" {
                    Action::Quit
                } else {
                    Action::Show
                });
                ctx3.request_repaint();
            }));
            let menu = Menu::new();
            menu.append_items(&[
                &MenuItem::with_id("show", "Open Download Anything", true, None),
                &MenuItem::with_id("quit", "Quit", true, None),
            ])
            .ok()?;
            let png = eframe::icon_data::from_png_bytes(ICON).ok()?;
            let image = tray_icon::Icon::from_rgba(png.rgba, png.width, png.height).ok()?;
            #[cfg(target_os = "macos")]
            let builder = TrayIconBuilder::new().with_icon_templated(image); // recoloured to match the menu bar
            #[cfg(windows)]
            let builder = TrayIconBuilder::new().with_icon(image);
            let icon = builder
                .with_menu(Box::new(menu))
                .with_menu_on_left_click(false)
                .with_tooltip("Download Anything")
                .build()
                .ok()?;
            show_in_dock(false);
            Some(Self {
                _icon: icon,
                actions,
            })
        }
    }

    impl Drop for Tray {
        fn drop(&mut self) {
            show_in_dock(true);
        }
    }

    impl App {
        pub fn tray_actions(&mut self, ctx: &egui::Context) {
            while let Some(action) = self.tray.as_ref().and_then(|t| t.actions.try_recv().ok()) {
                let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
                match action {
                    Action::Toggle if !self.hidden && focused => self.set_hidden(ctx, true),
                    Action::Toggle | Action::Show => self.set_hidden(ctx, false),
                    Action::Quit => {
                        if let Some(job) = &mut self.job {
                            let _ = job.child.kill();
                        }
                        std::process::exit(0);
                    }
                }
            }
        }

        pub fn set_hidden(&mut self, ctx: &egui::Context, hidden: bool) {
            self.hidden = hidden;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(!hidden));
            if !hidden {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
    }

    /// The settings (gear) menu.
    pub fn settings(ui: &mut egui::Ui, tray: &mut Option<Tray>) {
        let gear = super::icons::button(super::icons::SETTINGS);
        let (response, _) = egui::containers::menu::MenuButton::from_button(gear).ui(ui, |ui| {
            let mut on = tray.is_some();
            let label = if cfg!(target_os = "macos") {
                "Run from the menu bar (hides the Dock icon)"
            } else {
                "Run from the system tray"
            };
            if ui.checkbox(&mut on, label).changed() {
                *tray = if on { Tray::new(ui.ctx()) } else { None };
                save(tray.is_some());
            }
            ui.label(
                RichText::new(
                    "Closing the window keeps it running.\nClick the icon to open it again.",
                )
                .small()
                .weak(),
            );
        });
        response.on_hover_text("Settings");
    }

    // ponytail: one setting, so it's a marker file; switch to a settings file when there's a second
    fn marker() -> Option<PathBuf> {
        let base = if cfg!(windows) {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else {
            std::env::home_dir().map(|h| h.join("Library/Application Support"))
        }?;
        Some(base.join("Download Anything").join("menu-bar-mode"))
    }

    pub fn saved() -> bool {
        marker().is_some_and(|m| m.exists())
    }

    fn save(on: bool) {
        let Some(m) = marker() else { return };
        let _ = if on {
            std::fs::create_dir_all(m.parent().unwrap()).and_then(|_| std::fs::write(&m, ""))
        } else {
            std::fs::remove_file(&m)
        };
    }

    fn show_in_dock(show: bool) {
        #[cfg(target_os = "macos")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy as Policy};
            let app = NSApplication::sharedApplication(mtm);
            app.setActivationPolicy(if show {
                Policy::Regular
            } else {
                Policy::Accessory
            });
            #[allow(deprecated)] // activate() needs macOS 14
            app.activateIgnoringOtherApps(true); // switching policy can push our window behind others
        }
        #[cfg(windows)]
        let _ = show; // a hidden window already leaves the taskbar
    }
}

/// Runs a command with stdout+stderr merged, sending each output line to the UI.
fn spawn(args: &[String], ctx: &egui::Context) -> std::io::Result<(Child, Receiver<String>)> {
    let dir = downloads_dir();
    std::fs::create_dir_all(&dir)?; // in case Downloads was deleted
    let (reader, writer) = std::io::pipe()?;
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..])
        .current_dir(dir)
        .env("PATH", search_path())
        .stdin(Stdio::null())
        .stdout(writer.try_clone()?)
        .stderr(writer);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // CREATE_NO_WINDOW
    let child = cmd.spawn()?;
    drop(cmd); // close our copies of the pipe's write end so the reader sees EOF when the child exits
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(reader).split(b'\n').map_while(Result::ok) {
            if tx
                .send(String::from_utf8_lossy(&line).trim_end().to_owned())
                .is_err()
            {
                return;
            }
            ctx.request_repaint();
        }
        drop(tx);
        ctx.request_repaint(); // let the UI notice the job finished
    });
    Ok((child, rx))
}

/// GUI apps get a bare PATH, so add where Homebrew, winget, pipx and Deno's installer put things.
fn search_path() -> OsString {
    let old = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(Path::new(&home).join(".local/bin"));
        paths.push(Path::new(&home).join(".deno/bin"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        paths.push(
            Path::new(&local)
                .join("Microsoft")
                .join("WinGet")
                .join("Links"),
        );
    }
    paths.extend(std::env::split_paths(&old));
    std::env::join_paths(paths).unwrap_or(old)
}

fn downloads_dir() -> PathBuf {
    // ponytail: assumes the default Downloads folder; a relocated one on Windows isn't followed
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap_or_default();
    PathBuf::from(home).join("Downloads")
}

fn open(path: &Path, reveal: bool) {
    #[cfg(target_os = "macos")]
    let result = Command::new("open")
        .args(reveal.then_some("-R"))
        .arg(path)
        .spawn();
    #[cfg(windows)]
    let result = {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("explorer");
        if reveal {
            cmd.raw_arg(format!("/select,\"{}\"", path.display()));
        } else {
            cmd.arg(path);
        }
        cmd.spawn()
    };
    // ponytail: Linux opens the containing folder instead of highlighting the file
    #[cfg(not(any(target_os = "macos", windows)))]
    let result = Command::new("xdg-open")
        .arg(if reveal {
            path.parent().unwrap_or(path)
        } else {
            path
        })
        .spawn();
    let _ = result;
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into()
}

/// "MP3 · 4.2 MB"
fn details(path: &Path) -> String {
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_uppercase();
    let mb = std::fs::metadata(path).map_or(0.0, |m| m.len() as f64 / 1e6);
    format!("{ext} · {mb:.1} MB")
}

fn is_audio(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("mp3" | "m4a" | "opus" | "ogg" | "wav" | "flac")
    )
}

/// "[download]  42.3% of 3.45MiB" -> 42.3
fn percent(line: &str) -> Option<f32> {
    let before = &line[..line.find('%')?];
    before
        .rsplit(|c: char| !(c.is_ascii_digit() || c == '.'))
        .next()?
        .parse()
        .ok()
}

#[test]
fn parses_progress() {
    assert_eq!(
        percent("[download]  42.3% of   3.45MiB at  1.23MiB/s ETA 00:02"),
        Some(42.3)
    );
    assert_eq!(percent("[download] 100% of 3.45MiB"), Some(100.0));
    assert_eq!(percent("[download] Destination: 50% off…"), Some(50.0));
    assert_eq!(percent("WARNING: no progress here"), None);
    assert_eq!(percent("weird … %"), None);
}
