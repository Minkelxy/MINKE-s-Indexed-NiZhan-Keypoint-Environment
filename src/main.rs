#![windows_subsystem = "windows"]

mod models;
mod utils;
mod app;

use app::MapEditor;
use eframe::egui;
use std::fs;

fn main() -> eframe::Result<()> {
    println!("--- MINKE Strategy Editor Starting ---");

    let options = eframe::NativeOptions { 
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1350.0, 850.0])
            .with_drag_and_drop(true),
        ..Default::default() 
    };
    
    eframe::run_native("MINKE Editor", options, Box::new(|cc| {
        println!("[System] Graphics initialized.");
        
        let mut f = egui::FontDefinitions::default();
        println!("[System] Loading fonts...");
        if let Ok(d) = fs::read("C:\\Windows\\Fonts\\simhei.ttf") {
            f.font_data.insert("s".into(), egui::FontData::from_owned(d));
            f.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "s".into());
            println!("[System] Font SimHei.ttf loaded successfully.");
        } else {
            println!("[System] [WARN] SimHei.ttf not found.");
        }
        cc.egui_ctx.set_fonts(f);

        println!("[System] Constructing MapEditor...");
        let editor = MapEditor::new(cc);
        println!("[System] Logic ready, displaying window.");
        
        Box::new(editor)
    }))
}