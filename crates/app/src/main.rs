use clap::{Parser, Subcommand};
use fireshot_core::{CaptureError, CaptureRequest};

#[derive(Parser)]
#[command(name = "fireshot", version, about = "Wayland-first Fireshot rewrite (MVP)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Interactive capture with editor window.
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
    },
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

            let captured = run_async(&rt, fireshot_portal::capture_interactive())?;

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
        Command::Full { delay, path } => {
            let req = CaptureRequest {
                delay_ms: delay,
                ..Default::default()
            };
            if req.delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(req.delay_ms));
            }

            let captured = run_async(&rt, fireshot_portal::capture_interactive())?;
            let save_path = path.unwrap_or_else(|| "screenshot.png".to_string());
            captured
                .image
                .save(&save_path)
                .map_err(|e| CaptureError::Io(e.to_string()))?;
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
