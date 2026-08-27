use crate::{
    args::Game,
    batcher::build_render_job,
    dpi::{PhysicalPoint, PhysicalSize},
    fonts::{FontData, FontDefinitions, Fonts},
    gfx::GraphicsContext,
    installer::Installer,
    pob::PathOfBuilding,
    renderer::{RenderJob, textures::TextureManager},
    stage::{ActiveStage, StageEvent, StageTransition},
};
use anyhow::Result;
use std::sync::Arc;
use std::{cell::RefCell, path::PathBuf, rc::Rc};
use winit::{
    application::ApplicationHandler, event::*, event_loop::ActiveEventLoop,
    platform::modifier_supplement::KeyEventExtModifierSupplement, window::Window,
};

struct FrameOutput {
    pub render_job: Option<RenderJob>,
    pub request_redraw: bool,
}

pub struct App {
    gfx_context: Option<GraphicsContext>,
    fonts: Rc<RefCell<Fonts>>,
    texture_manager: Rc<RefCell<TextureManager>>,
    stage: ActiveStage,
    game: Game,
    script_dir: PathBuf,
    needs_reconfigure: bool,
}

impl App {
    pub fn new(game: Game, custom_script_dir: Option<PathBuf>) -> Result<Self> {
        let uses_custom_script_dir = custom_script_dir.is_some();
        let script_dir = custom_script_dir.unwrap_or_else(|| game.script_dir());

        let fonts = Rc::new(RefCell::new(Fonts::new(pob_font_definitions())));
        let texture_manager = Rc::new(RefCell::new(TextureManager::new()));

        let stage = if uses_custom_script_dir {
            // skip installer if custom script dir is provided. used for local testing.
            let pob =
                PathOfBuilding::new(&script_dir, Rc::clone(&fonts), Rc::clone(&texture_manager))?;
            ActiveStage::Main(pob)
        } else {
            ActiveStage::Startup(Installer::new(game, Rc::clone(&fonts)))
        };

        Ok(Self {
            gfx_context: None,
            fonts,
            texture_manager,
            stage,
            game,
            script_dir,
            needs_reconfigure: true,
        })
    }

    fn update(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let transition = self.stage.update();
        if let Some(transition) = transition? {
            match transition {
                StageTransition::ToMain => {
                    self.stage = ActiveStage::Main(PathOfBuilding::new(
                        &self.script_dir,
                        Rc::clone(&self.fonts),
                        Rc::clone(&self.texture_manager),
                    )?);
                    if let Some(gfx) = &self.gfx_context {
                        self.stage.set_window(Arc::clone(&gfx.window));
                    }
                }
                StageTransition::ToShutdown => {
                    self.handle_event(StageEvent::Exit);
                    event_loop.exit();
                }
            };
        }
        Ok(())
    }

    fn frame(&mut self, inhibit_elision: bool) -> anyhow::Result<FrameOutput> {
        self.fonts.borrow_mut().begin_frame();

        let stage_output = self.stage.frame()?;

        if let Some(font_image_delta) = self.fonts.borrow_mut().font_atlas_delta() {
            self.texture_manager
                .borrow()
                .update_font_texture(font_image_delta);
        }

        let textures_delta = self.texture_manager.borrow().take_delta();

        let render_job = if stage_output.can_elide && textures_delta.is_empty() && !inhibit_elision
        {
            None
        } else {
            Some(build_render_job(
                &stage_output.draw_commands,
                textures_delta,
                stage_output.scale_factor,
            ))
        };

        Ok(FrameOutput {
            render_job,
            request_redraw: stage_output.request_redraw,
        })
    }

