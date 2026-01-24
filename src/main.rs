use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use image::io::Reader as ImageReader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use rfd::FileDialog;

// ==========================================
// 1. 数据结构协议 (JSON)
// ==========================================
#[derive(Serialize, Deserialize, Clone)]
struct MapMeta {
    grid_pixel_size: f32,
    offset_x: f32,
    offset_y: f32,
}

#[derive(Serialize, Deserialize, Clone)]
struct LayerData {
    major_z: i32,
    name: String,
    elevation_grid: Vec<Vec<i8>>, 
}

#[derive(Serialize, Deserialize, Clone)]
struct BuildingExport {
    uid: usize,
    name: String,
    grid_x: usize,
    grid_y: usize,
    width: usize,
    height: usize,
}

#[derive(Serialize, Deserialize, Clone)]
struct MapTerrainExport {
    map_name: String,
    meta: MapMeta,
    layers: Vec<LayerData>,
}

#[derive(Serialize, Deserialize, Clone)]
struct MapBuildingsExport {
    map_name: String,
    buildings: Vec<BuildingExport>,
}

#[derive(Deserialize, Clone)]
struct BuildingConfig {
    name: String,
    width: usize,
    height: usize,
    color: [u8; 4],
    icon_path: String,
}

// ==========================================
// 2. 内部逻辑结构
// ==========================================
#[derive(Clone)]
struct BuildingTemplate {
    name: String,
    width: usize,
    height: usize,
    color: Color32,
    icon: Option<TextureHandle>,
}

#[derive(Clone)]
struct PlacedBuilding {
    uid: usize,
    template_name: String,
    grid_x: usize,
    grid_y: usize,
    width: usize,
    height: usize,
    color: Color32,
}

#[derive(PartialEq)]
enum EditMode { Terrain, Building }

// ==========================================
// 3. 编辑器核心状态
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
    brush_radius: i32, 
    zoom: f32,
    pan: Vec2,
    mode: EditMode,
    building_templates: Vec<BuildingTemplate>,
    selected_building_idx: usize,
    placed_buildings: Vec<PlacedBuilding>,
    next_uid: usize,
    map_filename: String,
    building_filename: String,
}

