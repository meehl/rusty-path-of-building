use crate::{
    dpi::{ConvertToLogical, PhysicalPoint, PhysicalSize},
    fonts::{FontData, FontDefinitions, Fonts},
    gfx::{GraphicsContext, RenderJob},
    input::InputState,
    installer::InstallMode,
    mode::{AppEvent, AppMode, ModeTransition},
    pob::PoBMode,
    renderer::{tessellator::Tessellator, textures::WrappedTextureManager},
    window::WindowState,
};
use anyhow::Result;
use arboard::Clipboard;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::ActiveEventLoop,
    keyboard::{ModifiersState, PhysicalKey},
    window::Window,
};

struct FrameOutput {
    pub render_job: RenderJob,
}

pub struct AppState {
    pub window: WindowState,
    pub input: InputState,
    pub fonts: Fonts,
    pub texture_manager: WrappedTextureManager,
    pub clipboard: Clipboard,
}

impl AppState {
    fn set_mouse_pos(&mut self, pos: PhysicalPoint<f32>) {
        self.input
            .set_mouse_pos(pos.to_logical(self.window.scale_factor));
    }
}

pub struct App {
    gfx_context: Option<GraphicsContext>,
    state: AppState,
    tessellator: Tessellator,
    force_render: bool,
    current_mode: AppMode,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            gfx_context: None,
            state: AppState {
                window: WindowState::default(),
                input: InputState::default(),
                fonts: Fonts::new(pob_font_definitions()),
                texture_manager: WrappedTextureManager::new(),
                clipboard: arboard::Clipboard::new()?,
            },
            tessellator: Tessellator::default(),
            force_render: false,
            current_mode: AppMode::Install(InstallMode::new()),
        })
    }

    fn update(&mut self) -> anyhow::Result<()> {
        let transition = self.current_mode.update(&mut self.state)?;
        if let Some(transition) = transition {
            self.current_mode = match transition {
                ModeTransition::PoB => {
                    let pob_mode = PoBMode::new(&mut self.state)?;
                    AppMode::PoB(pob_mode)
                }
            };
        }

        Ok(())
    }

    fn frame(&mut self) -> anyhow::Result<FrameOutput> {
        self.state.fonts.begin_frame();

        let mode_output = self.current_mode.frame(&mut self.state)?;

        let font_atlas_size = self.state.fonts.font_atlas().size();

        if let Some(font_image_delta) = self.state.fonts.font_atlas_delta() {
            self.state
                .texture_manager
                .update_font_texture(font_image_delta);
        }

        let textures_delta = self.state.texture_manager.take_delta();

        let render_job = if mode_output.can_elide && textures_delta.is_empty() && !self.force_render
        {
            RenderJob::Skip
        } else {
            self.force_render = false;

            let meshes = self.tessellator.convert_clipped_primitives(
                mode_output.primitives,
                font_atlas_size,
                self.state.window.scale_factor,
            );

            RenderJob::Render {
                meshes,
                textures_delta,
            }
        };

        Ok(FrameOutput { render_job })
    }

    fn handle_event(&mut self, event: AppEvent) {
        if let Err(err) = self.current_mode.handle_event(&mut self.state, event) {
            log::error!("{err}");
        }
    }
}

impl ApplicationHandler<GraphicsContext> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = match event_loop.create_window(Window::default_attributes()) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                log::error!("{err}");
                event_loop.exit();
                return;
            }
        };

        self.state.window.set_window(Arc::clone(&window));

        self.gfx_context = match pollster::block_on(GraphicsContext::new(window)) {
            Ok(gfx) => Some(gfx),
            Err(err) => {
                log::error!("{err}");
                event_loop.exit();
                return;
            }
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
                self.handle_event(AppEvent::Exit);
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                profiling::scope!("RedrawRequested");

                if let Err(err) = self.update() {
                    log::error!("{err}");
                    event_loop.exit();
                    return;
                }

                let render_job = match self.frame() {
                    Ok(FrameOutput { render_job }) => render_job,
                    Err(err) => {
                        log::error!("{err}");
                        event_loop.exit();
                        return;
                    }
                };

                if let Some(ref mut gfx) = self.gfx_context {
                    match gfx.render(render_job) {
                        Ok(_) => {}
                        // Reconfigure the surface if it's lost or outdated
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            let size = gfx.window.inner_size();
                            gfx.resize(size.width, size.height);
                            self.force_render = true;
                        }
                        Err(err) => {
                            log::error!("Unable to render: {err}");
                        }
                    }
                }

                profiling::finish_frame!();
            }
            WindowEvent::Resized(size) => {
                if let Some(ref mut gfx) = self.gfx_context {
                    gfx.resize(size.width, size.height);
                    self.force_render = true;
                }
                self.state.window.size = PhysicalSize::new(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.state.window.scale_factor = scale_factor as f32;
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        logical_key,
                        state,
                        ..
                    },
                ..
            } => {
                self.state.input.set_key_pressed(code, state.is_pressed());

                let event = match state {
                    ElementState::Pressed => AppEvent::KeyDown { code },
                    ElementState::Released => AppEvent::KeyUp { code },
                };
                self.handle_event(event);

                // handle text input
                if state.is_pressed() {
                    match logical_key {
                        winit::keyboard::Key::Character(text) => {
                            // only emit event if no modifier except Shift is pressed.
                            let modifiers = self.state.input.key_modifiers;
                            if modifiers.difference(ModifiersState::SHIFT).is_empty() {
                                for ch in text.chars() {
                                    let event = AppEvent::CharacterInput { ch };
                                    self.handle_event(event);
                                }
                            }
                        }
                        winit::keyboard::Key::Named(named) => {
                            if named == winit::keyboard::NamedKey::Space {
                                let event = AppEvent::CharacterInput { ch: ' ' };
                                self.handle_event(event);
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.state.input.key_modifiers = modifiers.state();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let is_double_click = self
                    .state
                    .input
                    .set_mouse_pressed(button, state.is_pressed());

                let event = match state {
                    ElementState::Pressed => AppEvent::MouseDown {
                        button,
                        is_double_click,
                    },
                    ElementState::Released => AppEvent::MouseUp { button },
                };
                self.handle_event(event);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = PhysicalPoint::new(position.x as f32, position.y as f32);
                self.state.set_mouse_pos(pos);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition { y, .. }) => {
                        y as f32
                    }
                };
                let event = AppEvent::MouseWheel { delta };
                self.handle_event(event);
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
        vec!["bitstream-vera-sans-mono".to_owned()],
    );

    definitions.generic_families.insert(
        parley::GenericFamily::SansSerif,
        vec![
            "liberation-sans".to_owned(),
            "liberation-sans-bold".to_owned(),
        ],
    );

    definitions.generic_families.insert(
        parley::GenericFamily::Serif,
        vec![
            "fontin-regular".to_owned(),
            "fontin-italic".to_owned(),
            "fontin-smallcaps".to_owned(),
        ],
    );

    definitions
}
