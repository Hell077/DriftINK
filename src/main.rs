use driftink::{app, control, ipc, ipc::Result, wayland};

fn main() {
    if let Err(error) = run() {
        eprintln!("driftink: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => app::run(false),
        Some("draw") => {
            if control::socket_path()?.exists() && control::send("status").is_ok() {
                println!("{}", control::send("enable")?);
                Ok(())
            } else {
                app::run(true)
            }
        }
        Some(
            command @ ("toggle" | "enable" | "disable" | "status" | "quit" | "undo" | "redo"
            | "save"),
        ) => {
            println!("{}", control::send(command)?);
            Ok(())
        }
        Some("doctor") => {
            let state = ipc::snapshot()?;
            println!("IPC: {}", ipc::socket_path()?.display());
            for o in state.outputs {
                println!(
                    "{}: {:?} logical pixels, camera {:?}, zoom {}",
                    o.name, o.size, o.camera, o.zoom
                );
            }
            Ok(())
        }
        Some("preview") => {
            let seconds = args
                .get(1)
                .map(|s| s.parse::<u64>())
                .transpose()?
                .unwrap_or(120);
            wayland::run(false, Some(seconds), true)
        }
        Some("poc") => {
            let strategy = args.get(1).map(String::as_str).unwrap_or("canvas");
            if !["canvas", "overlay"].contains(&strategy) {
                return Err("strategy must be canvas or overlay".into());
            }
            let seconds = args.get(2).map(|s| s.parse::<u64>()).transpose()?;
            wayland::run(strategy == "canvas", seconds, false)
        }
        _ => {
            println!(
                "Usage: driftink run | draw | toggle | enable | disable | status | save | undo | redo | quit | doctor\nExperiments: driftink preview [seconds] | poc [canvas|overlay] [seconds]"
            );
            Ok(())
        }
    }
}