impl MapEditor {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let load_icon = |ctx: &egui::Context, path: &str| -> Option<TextureHandle> {
            if let Ok(img_reader) = ImageReader::open(path) {
                if let Ok(img) = img_reader.decode() {
                    let size = [img.width() as _, img.height() as _];
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.to_rgba8().as_flat_samples().as_slice());
                    return Some(ctx.load_texture(path, color_image, Default::default()));
                }
            }
            None
        };

        let mut templates = Vec::new();
        if let Ok(config_str) = fs::read_to_string("buildings_config.json") {
            if let Ok(configs) = serde_json::from_str::<Vec<BuildingConfig>>(&config_str) {
                for cfg in configs {
                    templates.push(BuildingTemplate {
                        name: cfg.name,
                        width: cfg.width,
                        height: cfg.height,
                        color: Color32::from_rgba_unmultiplied(cfg.color[0], cfg.color[1], cfg.color[2], cfg.color[3]),
                        icon: load_icon(&cc.egui_ctx, &cfg.icon_path),
                    });
                }
            }
        }

        if templates.is_empty() {
            templates.push(BuildingTemplate { name: "Default (1x1)".into(), width: 1, height: 1, color: Color32::GRAY, icon: None });
        }

        let mut editor = Self {
            texture: None,
            grid_size: 32.0,
            offset_x: 0.0,
            offset_y: 0.0,
            grid_rows: 40,
            grid_cols: 40,
            current_major_z: 0,
            layers_data: HashMap::new(),
            current_brush: 0, 
            brush_radius: 0, 
            zoom: 1.0,
            pan: Vec2::ZERO,
            mode: EditMode::Terrain,
            building_templates: templates,
            selected_building_idx: 0,
            placed_buildings: Vec::new(),
            next_uid: 1000,
            map_filename: "terrain_01.json".to_string(),
            building_filename: "strategy_01.json".to_string(),
        };
        editor.layers_data.insert(0, vec![vec![-1; 40]; 40]);
        editor
    }

    fn resize_grids(&mut self) {
        let new_rows = self.grid_rows;
        let new_cols = self.grid_cols;
        for grid in self.layers_data.values_mut() {
            grid.resize(new_rows, vec![-1; new_cols]);
            for row in grid.iter_mut() { row.resize(new_cols, -1); }
        }
    }

    fn pick_and_load_image(&mut self, ctx: &egui::Context) {
        if let Some(path) = FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "bmp"]).pick_file() {
            if let Ok(img_reader) = ImageReader::open(path) {
                if let Ok(img) = img_reader.decode() {
                    let size = [img.width() as _, img.height() as _];
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.to_rgba8().as_flat_samples().as_slice());
                    self.texture = Some(ctx.load_texture("map_image", color_image, Default::default()));
                }
            }
        }
    }

    fn import_terrain(&mut self) {
        if let Some(path) = FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str::<MapTerrainExport>(&content) {
                    self.grid_size = data.meta.grid_pixel_size;
                    self.offset_x = data.meta.offset_x;
                    self.offset_y = data.meta.offset_y;
                    self.layers_data.clear();
                    for layer in data.layers {
                        self.grid_rows = layer.elevation_grid.len();
                        self.grid_cols = layer.elevation_grid[0].len();
                        self.layers_data.insert(layer.major_z, layer.elevation_grid);
                    }
                }
            }
        }
    }

    fn import_buildings(&mut self) {
        if let Some(path) = FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str::<MapBuildingsExport>(&content) {
                    self.placed_buildings = data.buildings.iter().map(|b| {
                        let color = self.building_templates.iter().find(|t| t.name == b.name).map(|t| t.color).unwrap_or(Color32::GRAY);
                        PlacedBuilding { uid: b.uid, template_name: b.name.clone(), grid_x: b.grid_x, grid_y: b.grid_y, width: b.width, height: b.height, color }
                    }).collect();
                    self.next_uid = self.placed_buildings.iter().map(|b| b.uid).max().unwrap_or(1000) + 1;
                }
            }
        }
    }

    fn export_terrain(&self) {
        let meta = MapMeta { grid_pixel_size: self.grid_size, offset_x: self.offset_x, offset_y: self.offset_y };
        let mut layers: Vec<LayerData> = self.layers_data.iter().map(|(&z, grid)| LayerData { major_z: z, name: format!("Major_Layer_{}", z), elevation_grid: grid.clone() }).collect();
        layers.sort_by_key(|l| l.major_z);
        let export_data = MapTerrainExport { map_name: "Ni-Zhan_Map".into(), meta, layers };
        if let Ok(json) = serde_json::to_string_pretty(&export_data) { let _ = fs::write(&self.map_filename, json); }
    }

    fn export_buildings(&self) {
        let b_exp: Vec<BuildingExport> = self.placed_buildings.iter().map(|b| BuildingExport { uid: b.uid, name: b.template_name.clone(), grid_x: b.grid_x, grid_y: b.grid_y, width: b.width, height: b.height }).collect();
        let export_data = MapBuildingsExport { map_name: "Ni-Zhan_Map".into(), buildings: b_exp };
        if let Ok(json) = serde_json::to_string_pretty(&export_data) { let _ = fs::write(&self.building_filename, json); }
    }

    fn can_place_building(&self, start_r: usize, start_c: usize, w: usize, h: usize) -> bool {
        if start_r + h > self.grid_rows || start_c + w > self.grid_cols { return false; }
        let current_grid = self.layers_data.get(&self.current_major_z).unwrap();
        let base_height = current_grid[start_r][start_c];
        if base_height < 0 { return false; }
        for r in start_r..(start_r + h) {
            for c in start_c..(start_c + w) {
                if current_grid[r][c] != base_height { return false; }
            }
        }
        for b in &self.placed_buildings {
            if start_c < b.grid_x + b.width && start_c + w > b.grid_x && start_r < b.grid_y + b.height && start_r + h > b.grid_y { return false; }
        }
        true
    }

    fn get_color(val: i8) -> Color32 {
        match val {
            -1 => Color32::from_rgba_unmultiplied(255, 0, 0, 100),   
             0 => Color32::from_rgba_unmultiplied(0, 255, 0, 40),    
             1 => Color32::from_rgba_unmultiplied(255, 255, 0, 100), 
             2 => Color32::from_rgba_unmultiplied(0, 150, 255, 100), 
             3 => Color32::from_rgba_unmultiplied(150, 0, 255, 100), 
             _ => Color32::TRANSPARENT,
        }
    }
}

