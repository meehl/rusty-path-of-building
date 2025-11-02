use crate::{
    args::Game,
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
use std::sync::Arc;
use winit::{
    application::ApplicationHandler, event::*, event_loop::ActiveEventLoop, keyboard::PhysicalKey,
    platform::modifier_supplement::KeyEventExtModifierSupplement, window::Window,
};

struct FrameOutput {
    pub render_job: RenderJob,
    pub should_continue: bool,
}

pub struct AppState {
    pub window: WindowState,
    pub input: InputState,
    pub fonts: Fonts,
    pub texture_manager: WrappedTextureManager,
}

impl AppState {
    fn set_mouse_pos(&mut self, pos: PhysicalPoint<f32>) {
        self.input
            .set_mouse_pos(pos.to_logical(self.window.scale_factor()));
    }
}

pub struct App {
    gfx_context: Option<GraphicsContext>,
    state: AppState,
    tessellator: Tessellator,
    needs_reconfigure: bool,
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
            },
            tessellator: Tessellator::default(),
            needs_reconfigure: true,
            force_render: true,
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
            let meshes = self.tessellator.convert_clipped_primitives(
                mode_output.primitives,
                font_atlas_size,
                self.state.window.scale_factor(),
            );

            RenderJob::Render {
                meshes,
                textures_delta,
            }
        };

        Ok(FrameOutput {
            render_job,
            should_continue: mode_output.should_continue,
        })
    }

    fn handle_event(&mut self, event: AppEvent) {
        if let Err(err) = self.current_mode.handle_event(&mut self.state, event) {
            log::error!("{err}");
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let (title, _app_id) = match Game::current() {
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
        self.state.window.set_window(Arc::clone(&window));
        self.gfx_context = Some(pollster::block_on(GraphicsContext::new(window))?);

        Ok(())
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

                if self.needs_reconfigure {
                    if let Some(ref mut gfx) = self.gfx_context {
                        let size = gfx.window.inner_size();
                        gfx.resize(size.width, size.height);
                    }
                    self.needs_reconfigure = false;
                    // Render at least one frame after reconfigure
                    self.force_render = true;
                }

                let is_focused = self.state.window.is_focused;
                let is_hovered = self.state.window.is_hovered;
                let should_render = is_focused || is_hovered || self.force_render;

                if should_render {
                    let FrameOutput {
                        render_job,
                        should_continue,
                    } = match self.frame() {
                        Ok(frame_output) => frame_output,
                        Err(err) => {
                            log::error!("{err}");
                            event_loop.exit();
                            return;
                        }
                    };

                    if let Some(ref mut gfx) = self.gfx_context {
                        match gfx.render(render_job, self.state.window.scale_factor()) {
                            Ok(_) => {
                                self.force_render = should_continue;

                                if is_focused || is_hovered || should_continue {
                                    self.state.window.request_redraw();
                                }
                            }
                            // Reconfigure the surface if it's lost or outdated
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                self.needs_reconfigure = true;
                                self.state.window.request_redraw();
                            }
                            Err(err) => {
                                log::error!("Unable to render: {err}");
                            }
                        }
                    }
                }

                profiling::finish_frame!();
            }
            WindowEvent::Resized(size) => {
                self.state.window.size = PhysicalSize::new(size.width, size.height);
                self.needs_reconfigure = true;
            }
            WindowEvent::Focused(focused) => {
                self.state.window.is_focused = focused;
                if focused {
                    self.state.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.state.window.set_scale_factor(scale_factor as f32);
            }
            WindowEvent::KeyboardInput {
                event:
                    key_event @ KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => {
                // update input state
                self.state.input.set_key_pressed(code, state.is_pressed());

                // forward KeyUp/KeyDown events
                let event = match state {
                    ElementState::Pressed => AppEvent::KeyDown { code },
                    ElementState::Released => AppEvent::KeyUp { code },
                };
                self.handle_event(event);

                // handle text input
                if let Some(text) = key_event.text_with_all_modifiers()
                    && state.is_pressed()
                {
                    for ch in text.chars() {
                        let event = AppEvent::CharacterInput { ch };
                        self.handle_event(event);
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
            WindowEvent::CursorEntered { .. } => {
                self.state.window.is_hovered = true;
                self.state.window.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.state.window.is_hovered = false;
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

fn load_icon() -> Option<winit::window::Icon> {
    let image_data = include_bytes!("../assets/icon.png");
    let image = image::load_from_memory(image_data).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    winit::window::Icon::from_rgba(image.into_raw(), width, height).ok()
}
