use super::{InputState, Params, RenderingBackend};
use anyhow::Result;
use raw_window_handle::HasWindowHandle;
use skia_safe::{
    gpu::{self, backend_render_targets, gl::FramebufferInfo, SurfaceOrigin},
    ColorType, Surface,
};
use std::{ffi::CString, num::NonZeroU32};
use winit::{
    dpi::{LogicalSize, Size},
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext},
    display::{GetGlDisplay, GlDisplay},
    prelude::{GlSurface, NotCurrentGlContext},
    surface::{Surface as GlutinSurface, SurfaceAttributesBuilder, WindowSurface},
};
use glutin_winit::DisplayBuilder;

/// OpenGL rendering backend implementation for Linux.
///
/// This uses glutin to create an OpenGL context/surface and Skia's GL backend to render.
pub struct OpenGlBackend {
    env: Env,
    fb_info: FramebufferInfo,
    num_samples: usize,
    stencil_size: usize,
    input_state: InputState,
}

// Guarantee drop order: Window must be dropped after DirectContext.
// See: https://github.com/rust-skia/rust-skia/issues/476
struct Env {
    surface: Surface,
    gl_surface: GlutinSurface<WindowSurface>,
    gr_context: skia_safe::gpu::DirectContext,
    gl_context: PossiblyCurrentContext,
    window: Window,
}

impl Drop for Env {
    fn drop(&mut self) {
        // Prevent potential driver crashes on teardown.
        self.gr_context.release_resources_and_abandon();
    }
}

impl OpenGlBackend {
    fn create_surface(
        window: &Window,
        fb_info: FramebufferInfo,
        gr_context: &mut skia_safe::gpu::DirectContext,
        num_samples: usize,
        stencil_size: usize,
    ) -> Surface {
        let size = window.inner_size();
        let size = (
            size.width.try_into().expect("Could not convert width"),
            size.height.try_into().expect("Could not convert height"),
        );
        let backend_render_target =
            backend_render_targets::make_gl(size, num_samples, stencil_size, fb_info);

        gpu::surfaces::wrap_backend_render_target(
            gr_context,
            &backend_render_target,
            SurfaceOrigin::BottomLeft,
            ColorType::RGBA8888,
            None,
            None,
        )
        .expect("Could not create skia surface")
    }
}

impl RenderingBackend for OpenGlBackend {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self> {
        use gl::types::GLint;

        let window_attributes = WindowAttributes::default()
            .with_title("Lolite CSS - OpenGL")
            .with_inner_size(Size::new(LogicalSize::new(800, 800)));

        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_transparency(true);

        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));
        let (window, gl_config) = display_builder
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|accum, config| {
                        let transparency_check = config.supports_transparency().unwrap_or(false)
                            & !accum.supports_transparency().unwrap_or(false);

                        if transparency_check || config.num_samples() < accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .unwrap()
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let window = window.expect("Could not create window with OpenGL context");
        let window_handle = window
            .window_handle()
            .expect("Failed to retrieve RawWindowHandle");
        let raw_window_handle = window_handle.as_raw();

        let context_attributes = ContextAttributesBuilder::new().build(Some(raw_window_handle));
        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(Some(raw_window_handle));

        let not_current_gl_context = unsafe {
            gl_config
                .display()
                .create_context(&gl_config, &context_attributes)
                .unwrap_or_else(|_| {
                    gl_config
                        .display()
                        .create_context(&gl_config, &fallback_context_attributes)
                        .expect("failed to create context")
                })
        };

        let (width, height): (u32, u32) = window.inner_size().into();
        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window_handle,
            NonZeroU32::new(width.max(1)).unwrap(),
            NonZeroU32::new(height.max(1)).unwrap(),
        );

        let gl_surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &attrs)
                .expect("Could not create gl window surface")
        };

        let gl_context = not_current_gl_context
            .make_current(&gl_surface)
            .expect("Could not make GL context current when setting up skia renderer");

        gl::load_with(|s| {
            gl_config
                .display()
                .get_proc_address(CString::new(s).unwrap().as_c_str())
        });

        let interface = skia_safe::gpu::gl::Interface::new_load_with(|name| {
            if name == "eglGetCurrentDisplay" {
                return std::ptr::null();
            }
            gl_config
                .display()
                .get_proc_address(CString::new(name).unwrap().as_c_str())
        })
        .ok_or_else(|| anyhow::anyhow!("Could not create Skia GL interface"))?;

        let mut gr_context = skia_safe::gpu::direct_contexts::make_gl(interface, None)
            .ok_or_else(|| anyhow::anyhow!("Could not create Skia GL direct context"))?;

        let fb_info = {
            let mut fboid: GLint = 0;
            unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fboid) };

            FramebufferInfo {
                fboid: fboid.try_into().unwrap(),
                format: skia_safe::gpu::gl::Format::RGBA8.into(),
                ..Default::default()
            }
        };

        let num_samples = gl_config.num_samples() as usize;
        let stencil_size = gl_config.stencil_size() as usize;
        let surface =
            Self::create_surface(&window, fb_info, &mut gr_context, num_samples, stencil_size);

        Ok(Self {
            env: Env {
                surface,
                gl_surface,
                gr_context,
                gl_context,
                window,
            },
            fb_info,
            num_samples,
            stencil_size,
            input_state: InputState::default(),
        })
    }

    fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::Resized(physical_size) => {
                let (width, height): (u32, u32) = (*physical_size).into();

                self.env.gl_surface.resize(
                    &self.env.gl_context,
                    NonZeroU32::new(width.max(1)).unwrap(),
                    NonZeroU32::new(height.max(1)).unwrap(),
                );

                self.env.surface = Self::create_surface(
                    &self.env.window,
                    self.fb_info,
                    &mut self.env.gr_context,
                    self.num_samples,
                    self.stencil_size,
                );
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, params: &mut Params) {
        (params.on_draw)(self.env.surface.canvas());
        self.env.gr_context.flush_and_submit();
        let _ = self.env.gl_surface.swap_buffers(&self.env.gl_context);
    }

    fn input_state_mut(&mut self) -> &mut InputState {
        &mut self.input_state
    }

    fn input_state(&self) -> &InputState {
        &self.input_state
    }

    fn request_redraw(&self) {
        self.env.window.request_redraw();
    }
}
