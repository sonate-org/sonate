use super::{InputState, Params, RenderingBackend};
use anyhow::Result;
use std::cell::RefCell;
use winit::{
    dpi::{LogicalSize, Size},
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

#[cfg(all(target_os = "windows"))]
use windows::{
    core::Interface,
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_11_0,
            Direct3D12::{D3D12CreateDevice, ID3D12Device, D3D12_RESOURCE_STATE_COMMON},
            Dxgi::{
                Common::{
                    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
                    DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
                },
                CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory4, IDXGISwapChain3,
                DXGI_ADAPTER_FLAG, DXGI_ADAPTER_FLAG_NONE, DXGI_ADAPTER_FLAG_SOFTWARE,
                DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_DISCARD,
                DXGI_USAGE_RENDER_TARGET_OUTPUT,
            },
        },
    },
};

#[cfg(all(target_os = "windows"))]
use skia_safe::{
    gpu::{
        d3d::{BackendContext, TextureResourceInfo},
        surfaces, BackendRenderTarget, DirectContext, Protected, SurfaceOrigin,
    },
    ColorType, Surface,
};

const BUFFER_COUNT: usize = 2;

/// Direct3D 12 rendering backend implementation
#[cfg(all(target_os = "windows"))]
pub struct D3D12Backend<'a> {
    window: Window,
    swap_chain: IDXGISwapChain3,
    direct_context: DirectContext,
    surfaces: [(Surface, BackendRenderTarget); BUFFER_COUNT],
    input_state: InputState,
    params: &'a RefCell<Params>,
}

#[cfg(all(target_os = "windows"))]
impl<'a> RenderingBackend<'a> for D3D12Backend<'a> {
    fn new(event_loop: &ActiveEventLoop, params: &'a RefCell<Params>) -> Result<Self> {
        let mut window_attributes = WindowAttributes::default();
        window_attributes.inner_size = Some(Size::new(LogicalSize::new(800, 800)));
        window_attributes.title = "Lolite CSS - Direct3D 12".into();

        let window = event_loop
            .create_window(window_attributes)
            .expect("Failed to create window");

        let hwnd = HWND(u64::from(window.id()) as *mut _);
        let (width, height) = window.inner_size().into();

        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory1() }?;
        let (adapter, device) = get_hardware_adapter_and_device(&factory)?;
        let queue = unsafe { device.CreateCommandQueue(&Default::default()) }?;

        let backend_context = BackendContext {
            adapter,
            device,
            queue,
            memory_allocator: None,
            protected_context: Protected::No,
        };
        let mut direct_context = unsafe { DirectContext::new_d3d(&backend_context, None) }.unwrap();

        let swap_chain: IDXGISwapChain3 = unsafe {
            factory.CreateSwapChainForHwnd(
                &backend_context.queue,
                hwnd,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: width,
                    Height: height,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: BUFFER_COUNT as _,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    ..Default::default()
                },
                None,
                None,
            )
        }?
        .cast()?;

        let surfaces: [_; BUFFER_COUNT] = std::array::from_fn(|i| {
            let resource = unsafe { swap_chain.GetBuffer(i as u32).unwrap() };

            let backend_render_target = BackendRenderTarget::new_d3d(
                window.inner_size().into(),
                &TextureResourceInfo {
                    resource,
                    alloc: None,
                    resource_state: D3D12_RESOURCE_STATE_COMMON,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    sample_count: 1,
                    level_count: 0,
                    sample_quality_pattern: DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
                    protected: Protected::No,
                },
            );

            let surface = surfaces::wrap_backend_render_target(
                &mut direct_context,
                &backend_render_target,
                SurfaceOrigin::TopLeft,
                ColorType::RGBA8888,
                None,
                None,
            )
            .unwrap();

            (surface, backend_render_target)
        });

        println!(
            "D3D12 backend initialized with {} surfaces.",
            surfaces.len()
        );

        let input_state = InputState::default();

        Ok(Self {
            window,
            swap_chain,
            direct_context,
            surfaces,
            input_state,
            params,
        })
    }

    fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            // D3D12-specific window events can be handled here
            // For now, we don't handle any backend-specific events
            _ => false, // Event not handled by this backend
        }
    }

    fn render(&mut self) {
        let index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() };
        let (surface, _) = &mut self.surfaces[index as usize];
        let canvas = surface.canvas();

        // Call the user's on_draw callback for all drawing
        (self.params.borrow_mut().on_draw)(canvas);

        self.direct_context.flush_and_submit_surface(surface, None);

        unsafe { self.swap_chain.Present(1, DXGI_PRESENT::default()) }.unwrap();

        // NOTE: If you get some error when you render, you can check it with:
        // unsafe {
        //     device.GetDeviceRemovedReason().ok().unwrap();
        // }
    }

    fn input_state_mut(&mut self) -> &mut InputState {
        &mut self.input_state
    }

    fn input_state(&self) -> &InputState {
        &self.input_state
    }

    fn params(&self) -> &'a RefCell<Params> {
        self.params
    }

    fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

#[cfg(all(target_os = "windows"))]
fn get_hardware_adapter_and_device(
    factory: &IDXGIFactory4,
) -> windows::core::Result<(IDXGIAdapter1, ID3D12Device)> {
    for i in 0.. {
        let adapter = unsafe { factory.EnumAdapters1(i) }?;

        let adapter_desc = unsafe { adapter.GetDesc1() }?;

        if (DXGI_ADAPTER_FLAG(adapter_desc.Flags as _) & DXGI_ADAPTER_FLAG_SOFTWARE)
            != DXGI_ADAPTER_FLAG_NONE
        {
            continue; // Don't select the Basic Render Driver adapter.
        }

        let mut device = None;
        if unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }.is_ok() {
            return Ok((adapter, device.unwrap()));
        }
    }
    unreachable!()
}

// Stub implementation for non-Windows platforms
#[cfg(not(all(target_os = "windows")))]
pub struct D3D12Backend<'a> {
    _phantom: std::marker::PhantomData<&'a ()>,
}

#[cfg(not(all(target_os = "windows")))]
impl<'a> RenderingBackend<'a> for D3D12Backend<'a> {
    fn new(_event_loop: &ActiveEventLoop, _params: &'a RefCell<Params>) -> Result<Self> {
        anyhow::bail!("D3D12 backend is not available on this platform");
    }

    fn handle_window_event(&mut self, _event: &WindowEvent) -> bool {
        false
    }

    fn render(&mut self) {}

    fn input_state_mut(&mut self) -> &mut InputState {
        unreachable!("D3D12 backend is not available on this platform")
    }

    fn input_state(&self) -> &InputState {
        unreachable!("D3D12 backend is not available on this platform")
    }

    fn params(&self) -> &'a RefCell<Params> {
        unreachable!("D3D12 backend is not available on this platform")
    }

    fn request_redraw(&self) {
        unreachable!("D3D12 backend is not available on this platform")
    }
}
