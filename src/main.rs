#![windows_subsystem = "windows"] // no console window on Windows

use eframe::egui;
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

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Download Anything")
            .with_app_id("download-anything")
            .with_inner_size([480.0, 460.0])
            .with_min_inner_size([400.0, 340.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).unwrap(),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "Download Anything",
        options,
        Box::new(|cc| {
            cc.egui_ctx.all_styles_mut(|s| {
                s.spacing.item_spacing = egui::vec2(8.0, 8.0);
                s.spacing.button_padding = egui::vec2(10.0, 5.0);
            });
            Ok(Box::new(App::new()))
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
    failed: bool,
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
            failed: false,
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        let url = self.url.trim().to_owned();
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
            &url,
        ]);
        self.run(&args, false, ctx);
    }

    fn run(&mut self, args: &[&str], update: bool, ctx: &egui::Context) {
        if self.job.is_some() {
            return;
        }
        self.progress = 0.0;
        self.last_file = None;
        match spawn(args, ctx) {
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
                self.status = match (e.kind(), args[0]) {
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
        self.failed = !ok && !job.cancelled && !job.update;
        self.progress = if ok { 1.0 } else { 0.0 };
        if job.cancelled {
            self.status = "Cancelled.".into();
        } else if job.update {
            if ok {
                self.status = "yt-dlp is up to date. Try your link again.".into();
            }
        } else if self.failed {
            self.status += "\nSite changed? Click Update yt-dlp, then try again.";
        } else if let Some(f) = &self.last_file {
            self.status = format!("Done: {}", file_name(f));
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.poll();
        let ctx = ui.ctx().clone();
        let (mut start, mut update) = (false, false);

        egui::Panel::top("controls")
            .frame(egui::Frame::central_panel(ui.style()).inner_margin(16))
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
                        ui.selectable_value(&mut self.audio, true, "MP3");
                        ui.selectable_value(&mut self.audio, false, "MP4");
                    },
                    |ui| {
                        if let Some(job) = &mut self.job {
                            if ui.button("Cancel").clicked() {
                                job.cancelled = true;
                                let _ = job.child.kill();
                            }
                        } else {
                            let download =
                                egui::Button::new(egui::RichText::new("Download").strong())
                                    .fill(ui.visuals().selection.bg_fill)
                                    .min_size(egui::vec2(96.0, 0.0));
                            start |= ui
                                .add_enabled(!self.url.trim().is_empty(), download)
                                .clicked();
                            // ponytail: manual button, not auto-update; sites break yt-dlp every few months
                            update = self.failed && ui.button("Update yt-dlp").clicked();
                        }
                    },
                );
                ui.add(egui::ProgressBar::new(self.progress).desired_height(6.0));
                ui.label(egui::RichText::new(&self.status).weak());
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(ui.style())
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ui, |ui| {
                if self.files.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new(
                                "No downloads yet\nFiles you download show up here.",
                            )
                            .weak(),
                        )
                    });
                    return;
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for f in self.files.iter().rev() {
                        egui::Sides::new().shrink_left().show(
                            ui,
                            |ui| {
                                let icon = if is_audio(f) { "🎵" } else { "🎬" };
                                let name = egui::Label::new(format!("{icon}  {}", file_name(f)))
                                    .truncate()
                                    .sense(egui::Sense::click());
                                if ui.add(name).on_hover_text("Click to open").clicked() {
                                    open(f, false);
                                }
                            },
                            |ui| {
                                if ui
                                    .small_button("🔍")
                                    .on_hover_text("Show in folder")
                                    .clicked()
                                {
                                    open(f, true);
                                }
                            },
                        );
                    }
                });
            });

        if start {
            self.start(&ctx);
        }
        if update {
            self.run(UPDATE, true, &ctx);
        }
    }
}

/// Runs a command with stdout+stderr merged, sending each output line to the UI.
fn spawn(args: &[&str], ctx: &egui::Context) -> std::io::Result<(Child, Receiver<String>)> {
    let (reader, writer) = std::io::pipe()?;
    let mut cmd = Command::new(args[0]);
    cmd.args(&args[1..])
        .current_dir(downloads_dir())
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
