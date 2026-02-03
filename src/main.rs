#![windows_subsystem = "windows"]
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use image::io::Reader as ImageReader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use rfd::FileDialog;

// ==========================================
// 1. 数据结构协议
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
    wave_num: i32,
    is_late: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct UpgradeEvent {
    building_name: String, 
    wave_num: i32,
    is_late: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct DemolishEvent {
    uid: usize,          
    name: String,
    grid_x: usize,
    grid_y: usize,
    width: usize,
    height: usize,
    wave_num: i32,
    is_late: bool,
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
    #[serde(default)]
    upgrades: Vec<UpgradeEvent>,
    #[serde(default)]
    demolishes: Vec<DemolishEvent>, 
}

#[derive(Deserialize, Clone)]
struct BuildingConfig {
    name: String,
    width: usize,
    height: usize,
    color: [u8; 4],
    icon_path: String,
}

#[derive(Deserialize, Clone)]
struct MapPreset {
    name: String,
    image_path: String,
    terrain_path: String,
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
    wave_num: i32,
    is_late: bool,
}

#[derive(PartialEq, Debug)]
enum EditMode { Terrain, Building, Demolish }

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
    selected_upgrade_target_idx: usize, 
    // [Fix] 移除了 selected_demolish_target_idx

    placed_buildings: Vec<PlacedBuilding>,
    next_uid: usize,
    map_filename: String,
    building_filename: String,
    presets: Vec<MapPreset>,
    current_wave_num: i32,
    current_is_late: bool,
    upgrade_events: Vec<UpgradeEvent>,
    demolish_events: Vec<DemolishEvent>,
}

impl MapEditor {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let fix_path = |p: &str| -> String {
            if p.starts_with("maps/") { p.to_string() }
            else { format!("maps/{}", p) }
        };

