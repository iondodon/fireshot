use clap::{Parser, Subcommand};
use fireshot_core::{CaptureError, CaptureRequest};
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Tray, TrayService};
use log::{debug, error};
use tokio::sync::{mpsc, oneshot};
use zbus::dbus_interface;

#[derive(Parser)]
#[command(
    name = "fireshot",
    version,
    about = "Wayland-first Fireshot rewrite (MVP)",
    after_help = "Examples:\n  fireshot gui\n  fireshot gui -d 2000 -p /tmp/cap.png\n  fireshot full -p /tmp/cap.png\n  fireshot full --edit\n\nPortal notes:\n  Requires xdg-desktop-portal and a backend (wlr/gnome/kde)."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Capture fullscreen for editor (selection happens in the editor).
    Gui {
        /// Delay in milliseconds before requesting capture.
        #[arg(short, long, default_value_t = 0)]
        delay: u64,
        /// Save the capture to a path.
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Capture and save without opening the editor.
    Full {
        /// Delay in milliseconds before requesting capture.
        #[arg(short, long, default_value_t = 0)]
        delay: u64,
        /// Save the capture to a path.
        #[arg(short, long)]
        path: Option<String>,
        /// Open the editor after capture.
        #[arg(long, default_value_t = false)]
        edit: bool,
    },
    /// Run DBus daemon to handle capture requests.
    Daemon,
    /// Print portal and environment diagnostics.
    Diagnose {
        /// Attempt a Screenshot portal call (will prompt).
        #[arg(long)]
        ping: bool,
    },
}

fn main() -> Result<(), CaptureError> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| CaptureError::Io(e.to_string()))?;

    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Gui { delay: 0, path: None });

    match command {
        Command::Diagnose { ping } => {
            diagnose(&rt, ping);
        }
        Command::Gui { delay, path } => {
            let req = CaptureRequest {
                delay_ms: delay,
                ..Default::default()
            };
            if req.delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(req.delay_ms));
            }

            let captured = run_async(&rt, fireshot_portal::capture_fullscreen())?;

            if let Some(save_path) = path.as_ref() {
                captured
                    .image
                    .save(save_path)
                    .map_err(|e| CaptureError::Io(e.to_string()))?;
            }

            if path.is_none() {
                fireshot_gui::run_viewer(captured.image)?;
            }
        }
        Command::Full { delay, path, edit } => {
            let req = CaptureRequest {
                delay_ms: delay,
                ..Default::default()
            };
            if req.delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(req.delay_ms));
            }

            let captured = run_async(&rt, fireshot_portal::capture_fullscreen())?;
            if let Some(save_path) = path.as_ref() {
                captured
                    .image
                    .save(save_path)
                    .map_err(|e| CaptureError::Io(e.to_string()))?;
            }
            if edit {
                fireshot_gui::run_viewer(captured.image)?;
            } else if path.is_none() {
                let save_path = "screenshot.png".to_string();
                captured
                    .image
                    .save(&save_path)
                    .map_err(|e| CaptureError::Io(e.to_string()))?;
            }
        }
        Command::Daemon => {
            run_daemon(&rt)?;
        }
    }

    Ok(())
}

fn diagnose(rt: &tokio::runtime::Runtime, ping: bool) {
    println!("Fireshot Wayland diagnostics");
    println!("env:");
    for key in [
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
        "WAYLAND_DISPLAY",
        "DISPLAY",
    ] {
        let val = std::env::var(key).unwrap_or_else(|_| "<unset>".to_string());
        println!("  {}={}", key, val);
    }

    println!();
    println!("portal service:");
    let dbus_result = rt.block_on(async {
        let conn = zbus::Connection::session().await?;
        let proxy = zbus::fdo::DBusProxy::new(&conn).await?;
        let name = zbus::names::BusName::try_from("org.freedesktop.portal.Desktop")
            .map_err(zbus::Error::Names)?;
        let has_owner = proxy.name_has_owner(name).await?;
        Ok::<bool, zbus::Error>(has_owner)
    });
    match dbus_result {
        Ok(has_owner) => println!("  org.freedesktop.portal.Desktop: {}", has_owner),
        Err(err) => println!("  session bus error: {}", err),
    }

    println!();
    println!("portal backends:");
    let portals_dir = std::path::Path::new("/usr/share/xdg-desktop-portal/portals");
    if portals_dir.exists() {
        match std::fs::read_dir(portals_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        println!("  {}", name);
                    }
                }
            }
            Err(err) => println!("  error reading {}: {}", portals_dir.display(), err),
        }
    } else {
        println!("  {} not found", portals_dir.display());
    }

    println!();
    println!("portals.conf:");
    for path in [
        format!(
            "{}/.config/xdg-desktop-portal/portals.conf",
            std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
        ),
        "/usr/share/xdg-desktop-portal/portals.conf".to_string(),
    ] {
        let p = std::path::Path::new(&path);
        if p.exists() {
            println!("  {}", p.display());
            match std::fs::read_to_string(p) {
                Ok(contents) => {
                    for line in contents.lines() {
                        println!("    {}", line);
                    }
                }
                Err(err) => println!("    error: {}", err),
            }
        }
    }

    if ping {
        println!();
        println!("portal ping:");
        let ping_result = run_async(rt, fireshot_portal::probe_screenshot());
        match ping_result {
            Ok(uri) => println!("  screenshot ok: {}", uri),
            Err(err) => println!("  screenshot error: {}", err),
        }
    }
}

