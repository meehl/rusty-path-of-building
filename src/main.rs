use crate::{app::App, args::Args};
use clap::Parser;
use std::path::{Path, PathBuf};
use winit::event_loop::EventLoop;

mod api;
mod app;
mod args;
mod clipboard;
mod color;
mod dpi;
mod fonts;
mod gfx;
mod input;
mod installer;
mod layers;
mod lua;
mod math;
mod mode;
mod pob;
mod renderer;
mod subscript;
mod util;
mod window;
mod worker_pool;

fn main() -> anyhow::Result<()> {
    profiling::register_thread!("Main Thread");
    env_logger::init();

    #[cfg(feature = "profile-with-puffin")]
    let _puffin_server = {
        let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
        let server = puffin_http::Server::new(&server_addr).unwrap();
        eprintln!("Serving profiling data on {server_addr}. Run `puffin_viewer` to view it.");
        profiling::puffin::set_scopes_on(true);
        server
    };

    let args = Args::parse();
    let script_dir = find_nearby_launch_script();

    let mut app = App::new(args.game, script_dir)?;

    let event_loop = EventLoop::with_user_event().build()?;
    event_loop.run_app(&mut app)?;

    Ok(())
}

/// Search for the Launch.lua file in nearby locations
fn find_nearby_launch_script() -> Option<PathBuf> {
    let mut candidates = vec![Path::new("Launch.lua"), Path::new("src/Launch.lua")];

    if let Ok(cwd) = std::env::current_dir()
        && cwd.ends_with("runtime")
    {
        candidates.push(Path::new("../src/Launch.lua"));
    }

    for candidate in candidates {
        if candidate.try_exists().is_ok_and(|exists| exists) {
            if let Some(Ok(candidate)) = candidate.parent().map(Path::canonicalize) {
                return Some(candidate);
            }
        }
    }

    None
}
