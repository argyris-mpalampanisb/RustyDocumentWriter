#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod app;

fn main() -> eframe::Result<()> {
    let open = std::env::args().nth(1).map(std::path::PathBuf::from);
    let icon = {
        // A simple generated window icon (violet "page").
        let mut rgba = vec![0u8; 32 * 32 * 4];
        for y in 0..32 {
            for x in 0..32 {
                let i = (y * 32 + x) * 4;
                let inside = (3..29).contains(&x) && (2..30).contains(&y);
                if inside {
                    let right = x >= 16;
                    rgba[i..i + 4].copy_from_slice(if right { &[0x8E, 0x74, 0xF0, 255] } else { &[0x6B, 0x4F, 0xD8, 255] });
                }
            }
        }
        egui::IconData { rgba, width: 32, height: 32 }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RustyDocumentWriter")
            .with_inner_size([1280.0, 820.0])
            .with_maximized(true)
            .with_min_inner_size([760.0, 480.0])
            .with_decorations(false)
            .with_resizable(true)
            .with_icon(std::sync::Arc::new(icon))
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("RustyDocumentWriter", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, open)))))
}