    fn handle_event(&mut self, event: StageEvent) {
        if let Err(err) = self.stage.handle_event(event) {
            log::error!("{err}");
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let (title, _app_id) = match self.game {
            Game::Poe1 => ("Path of Building 1", "rusty-path-of-building-1"),
            Game::Poe2 => ("Path of Building 2", "rusty-path-of-building-2"),
        };

        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes()
            .with_title(title)
            .with_window_icon(load_icon());

        #[cfg(target_os = "linux")]
        {
            use winit::platform::wayland::ActiveEventLoopExtWayland;
            use winit::platform::x11::ActiveEventLoopExtX11;

            if event_loop.is_x11() {
                use winit::platform::x11::WindowAttributesExtX11;
                window_attributes = window_attributes.with_name(_app_id, _app_id);
            } else if event_loop.is_wayland() {
                use winit::platform::wayland::WindowAttributesExtWayland;
                window_attributes = window_attributes.with_name(_app_id, _app_id);
            }
        }

        let window = event_loop.create_window(window_attributes)?;
        let window = Arc::new(window);
        self.stage.set_window(Arc::clone(&window));
        self.gfx_context = Some(pollster::block_on(GraphicsContext::new(window))?);

        Ok(())
    }

    fn request_redraw(&self) {
        if let Some(gfx) = &self.gfx_context {
            gfx.window.request_redraw();
        }
    }
}

impl ApplicationHandler<GraphicsContext> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(err) = self.create_window(event_loop) {
            log::error!("{err}");
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                if self.stage.can_exit() {
                    self.handle_event(StageEvent::Exit);
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                profiling::scope!("RedrawRequested");

                if let Err(err) = self.update(event_loop) {
                    log::error!("{err}");
                    event_loop.exit();
                    return;
                }

                let mut inhibit_elision = false;
                if self.needs_reconfigure {
                    if let Some(ref mut gfx) = self.gfx_context {
                        let size = gfx.window.inner_size();
                        gfx.resize(size.width, size.height);
                        // resizing recreates the blip texture (previous frame) thus elision needs to
                        // be inhibited for at least one frame.
                        inhibit_elision = true;
                    }
                    self.needs_reconfigure = false;
                }

                let FrameOutput {
                    render_job,
                    request_redraw,
                } = match self.frame(inhibit_elision) {
                    Ok(frame_output) => frame_output,
                    Err(err) => {
                        log::error!("{err}");
                        event_loop.exit();
                        return;
                    }
                };

                if let Some(ref mut gfx) = self.gfx_context {
                    match gfx.render(render_job) {
                        Ok(_) => {}
                        Err(err) => {
                            log::error!("Unable to render: {err}");
                        }
                    }
                }

                if request_redraw {
                    self.request_redraw();
                }

                profiling::finish_frame!();
            }
            WindowEvent::Resized(size) => {
                self.needs_reconfigure = true;
                self.handle_event(StageEvent::Resized(PhysicalSize::new(
                    size.width,
                    size.height,
                )));
                self.request_redraw();
            }
            WindowEvent::Focused(focused) => {
                self.handle_event(StageEvent::FocusChanged(focused));
                if focused {
                    self.request_redraw();
                } else {
                    // Clear inputs on lost focus to avoid "stuck" keys on Wayland systems.
                    self.stage.clear_pressed();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.handle_event(StageEvent::ScaleFactorChanged(scale_factor));
                self.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let state = event.state;

                // forward KeyUp/KeyDown events
                let stage_event = match state {
                    ElementState::Pressed => StageEvent::KeyDown {
                        key: event.logical_key.clone(),
                    },
                    ElementState::Released => StageEvent::KeyUp {
                        key: event.logical_key.clone(),
                    },
                };
                self.handle_event(stage_event);

                // handle text input
                if let Some(text) = event.text_with_all_modifiers()
                    && state.is_pressed()
                {
                    for ch in text.chars() {
                        let event = StageEvent::CharacterInput { ch };
                        self.handle_event(event);
                    }
                }
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.handle_event(StageEvent::ModifiersChanged {
                    state: modifiers.state(),
                });
                self.request_redraw();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let event = match state {
                    ElementState::Pressed => StageEvent::MouseDown { button },
                    ElementState::Released => StageEvent::MouseUp { button },
                };
                self.handle_event(event);
                self.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = PhysicalPoint::new(position.x as f32, position.y as f32);
                self.handle_event(StageEvent::MouseMoved { pos });
                self.request_redraw();
            }
            WindowEvent::CursorEntered { .. } => {
                self.handle_event(StageEvent::HoverChanged(true));
                self.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.handle_event(StageEvent::HoverChanged(false));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition { y, .. }) => {
                        y as f32
                    }
                };
                self.handle_event(StageEvent::MouseWheel { delta });
                self.request_redraw();
            }
            _ => {}
        }
    }
}

fn pob_font_definitions() -> FontDefinitions {
    let mut definitions = FontDefinitions::default();

    definitions.font_data.insert(
        "bitstream-vera-sans-mono".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../fonts/VeraMono.ttf"
        ))),
    );
    definitions.font_data.insert(
        "liberation-sans".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../fonts/LiberationSans-Regular.ttf"
        ))),
    );
    definitions.font_data.insert(
        "liberation-sans-bold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../fonts/LiberationSans-Bold.ttf"
        ))),
    );
    definitions.font_data.insert(
        "fontin-regular".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../fonts/fontin-regular.ttf"
        ))),
    );
    definitions.font_data.insert(
        "fontin-italic".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../fonts/fontin-italic.ttf"
        ))),
    );
    definitions.font_data.insert(
        "fontin-smallcaps".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../fonts/fontin-smallcaps.ttf"
        ))),
    );

    definitions.generic_families.insert(
        parley::GenericFamily::Monospace,
        vec!["Bitstream Vera Sans Mono".to_owned()],
    );

    definitions.generic_families.insert(
        parley::GenericFamily::SansSerif,
        vec!["Liberation Sans".to_owned()],
    );

    definitions.generic_families.insert(
        parley::GenericFamily::Serif,
        vec!["Fontin".to_owned(), "Fontin SmallCaps".to_owned()],
    );

    definitions
}

fn load_icon() -> Option<winit::window::Icon> {
    let image_data = include_bytes!("../assets/icon.png");
    let image = image::load_from_memory(image_data).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    winit::window::Icon::from_rgba(image.into_raw(), width, height).ok()
}
