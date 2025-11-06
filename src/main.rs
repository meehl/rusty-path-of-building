use crate::{
    app::App,
    args::{Args, Game},
};
use clap::Parser;
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
    Game::init(args.game);

    let mut app = App::new()?;

    let event_loop = EventLoop::with_user_event().build()?;
    event_loop.run_app(&mut app)?;

    Ok(())
}
