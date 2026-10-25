// In release no console opens next to the window (in debug it does, to see the logs).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod chat_panel;
mod modes;

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
            .with_min_inner_size([800.0, 500.0]),
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

/// On Windows DirectX 12 is used: some Vulkan drivers (for example Intel UHD) crash
/// while compiling vello's shaders. `WGPU_BACKEND` forces another one.
fn preferred_backends() -> wgpu::Backends {
    wgpu::Backends::from_env().unwrap_or(if cfg!(windows) {
        wgpu::Backends::DX12
    } else {
        wgpu::Backends::PRIMARY
    })
}
