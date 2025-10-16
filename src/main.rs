use crate::app::App;
use crate::args::{Args, Game};
use crate::installer::run_installer;
use clap::Parser;
use winit::event_loop::EventLoop;

mod api;
mod app;
mod args;
mod color;
mod context;
mod dpi;
mod fonts;
mod gfx;
mod input;
mod installer;
mod layers;
mod lua;
mod math;
mod renderer;
mod subscript;
mod util;
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
    Game::init(args.game);

    run_installer()?;

    let mut app = App::new()?;

    let event_loop = EventLoop::with_user_event().build()?;
    event_loop.run_app(&mut app)?;

    Ok(())
}
