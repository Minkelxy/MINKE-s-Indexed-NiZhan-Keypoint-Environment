use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use image::io::Reader as ImageReader;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;

// ==========================================
// 1. JSON 导出专用数据结构
// ==========================================
#[derive(Serialize)]
struct MapMeta {
    grid_pixel_size: f32,
    offset_x: f32,
    offset_y: f32,
}

#[derive(Serialize)]
struct LayerData {
    major_z: i32,
    name: String,
    elevation_grid: Vec<Vec<i8>>, 
}

#[derive(Serialize)]
struct MapExportData {
    map_name: String,
    meta: MapMeta,
    layers: Vec<LayerData>,
}

// ==========================================
// 2. 主函数启动
// ==========================================
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 850.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MINKE's Indexed Ni-Zhan Keypoint Environment",
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            if let Ok(font_data) = fs::read("C:\\Windows\\Fonts\\simhei.ttf") {
                fonts.font_data.insert("system_font".to_owned(), egui::FontData::from_owned(font_data));
                fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "system_font".to_owned());
                fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().insert(0, "system_font".to_owned());
            }
            cc.egui_ctx.set_fonts(fonts);
            Box::new(MapEditor::new(cc))
        }),
    )
}

// ==========================================
// 3. 编辑器核心状态结构
// ==========================================
struct MapEditor {
    texture: Option<TextureHandle>,
    grid_size: f32,
    offset_x: f32,
    offset_y: f32,
    grid_rows: usize,
    grid_cols: usize,
    current_major_z: i32,
    layers_data: HashMap<i32, Vec<Vec<i8>>>, 
    current_brush: i8,
    zoom: f32,
    pan: Vec2,
}

impl MapEditor {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let initial_rows = 40;
        let initial_cols = 40;
        let mut editor = Self {
            texture: None,
            grid_size: 32.0,
            offset_x: 0.0,
            offset_y: 0.0,
            grid_rows: initial_rows,
            grid_cols: initial_cols,
            current_major_z: 0,
            layers_data: HashMap::new(),
            current_brush: -1, 
            zoom: 1.0,
            pan: Vec2::ZERO,
        };
        editor.layers_data.insert(0, vec![vec![0; initial_cols]; initial_rows]);
        editor.load_image(&cc.egui_ctx, "1.png"); 
        editor
    }

    fn load_image(&mut self, ctx: &egui::Context, path: &str) {
        if let Ok(img) = ImageReader::open(path) {
            if let Ok(decoded) = img.decode() {
                let size = [decoded.width() as _, decoded.height() as _];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.to_rgba8().as_flat_samples().as_slice());
                self.texture = Some(ctx.load_texture("map_image", color_image, Default::default()));
            }
        }
    }

    fn resize_grids(&mut self) {
        let new_rows = self.grid_rows;
        let new_cols = self.grid_cols;
        for grid in self.layers_data.values_mut() {
            grid.resize(new_rows, vec![0; new_cols]);
            for row in grid.iter_mut() {
                row.resize(new_cols, 0); 
            }
        }
    }

    fn get_color(val: i8) -> Color32 {
        match val {
            -1 => Color32::from_rgba_unmultiplied(255, 0, 0, 100),   // 障碍：红
             0 => Color32::from_rgba_unmultiplied(0, 255, 0, 60),    // 高度0：绿
             1 => Color32::from_rgba_unmultiplied(255, 255, 0, 100), // 高度1：黄
             2 => Color32::from_rgba_unmultiplied(0, 150, 255, 100), // 高度2：蓝
             3 => Color32::from_rgba_unmultiplied(150, 0, 255, 100), // 高度3：紫
             _ => Color32::from_rgba_unmultiplied(255, 255, 255, 50),
        }
    }

    fn screen_to_canvas(&self, screen_pos: Pos2, rect_min: Pos2) -> Pos2 {
        let rel = screen_pos - rect_min - self.pan;
        Pos2::new(rel.x / self.zoom, rel.y / self.zoom)
    }

    fn canvas_to_screen(&self, canvas_pos: Pos2, rect_min: Pos2) -> Pos2 {
        rect_min + self.pan + Vec2::new(canvas_pos.x * self.zoom, canvas_pos.y * self.zoom)
    }

    fn export_to_json(&self) {
        let meta = MapMeta { grid_pixel_size: self.grid_size, offset_x: self.offset_x, offset_y: self.offset_y };
        let mut layers: Vec<LayerData> = self.layers_data.iter().map(|(&z, grid)| LayerData {
            major_z: z,
            name: format!("Major_Layer_{}", z),
            elevation_grid: grid.clone(),
        }).collect();
        layers.sort_by_key(|l| l.major_z);

        let export_data = MapExportData { map_name: "Ni-Zhan_Exported_Map".to_string(), meta, layers };
        if let Ok(json) = serde_json::to_string_pretty(&export_data) {
            if let Err(e) = fs::write("minke_map_data.json", json) { eprintln!("导出失败: {}", e); } 
            else { println!("✅ 导出成功!"); }
        }
    }
}

