mod app;
mod panels;

use eframe::egui;
use eframe::egui_wgpu::{self, WgpuSetup, wgpu};

fn main() -> eframe::Result {
    env_logger::init();

    let mut wgpu_options = egui_wgpu::WgpuConfiguration::default();
    if let WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = preferred_backends();
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AIA Studio")
            .with_inner_size([1400.0, 860.0])
            .with_min_inner_size([800.0, 500.0])
            .with_maximized(true),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    };
    eframe::run_native(
        "AIA Studio",
        options,
        Box::new(|cc| Ok(Box::new(app::AiaApp::new(cc)))),
    )
}

/// En Windows se usa DirectX 12: algunos drivers Vulkan (por ejemplo Intel UHD)
/// se cuelgan al compilar los shaders de vello. `WGPU_BACKEND` permite forzar otro.
fn preferred_backends() -> wgpu::Backends {
    wgpu::Backends::from_env().unwrap_or(if cfg!(windows) {
        wgpu::Backends::DX12
    } else {
        wgpu::Backends::PRIMARY
    })
}
