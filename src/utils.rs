use eframe::egui::Color32;

pub fn get_time_value(wave: i32, late: bool) -> i32 {
    wave * 2 + if late { 1 } else { 0 }
}

pub fn fix_path(p: &str) -> String {
    if p.starts_with("maps/") { p.to_string() }
    else { format!("maps/{}", p) }
}

pub fn get_layer_color(val: i8) -> Color32 {
    match val {
        -1 => Color32::from_rgba_unmultiplied(255, 0, 0, 100),   
         0 => Color32::from_rgba_unmultiplied(0, 255, 0, 40),    
         1 => Color32::from_rgba_unmultiplied(255, 255, 0, 100), 
         2 => Color32::from_rgba_unmultiplied(0, 150, 255, 100), 
         3 => Color32::from_rgba_unmultiplied(150, 0, 255, 100), 
         _ => Color32::TRANSPARENT,
    }
}