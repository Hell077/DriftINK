mod camera;
mod ipc;
mod wayland;
use ipc::Result;

fn main() {
    if let Err(error) = run() {
        eprintln!("driftink: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
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
                "Usage: driftink preview [seconds] | driftink doctor | driftink poc [canvas|overlay] [seconds]\nPhase 0 only: click-through test rectangle centered at world (0, 0).\nCanvas strategy requires the supplied driftwm rule. Overlay requires no rule."
            );
            Ok(())
        }
    }
}