// ==========================================
// 4. GUI 渲染循环
// ==========================================
impl eframe::App for MapEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        
        egui::SidePanel::left("control_panel").min_width(280.0).show(ctx, |ui| {
            ui.heading("MINKE 地图数据化引擎");
            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label("🛠 基础网格设定");
                ui.horizontal(|ui| { ui.label("网格大小:"); ui.add(egui::DragValue::new(&mut self.grid_size).speed(0.1)); });
                ui.horizontal(|ui| { ui.label("原点 X :"); ui.add(egui::DragValue::new(&mut self.offset_x).speed(0.5)); });
                ui.horizontal(|ui| { ui.label("原点 Y :"); ui.add(egui::DragValue::new(&mut self.offset_y).speed(0.5)); });
                
                ui.separator();
                ui.label("📏 动态调整行列数");
                ui.horizontal(|ui| {
                    ui.label("行数 (H):");
                    if ui.add(egui::DragValue::new(&mut self.grid_rows).speed(1).clamp_range(1..=1000)).changed() { self.resize_grids(); }
                });
                ui.horizontal(|ui| {
                    ui.label("列数 (W):");
                    if ui.add(egui::DragValue::new(&mut self.grid_cols).speed(1).clamp_range(1..=1000)).changed() { self.resize_grids(); }
                });
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("当前大层级 (Major Z):");
                ui.add(egui::DragValue::new(&mut self.current_major_z).speed(1));
            });

            if !self.layers_data.contains_key(&self.current_major_z) {
                self.layers_data.insert(self.current_major_z, vec![vec![0; self.grid_cols]; self.grid_rows]);
            }

            ui.add_space(10.0);
            
            // --- 修复点：正确使用 ui.horizontal 和 radio_value ---
            ui.group(|ui| {
                ui.heading("🖌️ 画笔工具 (选择海拔)");
                ui.label("选定画笔后，左键涂抹。右键=橡皮擦(变绿)");
                ui.add_space(5.0);

                let brushes = [
                    (-1, "🔴 障碍物 (-1)"),
                    (0, "🟢 基础平地 (0)"),
                    (1, "🟡 高台/土坡 (1)"),
                    (2, "🔵 二层/人造台 (2)"),
                    (3, "🟣 塔顶/最高点 (3)"),
                ];

                for (val, label) in brushes.iter() {
                    // 使用 horizontal 将单选框、颜色块、文字排在同一行
                    ui.horizontal(|ui| {
                        // radio_value 会自动处理状态比对和更新
                        ui.radio_value(&mut self.current_brush, *val, "");
                        
                        // 绘制颜色块
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(15.0, 15.0), Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, Self::get_color(*val));
                        
                        // 绘制标签
                        ui.label(*label);
                    });
                }
            });

            ui.add_space(20.0);
            if ui.add_sized([ui.available_width(), 40.0], egui::Button::new("💾 导出为 JSON 文件").fill(Color32::from_rgb(0, 120, 0))).clicked() {
                self.export_to_json();
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let input = ui.input(|i| i.clone());
            let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let rect_min = response.rect.min;

            if response.hovered() {
                let scroll_delta = input.raw_scroll_delta.y;
                if scroll_delta != 0.0 {
                    let old_zoom = self.zoom;
                    self.zoom *= 1.0 + (scroll_delta * 0.001);
                    self.zoom = self.zoom.clamp(0.1, 10.0);
                    if let Some(mouse_pos) = input.pointer.hover_pos() {
                        let rel_mouse = mouse_pos - rect_min - self.pan;
                        self.pan -= rel_mouse * (self.zoom / old_zoom - 1.0);
                    }
                }
            }
            if input.pointer.button_down(egui::PointerButton::Middle) { self.pan += input.pointer.delta(); }

            if let Some(texture) = &self.texture {
                let img_size = Vec2::new(texture.size()[0] as f32, texture.size()[1] as f32);
                let img_screen_min = self.canvas_to_screen(Pos2::ZERO, rect_min);
                painter.image(texture.id(), Rect::from_min_size(img_screen_min, img_size * self.zoom), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }

            let canvas_origin = Pos2::new(self.offset_x, self.offset_y);

            if response.dragged() || response.clicked() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let is_primary = input.pointer.button_down(egui::PointerButton::Primary);
                    let is_secondary = input.pointer.button_down(egui::PointerButton::Secondary);
                    
                    if is_primary || is_secondary {
                        let canvas_pos = self.screen_to_canvas(pointer_pos, rect_min);
                        let rel_x = canvas_pos.x - canvas_origin.x;
                        let rel_y = canvas_pos.y - canvas_origin.y;

                        if rel_x >= 0.0 && rel_y >= 0.0 {
                            let col = (rel_x / self.grid_size).floor() as usize;
                            let row = (rel_y / self.grid_size).floor() as usize;

                            if row < self.grid_rows && col < self.grid_cols {
                                let grid = self.layers_data.get_mut(&self.current_major_z).unwrap();
                                if is_primary {
                                    grid[row][col] = self.current_brush; 
                                } else {
                                    grid[row][col] = 0; 
                                }
                            }
                        }
                    }
                }
            }

            let screen_origin = self.canvas_to_screen(canvas_origin, rect_min);
            let zoomed_grid_size = self.grid_size * self.zoom;
            let current_grid = self.layers_data.get(&self.current_major_z).unwrap();

            for r in 0..self.grid_rows {
                for c in 0..self.grid_cols {
                    let cell_screen_pos = screen_origin + Vec2::new(c as f32 * zoomed_grid_size, r as f32 * zoomed_grid_size);
                    let cell_rect = Rect::from_min_size(cell_screen_pos, Vec2::new(zoomed_grid_size, zoomed_grid_size));

                    if response.rect.intersects(cell_rect) {
                        let val = current_grid[r][c];
                        painter.rect_filled(cell_rect, 0.0, Self::get_color(val));
                    }
                }
            }

            let stroke = Stroke::new(1.0, Color32::from_white_alpha(50));
            for r in 0..=self.grid_rows {
                let y = screen_origin.y + r as f32 * zoomed_grid_size;
                painter.line_segment([Pos2::new(screen_origin.x, y), Pos2::new(screen_origin.x + self.grid_cols as f32 * zoomed_grid_size, y)], stroke);
            }
            for c in 0..=self.grid_cols {
                let x = screen_origin.x + c as f32 * zoomed_grid_size;
                painter.line_segment([Pos2::new(x, screen_origin.y), Pos2::new(x, screen_origin.y + self.grid_rows as f32 * zoomed_grid_size)], stroke);
            }
        });
    }
}