fn run_async<T>(
    rt: &tokio::runtime::Runtime,
    future: impl std::future::Future<Output = Result<T, CaptureError>>,
) -> Result<T, CaptureError> {
    rt.block_on(future)
}

struct FireshotService {
    shutdown: std::sync::Mutex<Option<oneshot::Sender<()>>>,
}

#[dbus_interface(name = "org.fireshot.Fireshot")]
impl FireshotService {
    fn gui(&self, delay_ms: u64, path: String) {
        let path = if path.is_empty() { None } else { Some(path) };
        spawn_capture(CaptureKind::Gui { delay_ms, path });
    }

    fn full(&self, delay_ms: u64, path: String) {
        let path = if path.is_empty() { None } else { Some(path) };
        spawn_capture(CaptureKind::Full { delay_ms, path, edit: false });
    }

    fn full_gui(&self, delay_ms: u64, path: String) {
        let path = if path.is_empty() { None } else { Some(path) };
        spawn_capture(CaptureKind::Full { delay_ms, path, edit: true });
    }

    fn quit(&self) {
        if let Some(sender) = self.shutdown.lock().ok().and_then(|mut s| s.take()) {
            let _ = sender.send(());
        }
    }

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

enum CaptureKind {
    Gui { delay_ms: u64, path: Option<String> },
    Full { delay_ms: u64, path: Option<String>, edit: bool },
}

enum DaemonCommand {
    Gui,
    FullEdit,
    Quit,
}

struct FireshotTray {
    cmd_tx: mpsc::UnboundedSender<DaemonCommand>,
}

impl Tray for FireshotTray {
    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.cmd_tx.send(DaemonCommand::Gui);
    }

    fn id(&self) -> String {
        "fireshot".to_string()
    }

    fn title(&self) -> String {
        "Fireshot".to_string()
    }

    fn icon_name(&self) -> String {
        "camera-photo".to_string()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Capture (GUI)".into(),
                icon_name: "camera-photo".into(),
                activate: Box::new(|this: &mut FireshotTray| {
                    let _ = this.cmd_tx.send(DaemonCommand::Gui);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Full Screen".into(),
                icon_name: "display".into(),
                activate: Box::new(|this: &mut FireshotTray| {
                    let _ = this.cmd_tx.send(DaemonCommand::FullEdit);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut FireshotTray| {
                    let _ = this.cmd_tx.send(DaemonCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn spawn_capture(kind: CaptureKind) {
    std::thread::spawn(move || {
        debug!("spawn_capture: start");
        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(err) => {
                error!("daemon capture: failed to resolve exe: {}", err);
                return;
            }
        };

        let mut cmd = std::process::Command::new(exe);
        match kind {
            CaptureKind::Gui { delay_ms, path } => {
                cmd.arg("gui");
                if delay_ms > 0 {
                    cmd.arg("-d").arg(delay_ms.to_string());
                }
                if let Some(path) = path {
                    cmd.arg("-p").arg(path);
                }
            }
            CaptureKind::Full { delay_ms, path, edit } => {
                cmd.arg("full");
                if delay_ms > 0 {
                    cmd.arg("-d").arg(delay_ms.to_string());
                }
                if let Some(path) = path {
                    cmd.arg("-p").arg(path);
                }
                if edit {
                    cmd.arg("--edit");
                }
            }
        }

        if let Err(err) = cmd.spawn() {
            error!("daemon capture: failed to spawn child: {}", err);
        }
        debug!("spawn_capture: end");
    });
}

fn run_daemon(rt: &tokio::runtime::Runtime) -> Result<(), CaptureError> {
    rt.block_on(async {
        env_logger::builder().is_test(false).try_init().ok();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let service = FireshotService {
            shutdown: std::sync::Mutex::new(Some(shutdown_tx)),
        };
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
        let tray_service = TrayService::new(FireshotTray { cmd_tx });

        let _conn = zbus::ConnectionBuilder::session()
            .map_err(|e| CaptureError::Io(e.to_string()))?
            .name("org.fireshot.Fireshot")
            .map_err(|e| CaptureError::Io(e.to_string()))?
            .serve_at("/org/fireshot/Fireshot", service)
            .map_err(|e| CaptureError::Io(e.to_string()))?
            .build()
            .await
            .map_err(|e| CaptureError::Io(e.to_string()))?;

        tray_service.spawn();
        println!("fireshot daemon running (org.fireshot.Fireshot)");
        tokio::pin!(shutdown_rx);
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                Some(cmd) = cmd_rx.recv() => match cmd {
                    DaemonCommand::Gui => {
                        spawn_capture(CaptureKind::Gui { delay_ms: 0, path: None });
                    }
                    DaemonCommand::FullEdit => {
                        spawn_capture(CaptureKind::Full { delay_ms: 0, path: None, edit: true });
                    }
                    DaemonCommand::Quit => break,
                },
            }
        }
        Ok(())
    })
}
