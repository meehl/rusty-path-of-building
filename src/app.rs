use std::sync::Arc;

use anyhow::Result;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::*,
    event_loop::ActiveEventLoop,
    keyboard::{ModifiersState, PhysicalKey},
    window::Window,
};

use crate::{
    context::{CONTEXT, FrameOutput},
    gfx::GraphicsContext,
    input::{keycode_as_str, mousebutton_as_str},
    lua::LuaInstance,
};

pub struct App {
    gfx_context: Option<GraphicsContext>,
    lua_instance: LuaInstance,
}

impl App {
    pub fn new() -> Result<Self> {
        let lua_instance = LuaInstance::new()?;

        Ok(Self {
            gfx_context: None,
            lua_instance,
        })
    }

    fn frame(&mut self) -> FrameOutput {
        // Handle restart requests
        if CONTEXT.with_borrow(|ctx| ctx.needs_restart) {
            match self.lua_instance.restart() {
                Ok(_) => CONTEXT.with_borrow_mut(|ctx| ctx.needs_restart = false),
                Err(e) => panic!("Error occused while restarting lua instance: {}", e),
            }
        };

        self.lua_instance.handle_subscripts();

        // Clear data from previous frame and prepare for new one
        CONTEXT.with_borrow_mut(|ctx| ctx.begin_frame());

        // Call back into lua and tell it to draw a frame
        self.lua_instance.on_frame().unwrap();

        // End frame and get outupts
        CONTEXT.with_borrow_mut(|ctx| ctx.end_frame())
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

        CONTEXT.with_borrow_mut(|ctx| ctx.set_window(Arc::clone(&window)));

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
        CONTEXT.with_borrow_mut(|ctx| ctx.on_window_event(&event));

        match event {
            WindowEvent::CloseRequested => {
                self.lua_instance.on_exit().unwrap();
                event_loop.exit()
            }
            WindowEvent::Resized(size) => {
                if let Some(ref mut gfx) = self.gfx_context {
                    gfx.resize(size.width, size.height);
                    CONTEXT.with_borrow_mut(|ctx| ctx.force_render = true)
                }
            }
            WindowEvent::RedrawRequested => {
                profiling::scope!("RedrawRequested");

                let FrameOutput { render_job } = self.frame();

                if let Some(ref mut gfx) = self.gfx_context {
                    match gfx.render(render_job) {
                        Ok(_) => {}
                        // Reconfigure the surface if it's lost or outdated
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            let size = gfx.window.inner_size();
                            gfx.resize(size.width, size.height);
                            CONTEXT.with_borrow_mut(|ctx| ctx.force_render = true)
                        }
                        Err(err) => {
                            log::error!("Unable to render: {err}");
                        }
                    }
                }

                profiling::finish_frame!();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        text,
                        ..
                    },
                ..
            } => {
                // Only call OnChar if no modifier key except SHIFT is pressed.
                // This prevents 'v' from getting inserted into a text field
                // when 'CTRL + v' is used to paste.
                if state.is_pressed()
                    && CONTEXT.with_borrow(|ctx| {
                        ctx.input_state
                            .modifiers()
                            .difference(ModifiersState::SHIFT)
                            .is_empty()
                    })
                    && let Some(text) = text
                {
                    self.lua_instance.on_char(&text).unwrap();
                }

                if let Some(key_string) = keycode_as_str(code) {
                    match state {
                        ElementState::Pressed => {
                            self.lua_instance.on_key_down(&key_string, false).unwrap();
                        }
                        ElementState::Released => {
                            self.lua_instance.on_key_up(&key_string).unwrap();
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button_string) = mousebutton_as_str(button) {
                    match state {
                        ElementState::Pressed => {
                            let is_double_click = CONTEXT
                                .with_borrow_mut(|ctx| ctx.input_state.is_double_click(button));
                            self.lua_instance
                                .on_key_down(&button_string, is_double_click)
                                .unwrap();
                        }
                        ElementState::Released => {
                            self.lua_instance.on_key_up(&button_string).unwrap();
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let vertical_scroll_amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(PhysicalPosition { y, .. }) => y,
                };

                if vertical_scroll_amount > 0.0 {
                    self.lua_instance.on_key_up("WHEELUP").unwrap();
                } else if vertical_scroll_amount < 0.0 {
                    self.lua_instance.on_key_up("WHEELDOWN").unwrap();
                }
            }
            _ => {}
        }
    }
}