impl eframe::App for MapEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("control").min_width(320.0).show(ctx, |ui| {
            ui.heading("MINKE 塔防策略编辑器");
            ui.add_space(5.0);
            if ui.button("载入底图").clicked() { self.pick_and_load_image(ctx); }
            ui.separator();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, EditMode::Terrain, "地形涂抹");
                ui.selectable_value(&mut self.mode, EditMode::Building, "放置建筑");
            });
            ui.separator();

            match self.mode {
                EditMode::Terrain => {
                    ui.group(|ui| {
                        let brushes = [(-1, "障碍"), (0, "平地"), (1, "高台1"), (2, "高台2"), (3, "高台3")];
                        for (val, label) in brushes.iter() {
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut self.current_brush, *val, *label);
                                let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), Sense::hover());
                                ui.painter().rect_filled(rect, 2.0, Self::get_color(*val));
                            });
                        }
                        ui.add(egui::Slider::new(&mut self.brush_radius, 0..=10).text("笔刷大小"));
                    });
                },
                EditMode::Building => {
                    ui.group(|ui| {
                        for (i, t) in self.building_templates.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut self.selected_building_idx, i, "");
                                let (rect, _) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::hover());
                                if let Some(icon) = &t.icon { ui.painter().image(icon.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
                                else { ui.painter().rect_filled(rect, 2.0, t.color); }
                                ui.label(format!("{} ({}x{})", t.name, t.width, t.height));
                            });
                        }
                    });
                    if ui.button("清除所有建筑").clicked() { self.placed_buildings.clear(); }
                }
            }

            ui.add_space(10.0);
            ui.group(|ui| {
                ui.horizontal(|ui| { ui.label("网格大小:"); ui.add(egui::DragValue::new(&mut self.grid_size).speed(0.1)); });
                ui.horizontal(|ui| { ui.label("原点X/Y:"); ui.add(egui::DragValue::new(&mut self.offset_x)); ui.add(egui::DragValue::new(&mut self.offset_y)); });
                ui.horizontal(|ui| { 
                    ui.label("行列:"); 
                    if ui.add(egui::DragValue::new(&mut self.grid_rows)).changed() { self.resize_grids(); }
                    if ui.add(egui::DragValue::new(&mut self.grid_cols)).changed() { self.resize_grids(); }
                });
            });

            ui.add_space(10.0);
            ui.group(|ui| {
                ui.label("地形文件 (.json):");
                ui.horizontal(|ui| { ui.text_edit_singleline(&mut self.map_filename); if ui.button("导出").clicked() { self.export_terrain(); } if ui.button("导入").clicked() { self.import_terrain(); } });
                ui.add_space(5.0);
                ui.label("建筑文件 (.json):");
                ui.horizontal(|ui| { ui.text_edit_singleline(&mut self.building_filename); if ui.button("导出").clicked() { self.export_buildings(); } if ui.button("导入").clicked() { self.import_buildings(); } });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let input = ui.input(|i| i.clone());
            let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let panel_rect = response.rect; 

            if response.hovered() {
                let scroll = input.raw_scroll_delta.y;
                if scroll != 0.0 {
                    let old = self.zoom; self.zoom = (self.zoom * (1.0 + scroll * 0.001)).clamp(0.1, 10.0);
                    if let Some(pos) = input.pointer.hover_pos() { self.pan -= (pos - panel_rect.min - self.pan) * (self.zoom / old - 1.0); }
                }
            }
            if input.pointer.button_down(egui::PointerButton::Middle) { self.pan += input.pointer.delta(); }

            let origin = panel_rect.min + self.pan + Vec2::new(self.offset_x * self.zoom, self.offset_y * self.zoom);
            let z_grid = self.grid_size * self.zoom;

            if let Some(tex) = &self.texture {
                painter.image(tex.id(), Rect::from_min_size(panel_rect.min + self.pan, tex.size_vec2() * self.zoom), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }

            // 核心交互：已修复溢出问题
            if let Some(pos) = input.pointer.hover_pos() {
                let rel = pos - origin; 
                let c = (rel.x / z_grid).floor() as i32;
                let r = (rel.y / z_grid).floor() as i32;

                if r >= 0 && c >= 0 && (r as usize) < self.grid_rows && (c as usize) < self.grid_cols {
                    let (r_idx, c_idx) = (r as usize, c as usize);
                    if self.mode == EditMode::Terrain {
                        let is_p = input.pointer.button_down(egui::PointerButton::Primary);
                        let is_s = input.pointer.button_down(egui::PointerButton::Secondary);
                        if is_p || is_s {
                            let grid = self.layers_data.get_mut(&self.current_major_z).unwrap();
                            let val = if is_p { self.current_brush } else { -1 };
                            let rad = self.brush_radius;
                            
                            // 【修复核心】计算安全的 usize 边界
                            let r_start = (r - rad).max(0) as usize;
                            let r_end = (r + rad).min(self.grid_rows as i32 - 1) as usize;
                            let c_start = (c - rad).max(0) as usize;
                            let c_end = (c + rad).min(self.grid_cols as i32 - 1) as usize;

                            for dr in r_start..=r_end {
                                for dc in c_start..=c_end {
                                    grid[dr][dc] = val;
                                }
                            }
                        }
                    } else if self.mode == EditMode::Building {
                        if response.clicked_by(egui::PointerButton::Primary) {
                            let t = &self.building_templates[self.selected_building_idx];
                            if self.can_place_building(r_idx, c_idx, t.width, t.height) {
                                self.placed_buildings.push(PlacedBuilding { uid: self.next_uid, template_name: t.name.clone(), grid_x: c_idx, grid_y: r_idx, width: t.width, height: t.height, color: t.color });
                                self.next_uid += 1;
                            }
                        } else if response.clicked_by(egui::PointerButton::Secondary) {
                            self.placed_buildings.retain(|b| !(c_idx >= b.grid_x && c_idx < b.grid_x + b.width && r_idx >= b.grid_y && r_idx < b.grid_y + b.height));
                        }
                    }
                }
            }

            // 渲染网格
            let grid = self.layers_data.get(&self.current_major_z).unwrap();
            for r in 0..self.grid_rows {
                for c in 0..self.grid_cols {
                    let rect = Rect::from_min_size(origin + Vec2::new(c as f32 * z_grid, r as f32 * z_grid), Vec2::splat(z_grid)).shrink(0.5);
                    if panel_rect.intersects(rect) { painter.rect_filled(rect, 0.0, Self::get_color(grid[r][c])); }
                }
            }

            // 渲染建筑 (图标/色块)
            for b in &self.placed_buildings {
                let rect = Rect::from_min_size(origin + Vec2::new(b.grid_x as f32 * z_grid, b.grid_y as f32 * z_grid), Vec2::new(b.width as f32 * z_grid, b.height as f32 * z_grid));
                let temp = self.building_templates.iter().find(|t| t.name == b.template_name);
                if let Some(t) = temp {
                    if let Some(icon) = &t.icon { painter.image(icon.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
                    else { painter.rect_filled(rect, 4.0, b.color); }
                }
                painter.rect_stroke(rect, 1.5, Stroke::new(1.5, Color32::from_black_alpha(180)));
            }

            // HUD 及幽灵预览
            if let Some(pos) = input.pointer.hover_pos() {
                let px = (pos.x - panel_rect.min.x - self.pan.x) / self.zoom;
                let py = (pos.y - panel_rect.min.y - self.pan.y) / self.zoom;
                let c = ((pos.x - origin.x) / z_grid).floor() as i32;
                let r = ((pos.y - origin.y) / z_grid).floor() as i32;
                let hud_rect = Rect::from_min_size(panel_rect.min + Vec2::new(10., 10.), Vec2::new(200., 40.));
                painter.rect_filled(hud_rect, 4.0, Color32::from_black_alpha(150));
                painter.text(hud_rect.min + Vec2::new(5., 5.), egui::Align2::LEFT_TOP, format!("Pixel: {:.1}, {:.1}\nGrid: [{}, {}]", px, py, r, c), egui::FontId::proportional(12.0), Color32::WHITE);

                if self.mode == EditMode::Building && r >= 0 && c >= 0 && (r as usize) < self.grid_rows && (c as usize) < self.grid_cols {
                    let t = &self.building_templates[self.selected_building_idx];
                    let ghost = Rect::from_min_size(origin + Vec2::new(c as f32 * z_grid, r as f32 * z_grid), Vec2::new(t.width as f32 * z_grid, t.height as f32 * z_grid));
                    let color = if self.can_place_building(r as usize, c as usize, t.width, t.height) { Color32::GREEN } else { Color32::RED };
                    painter.rect_stroke(ghost, 0.0, Stroke::new(2.5, color));
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([1350.0, 850.0]), ..Default::default() };
    eframe::run_native("MINKE Editor", options, Box::new(|cc| {
        let mut f = egui::FontDefinitions::default();
        if let Ok(d) = fs::read("C:\\Windows\\Fonts\\simhei.ttf") {
            f.font_data.insert("s".into(), egui::FontData::from_owned(d));
            f.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "s".into());
        }
        cc.egui_ctx.set_fonts(f);
        Box::new(MapEditor::new(cc))
    }))
}