        let load_icon = |ctx: &egui::Context, path: &str| -> Option<TextureHandle> {
            let full_path = fix_path(path);
            if let Ok(img_reader) = ImageReader::open(&full_path) {
                if let Ok(img) = img_reader.decode() {
                    let size = [img.width() as _, img.height() as _];
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.to_rgba8().as_flat_samples().as_slice());
                    return Some(ctx.load_texture(&full_path, color_image, Default::default()));
                }
            }
            None
        };

        let mut b_templates = Vec::new();
        if let Ok(config_str) = fs::read_to_string("maps/buildings_config.json") {
            if let Ok(configs) = serde_json::from_str::<Vec<BuildingConfig>>(&config_str) {
                for cfg in configs {
                    b_templates.push(BuildingTemplate {
                        name: cfg.name,
                        width: cfg.width,
                        height: cfg.height,
                        color: Color32::from_rgba_unmultiplied(cfg.color[0], cfg.color[1], cfg.color[2], cfg.color[3]),
                        icon: load_icon(&cc.egui_ctx, &cfg.icon_path),
                    });
                }
            }
        }
        if b_templates.is_empty() {
            b_templates.push(BuildingTemplate { name: "Default (1x1)".into(), width: 1, height: 1, color: Color32::GRAY, icon: None });
        }

        let mut map_presets = Vec::new();
        if let Ok(pre_str) = fs::read_to_string("maps/map_presets.json") {
            if let Ok(presets) = serde_json::from_str::<Vec<MapPreset>>(&pre_str) {
                map_presets = presets;
            }
        }

        let mut editor = Self {
            texture: None, grid_size: 32.0, offset_x: 0.0, offset_y: 0.0,
            grid_rows: 40, grid_cols: 40, current_major_z: 0,
            layers_data: HashMap::new(), current_brush: 0, brush_radius: 0,
            zoom: 1.0, pan: Vec2::ZERO, mode: EditMode::Terrain,
            building_templates: b_templates,
            selected_building_idx: 0,
            selected_upgrade_target_idx: 0,
            // [Fix] 移除了初始化
            placed_buildings: Vec::new(),
            next_uid: 1000,
            map_filename: "terrain_01.json".to_string(),
            building_filename: "strategy_01.json".to_string(),
            presets: map_presets,
            current_wave_num: 1, current_is_late: false,
            upgrade_events: Vec::new(),
            demolish_events: Vec::new(),
        };
        editor.layers_data.insert(0, vec![vec![-1; 40]; 40]);
        editor
    }

    fn apply_preset(&mut self, ctx: &egui::Context, preset: &MapPreset) {
        let fix_path = |p: &str| -> String { if p.starts_with("maps/") { p.to_string() } else { format!("maps/{}", p) } };
        let image_p = fix_path(&preset.image_path);
        let terrain_p = fix_path(&preset.terrain_path);

        if let Ok(img_reader) = ImageReader::open(&image_p) {
            if let Ok(img) = img_reader.decode() {
                let size = [img.width() as _, img.height() as _];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.to_rgba8().as_flat_samples().as_slice());
                self.texture = Some(ctx.load_texture(&image_p, color_image, Default::default()));
            }
        }
        if let Ok(content) = fs::read_to_string(&terrain_p) {
            if let Ok(data) = serde_json::from_str::<MapTerrainExport>(&content) {
                self.grid_size = data.meta.grid_pixel_size; self.offset_x = data.meta.offset_x; self.offset_y = data.meta.offset_y;
                self.layers_data.clear();
                for layer in data.layers {
                    self.grid_rows = layer.elevation_grid.len(); self.grid_cols = layer.elevation_grid[0].len();
                    self.layers_data.insert(layer.major_z, layer.elevation_grid);
                }
                self.map_filename = Path::new(&terrain_p).file_name().unwrap().to_string_lossy().into();
            }
        }
    }

    fn resize_grids(&mut self) {
        for grid in self.layers_data.values_mut() {
            grid.resize(self.grid_rows, vec![-1; self.grid_cols]);
            for row in grid.iter_mut() { row.resize(self.grid_cols, -1); }
        }
    }

    fn pick_and_load_image(&mut self, ctx: &egui::Context) {
        if let Some(path) = FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "bmp"]).pick_file() {
            if let Ok(img_reader) = ImageReader::open(&path) {
                if let Ok(img) = img_reader.decode() {
                    let size = [img.width() as _, img.height() as _];
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.to_rgba8().as_flat_samples().as_slice());
                    self.texture = Some(ctx.load_texture(path.to_string_lossy(), color_image, Default::default()));
                }
            }
        }
    }

    fn import_terrain(&mut self) {
        if let Some(path) = FileDialog::new().set_directory("output").add_filter("JSON", &["json"]).pick_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str::<MapTerrainExport>(&content) {
                    self.grid_size = data.meta.grid_pixel_size; self.offset_x = data.meta.offset_x; self.offset_y = data.meta.offset_y;
                    self.layers_data.clear();
                    for layer in data.layers {
                        self.grid_rows = layer.elevation_grid.len(); self.grid_cols = layer.elevation_grid[0].len();
                        self.layers_data.insert(layer.major_z, layer.elevation_grid);
                    }
                }
            }
        }
    }

    fn import_buildings(&mut self) {
        if let Some(path) = FileDialog::new().set_directory("output").add_filter("JSON", &["json"]).pick_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str::<MapBuildingsExport>(&content) {
                    self.placed_buildings = data.buildings.iter().map(|b| {
                        let color = self.building_templates.iter().find(|t| t.name == b.name).map(|t| t.color).unwrap_or(Color32::GRAY);
                        PlacedBuilding { 
                            uid: b.uid, template_name: b.name.clone(), grid_x: b.grid_x, grid_y: b.grid_y, 
                            width: b.width, height: b.height, color, wave_num: b.wave_num, is_late: b.is_late
                        }
                    }).collect();
                    self.next_uid = self.placed_buildings.iter().map(|b| b.uid).max().unwrap_or(1000) + 1;
                    self.upgrade_events = data.upgrades;
                    self.demolish_events = data.demolishes; 
                }
            }
        }
    }

    fn export_terrain(&self) {
        let _ = fs::create_dir_all("output");
        let meta = MapMeta { grid_pixel_size: self.grid_size, offset_x: self.offset_x, offset_y: self.offset_y };
        let mut layers: Vec<LayerData> = self.layers_data.iter().map(|(&z, grid)| LayerData { major_z: z, name: format!("Layer_{}", z), elevation_grid: grid.clone() }).collect();
        layers.sort_by_key(|l| l.major_z);
        let out_path = PathBuf::from("output").join(&self.map_filename);
        if let Ok(json) = serde_json::to_string_pretty(&MapTerrainExport { map_name: "Ni-Zhan_Map".into(), meta, layers }) { let _ = fs::write(out_path, json); }
    }

    fn export_buildings(&self) {
        let _ = fs::create_dir_all("output");
        let b_exp: Vec<BuildingExport> = self.placed_buildings.iter().map(|b| BuildingExport { 
            uid: b.uid, name: b.template_name.clone(), grid_x: b.grid_x, grid_y: b.grid_y, 
            width: b.width, height: b.height, wave_num: b.wave_num, is_late: b.is_late 
        }).collect();
        let out_path = PathBuf::from("output").join(&self.building_filename);
        let export_data = MapBuildingsExport { 
            map_name: "Ni-Zhan_Map".into(), buildings: b_exp, 
            upgrades: self.upgrade_events.clone(),
            demolishes: self.demolish_events.clone(), 
        };
        if let Ok(json) = serde_json::to_string_pretty(&export_data) { let _ = fs::write(out_path, json); }
    }

    fn get_time_value(wave: i32, late: bool) -> i32 {
        wave * 2 + if late { 1 } else { 0 }
    }

    fn get_building_demolish_time(&self, uid: usize) -> i32 {
        self.demolish_events.iter()
            .find(|d| d.uid == uid)
            .map(|d| Self::get_time_value(d.wave_num, d.is_late))
            .unwrap_or(i32::MAX)
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

        let t_current = Self::get_time_value(self.current_wave_num, self.current_is_late);

        for b in &self.placed_buildings {
            if start_c < b.grid_x + b.width && start_c + w > b.grid_x && 
               start_r < b.grid_y + b.height && start_r + h > b.grid_y {
                
                let t_create = Self::get_time_value(b.wave_num, b.is_late);
                let t_demolish = self.get_building_demolish_time(b.uid);

                if t_current >= t_create && t_current < t_demolish {
                    return false;
                }
            }
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
        egui::SidePanel::left("control").min_width(340.0).show(ctx, |ui| {
            ui.heading("🚀 MINKE 策略操作序列编辑器");
            ui.add_space(5.0);
            ui.group(|ui| {
                ui.label("加载关卡预设 (来自 maps/):");
                for (i, preset) in self.presets.clone().iter().enumerate() {
                    ui.push_id(i, |ui| { if ui.button(format!("载入: {}", preset.name)).clicked() { self.apply_preset(ctx, preset); } });
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, EditMode::Terrain, "地形涂抹");
                ui.selectable_value(&mut self.mode, EditMode::Building, "布局/建设");
                ui.selectable_value(&mut self.mode, EditMode::Demolish, "标记拆除");
            });
            ui.add_space(5.0);
            ui.group(|ui| {
                ui.label("⚙️ 操作序列时间设置:");
                ui.horizontal(|ui| {
                    ui.label("当前波次:");
                    ui.add(egui::DragValue::new(&mut self.current_wave_num).speed(1).clamp_range(1..=100));
                });
                ui.checkbox(&mut self.current_is_late, "本波次后期 (is_late)");
            });

            if self.mode == EditMode::Terrain {
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
            } else if self.mode == EditMode::Building {
                ui.push_id("upgrade_panel", |ui| {
                    ui.collapsing("⬆️ 升级任务序列", |ui| {
                        egui::ComboBox::from_label("目标塔")
                            .selected_text(&self.building_templates[self.selected_upgrade_target_idx].name)
                            .show_ui(ui, |ui| {
                                for (i, t) in self.building_templates.iter().enumerate() {
                                    ui.selectable_value(&mut self.selected_upgrade_target_idx, i, &t.name);
                                }
                            });
                        if ui.button("➕ 添加升级").clicked() {
                            self.upgrade_events.push(UpgradeEvent {
                                building_name: self.building_templates[self.selected_upgrade_target_idx].name.clone(),
                                wave_num: self.current_wave_num, is_late: self.current_is_late,
                            });
                        }
                    });
                });
                ui.group(|ui| {
                    ui.label("🏗️ 待放建筑物:");
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for (i, t) in self.building_templates.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut self.selected_building_idx, i, &t.name);
                                let (rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
                                if let Some(icon) = &t.icon { ui.painter().image(icon.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
                                else { ui.painter().rect_filled(rect, 2.0, t.color); }
                            });
                        }
                    });
                });
            } else {
                ui.group(|ui| {
                    ui.label("🔥 拆除序列:");
                    let mut delete_idx = None;
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        for (i, ev) in self.demolish_events.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.button("🗑").clicked() { delete_idx = Some(i); }
                                ui.label(format!("W{}{}: 拆 {}", ev.wave_num, if ev.is_late{"L"} else {""}, ev.name));
                            });
                        }
                    });
                    if let Some(idx) = delete_idx { self.demolish_events.remove(idx); }
                });
            }
            ui.add_space(10.0);
            ui.group(|ui| {
                ui.horizontal(|ui| { ui.label("网格:"); ui.add(egui::DragValue::new(&mut self.grid_size).speed(0.1)); });
                ui.horizontal(|ui| {
                    ui.label("行列:");
                    if ui.add(egui::DragValue::new(&mut self.grid_rows)).changed() { self.resize_grids(); }
                    if ui.add(egui::DragValue::new(&mut self.grid_cols)).changed() { self.resize_grids(); }
                });
                if ui.button("🖼️ 载入底图").clicked() { self.pick_and_load_image(ctx); }
            });
            ui.add_space(10.0);
            ui.group(|ui| {
                ui.label("💾 存取 (output/):");
                ui.horizontal(|ui| { ui.text_edit_singleline(&mut self.map_filename); if ui.button("导出地形").clicked() { self.export_terrain(); } });
                ui.horizontal(|ui| { ui.text_edit_singleline(&mut self.building_filename); if ui.button("导出序列").clicked() { self.export_buildings(); } });
                ui.horizontal(|ui| { if ui.button("导入地形").clicked() { self.import_terrain(); } if ui.button("导入序列").clicked() { self.import_buildings(); } });
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

            let grid = self.layers_data.get(&self.current_major_z).unwrap();
            for r in 0..self.grid_rows {
                for c in 0..self.grid_cols {
                    let rect = Rect::from_min_size(origin + Vec2::new(c as f32 * z_grid, r as f32 * z_grid), Vec2::splat(z_grid)).shrink(0.5);
                    if panel_rect.intersects(rect) { painter.rect_filled(rect, 0.0, Self::get_color(grid[r][c])); }
                }
            }

            let t_current = Self::get_time_value(self.current_wave_num, self.current_is_late);

            for b in &self.placed_buildings {
                let t_create = Self::get_time_value(b.wave_num, b.is_late);
                let t_demolish = self.get_building_demolish_time(b.uid);

                // [Fix] 优化了透明度变量赋值，消除警告
                let alpha_mult = if t_current >= t_demolish {
                    0.05 // 历史遗迹
                } else if t_current < t_create {
                    0.3 // 未来规划
                } else {
                    1.0 // 当前有效
                };

                // [Fix] 删除了未使用的 is_interactive 变量

                let rect = Rect::from_min_size(origin + Vec2::new(b.grid_x as f32 * z_grid, b.grid_y as f32 * z_grid), Vec2::new(b.width as f32 * z_grid, b.height as f32 * z_grid));
                
                let temp = self.building_templates.iter().find(|t| t.name == b.template_name);
                if let Some(t) = temp {
                    let tint = Color32::from_white_alpha((255.0 * alpha_mult) as u8);
                    if let Some(icon) = &t.icon { 
                        painter.image(icon.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint); 
                    } else { 
                        let c = b.color;
                        painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * alpha_mult) as u8)); 
                    }
                }
                
                if alpha_mult > 0.1 {
                    let stroke_alpha = (180.0 * alpha_mult) as u8;
                    painter.rect_stroke(rect, 1.5, Stroke::new(1.5, Color32::from_black_alpha(stroke_alpha)));
                    
                    if self.zoom > 0.4 {
                        let wave_text = format!("W{}{}", b.wave_num, if b.is_late { "L" } else { "" });
                        painter.text(rect.min + Vec2::new(2.0, 2.0), egui::Align2::LEFT_TOP, wave_text, egui::FontId::proportional(11.0 * self.zoom.max(1.0)), Color32::from_white_alpha(stroke_alpha));
                    }
                }

                if t_demolish != i32::MAX && alpha_mult > 0.1 {
                    painter.line_segment([rect.min, rect.max], Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 0, 0, (200.0 * alpha_mult) as u8)));
                    painter.line_segment([rect.left_bottom(), rect.right_top()], Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 0, 0, (200.0 * alpha_mult) as u8)));
                }
            }

            if let Some(pos) = input.pointer.hover_pos() {
                let rel = pos - origin; 
                if self.mode == EditMode::Terrain {
                    let c = (rel.x / z_grid).floor() as i32;
                    let r = (rel.y / z_grid).floor() as i32;
                    if r >= 0 && c >= 0 && (r as usize) < self.grid_rows && (c as usize) < self.grid_cols {
                        if input.pointer.button_down(egui::PointerButton::Primary) || input.pointer.button_down(egui::PointerButton::Secondary) {
                            let grid = self.layers_data.get_mut(&self.current_major_z).unwrap();
                            let val = if input.pointer.button_down(egui::PointerButton::Primary) { self.current_brush } else { -1 };
                            let rad = self.brush_radius;
                            for dr in (r-rad)..=(r+rad) {
                                for dc in (c-rad)..=(c+rad) {
                                    if dr >= 0 && dc >= 0 && (dr as usize) < self.grid_rows && (dc as usize) < self.grid_cols { grid[dr as usize][dc as usize] = val; }
                                }
                            }
                        }
                    }
                } else if self.mode == EditMode::Building {
                    let t = &self.building_templates[self.selected_building_idx];
                    let c = ((rel.x / z_grid) - (t.width as f32 / 2.0)).round() as i32;
                    let r = ((rel.y / z_grid) - (t.height as f32 / 2.0)).round() as i32;

                    let ghost_rect = Rect::from_min_size(origin + Vec2::new(c as f32 * z_grid, r as f32 * z_grid), Vec2::new(t.width as f32 * z_grid, t.height as f32 * z_grid));
                    let is_valid = r >= 0 && c >= 0 && self.can_place_building(r as usize, c as usize, t.width, t.height);
                    let ghost_color = if is_valid { Color32::GREEN } else { Color32::RED };
                    painter.rect_stroke(ghost_rect, 0.0, Stroke::new(2.5, ghost_color));

                    if response.clicked_by(egui::PointerButton::Primary) && is_valid {
                        self.placed_buildings.push(PlacedBuilding {
                            uid: self.next_uid, template_name: t.name.clone(), grid_x: c as usize, grid_y: r as usize,
                            width: t.width, height: t.height, color: t.color, wave_num: self.current_wave_num, is_late: self.current_is_late
                        });
                        self.next_uid += 1;
                    } else if response.clicked_by(egui::PointerButton::Secondary) {
                        let pick_c = (rel.x / z_grid).floor() as i32;
                        let pick_r = (rel.y / z_grid).floor() as i32;
                        self.placed_buildings.retain(|b| !(pick_c >= b.grid_x as i32 && pick_c < (b.grid_x + b.width) as i32 && pick_r >= b.grid_y as i32 && pick_r < (b.grid_y + b.height) as i32));
                        self.demolish_events.retain(|e| !self.placed_buildings.iter().any(|b| b.uid == e.uid));
                    }
                } else if self.mode == EditMode::Demolish {
                    let pick_c = (rel.x / z_grid).floor() as i32;
                    let pick_r = (rel.y / z_grid).floor() as i32;
                    let t_current = Self::get_time_value(self.current_wave_num, self.current_is_late);

                    let target_b = self.placed_buildings.iter().find(|b| {
                        let t_create = Self::get_time_value(b.wave_num, b.is_late);
                        let t_demolish = self.get_building_demolish_time(b.uid);
                        pick_c >= b.grid_x as i32 && pick_c < (b.grid_x + b.width) as i32 &&
                        pick_r >= b.grid_y as i32 && pick_r < (b.grid_y + b.height) as i32 &&
                        (t_current >= t_create && t_current < t_demolish) 
                    });

                    if let Some(b) = target_b {
                        let rect = Rect::from_min_size(origin + Vec2::new(b.grid_x as f32 * z_grid, b.grid_y as f32 * z_grid), Vec2::new(b.width as f32 * z_grid, b.height as f32 * z_grid));
                        painter.rect_stroke(rect, 0.0, Stroke::new(3.0, Color32::YELLOW));
                        if response.clicked_by(egui::PointerButton::Primary) {
                            if !self.demolish_events.iter().any(|e| e.uid == b.uid) {
                                self.demolish_events.push(DemolishEvent {
                                    uid: b.uid,
                                    name: b.template_name.clone(),
                                    grid_x: b.grid_x, grid_y: b.grid_y, width: b.width, height: b.height,
                                    wave_num: self.current_wave_num, is_late: self.current_is_late,
                                });
                            }
                        }
                    }
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([1350.0, 850.0]), ..Default::default() };
    eframe::run_native("MINKE 操作序列编辑器", options, Box::new(|cc| {
        let mut f = egui::FontDefinitions::default();
        if let Ok(d) = fs::read("C:\\Windows\\Fonts\\simhei.ttf") {
            f.font_data.insert("s".into(), egui::FontData::from_owned(d));
            f.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "s".into());
        }
        cc.egui_ctx.set_fonts(f);
        Box::new(MapEditor::new(cc))
    }))
}