use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2, Align2, FontId};
use image::io::Reader as ImageReader;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use rfd::FileDialog;

use crate::models::*;
use crate::utils::*;

pub struct MapEditor {
    pub(crate) texture: Option<TextureHandle>,
    pub(crate) grid_size: f32,
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
    pub(crate) map_bottom: f32,
    pub(crate) grid_rows: usize,
    pub(crate) grid_cols: usize,
    pub(crate) current_major_z: i32,
    pub(crate) layers_data: HashMap<i32, LayerData>, 
    pub(crate) current_edit_layer_type: BuildingType,
    pub(crate) current_brush: i8,
    pub(crate) brush_radius: i32, 
    pub(crate) zoom: f32,
    pub(crate) pan: Vec2,
    pub(crate) mode: EditMode,
    pub(crate) building_templates: Vec<BuildingTemplate>,
    pub(crate) selected_building_idx: usize,
    pub(crate) selected_upgrade_target_idx: usize, 
    pub(crate) placed_buildings: Vec<PlacedBuilding>,
    pub(crate) next_uid: usize,
    pub(crate) map_filename: String,
    pub(crate) building_filename: String,
    pub(crate) presets: Vec<MapPreset>,
    pub current_wave_num: i32,
    pub current_is_late: bool,
    pub(crate) upgrade_events: Vec<UpgradeEvent>,
    pub(crate) demolish_events: Vec<DemolishEvent>,
    pub(crate) hover_info: String,
}

impl MapEditor {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
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
                        b_type: cfg.b_type,
                        width: cfg.width, height: cfg.height,
                        color: Color32::from_rgba_unmultiplied(cfg.color[0], cfg.color[1], cfg.color[2], cfg.color[3]),
                        icon: load_icon(&cc.egui_ctx, &cfg.icon_path),
                    });
                }
            }
        }
        if b_templates.is_empty() {
            b_templates.push(BuildingTemplate { name: "默认 (1x1)".into(), b_type: BuildingType::Floor, width: 1, height: 1, color: Color32::GRAY, icon: None });
        }

        let mut map_presets = Vec::new();
        if let Ok(pre_str) = fs::read_to_string("maps/map_presets.json") {
            if let Ok(presets) = serde_json::from_str::<Vec<MapPreset>>(&pre_str) { map_presets = presets; }
        }

        let mut editor = Self {
            texture: None, grid_size: 32.0, offset_x: 0.0, offset_y: 0.0, 
            map_bottom: 1080.0,
            grid_rows: 40, grid_cols: 40, current_major_z: 0,
            layers_data: HashMap::new(), 
            current_edit_layer_type: BuildingType::Floor,
            current_brush: 0, brush_radius: 0,
            zoom: 1.0, pan: Vec2::ZERO, mode: EditMode::Terrain,
            building_templates: b_templates, selected_building_idx: 0, selected_upgrade_target_idx: 0,
            placed_buildings: Vec::new(), next_uid: 1000,
            map_filename: "terrain_01.json".to_string(), building_filename: "strategy_01.json".to_string(),
            presets: map_presets, current_wave_num: 1, current_is_late: false,
            upgrade_events: Vec::new(), demolish_events: Vec::new(),
            hover_info: String::new(),
        };

        let default_grid = vec![vec![-1; 40]; 40];
        editor.layers_data.insert(0, LayerData {
            major_z: 0,
            name: "Default Layer".into(),
            floor_grid: default_grid.clone(),
            wall_grid: default_grid.clone(),
            ceiling_grid: default_grid,
            elevation_grid: None, // 初始化为 None
        });

        editor
    }

    fn apply_preset(&mut self, ctx: &egui::Context, preset: &MapPreset) {
        let image_p = fix_path(&preset.image_path);
        let terrain_p = fix_path(&preset.terrain_path);
        if let Ok(img_reader) = ImageReader::open(&image_p) {
            if let Ok(img) = img_reader.decode() {
                let size = [img.width() as _, img.height() as _];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.to_rgba8().as_flat_samples().as_slice());
                self.texture = Some(ctx.load_texture(&image_p, color_image, Default::default()));
                self.map_bottom = size[1] as f32;
            }
        }
        if let Ok(content) = fs::read_to_string(&terrain_p) {
            if let Ok(data) = serde_json::from_str::<MapTerrainExport>(&content) {
                self.grid_size = data.meta.grid_pixel_size; self.offset_x = data.meta.offset_x; self.offset_y = data.meta.offset_y;
                if data.meta.bottom > 0.0 { self.map_bottom = data.meta.bottom; }
                self.layers_data.clear();
                for mut layer in data.layers {
                    // 🔥 自动迁移旧数据
                    layer.normalize();
                    
                    self.grid_rows = layer.floor_grid.len().max(self.grid_rows); 
                    self.grid_cols = layer.floor_grid.first().map_or(0, |r| r.len()).max(self.grid_cols);
                    self.layers_data.insert(layer.major_z, layer);
                }
                // 加载后确保所有层尺寸正确（防止旧数据为空导致 panic）
                self.resize_grids();
                self.map_filename = Path::new(&terrain_p).file_name().unwrap().to_string_lossy().into();
            }
        }
    }

    fn get_building_demolish_time(&self, uid: usize) -> i32 {
        self.demolish_events.iter().find(|d| d.uid == uid).map(|d| get_time_value(d.wave_num, d.is_late)).unwrap_or(i32::MAX)
    }

    fn check_terrain_capability(&self, terrain_id: i8, b_type: BuildingType) -> bool {
        if terrain_id < 0 { return false; }
        match b_type {
            BuildingType::Floor => true,
            BuildingType::Wall => true,
            BuildingType::Ceiling => true,
        }
    }

    fn can_place_building(&self, start_r: usize, start_c: usize, w: usize, h: usize, b_type: BuildingType) -> bool {
        if start_r + h > self.grid_rows || start_c + w > self.grid_cols { return false; }
        
        let layer = self.layers_data.get(&self.current_major_z).unwrap();
        let target_grid = layer.get_grid(b_type);
        
        // 增加安全检查：如果旧地图没有初始化该层 grid，直接返回 false
        if target_grid.is_empty() { return false; }

        let base_height = target_grid[start_r][start_c];
        if base_height < 0 { return false; } 

        for r in start_r..(start_r + h) {
            for c in start_c..(start_c + w) {
                let cell_h = target_grid[r][c];
                if cell_h != base_height { return false; }
                if !self.check_terrain_capability(cell_h, b_type) { return false; }
            }
        }

        let t_current = get_time_value(self.current_wave_num, self.current_is_late);
        for b in &self.placed_buildings {
            if b.b_type != b_type { continue; }

            if start_c < b.grid_x + b.width && start_c + w > b.grid_x && start_r < b.grid_y + b.height && start_r + h > b.grid_y {
                let t_create = get_time_value(b.wave_num, b.is_late);
                let t_demolish = self.get_building_demolish_time(b.uid);
                if t_current >= t_create && t_current < t_demolish { return false; }
            }
        }
        true
    }

    fn resize_grids(&mut self) {
        for layer in self.layers_data.values_mut() {
            for grid in [&mut layer.floor_grid, &mut layer.wall_grid, &mut layer.ceiling_grid] {
                // 如果是空网格（例如新创建的层或旧数据迁移后留空的层），先初始化为 -1
                if grid.is_empty() {
                    *grid = vec![vec![-1; self.grid_cols]; self.grid_rows];
                } else {
                    grid.resize(self.grid_rows, vec![-1; self.grid_cols]);
                    for row in grid.iter_mut() { row.resize(self.grid_cols, -1); }
                }
            }
        }
    }

    fn pick_and_load_image(&mut self, ctx: &egui::Context) {
        if let Some(path) = FileDialog::new().add_filter("图片文件", &["png", "jpg", "jpeg", "bmp"]).pick_file() {
            if let Ok(img_reader) = ImageReader::open(&path) {
                if let Ok(img) = img_reader.decode() {
                    let size = [img.width() as _, img.height() as _];
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.to_rgba8().as_flat_samples().as_slice());
                    self.texture = Some(ctx.load_texture(path.to_string_lossy(), color_image, Default::default()));
                    self.map_bottom = size[1] as f32;
                }
            }
        }
    }

    fn import_terrain(&mut self) {
        if let Some(path) = FileDialog::new().set_directory("output").add_filter("JSON地形", &["json"]).pick_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str::<MapTerrainExport>(&content) {
                    self.grid_size = data.meta.grid_pixel_size; self.offset_x = data.meta.offset_x; self.offset_y = data.meta.offset_y;
                    if data.meta.bottom > 0.0 { self.map_bottom = data.meta.bottom; }
                    self.layers_data.clear();
                    for mut layer in data.layers {
                        // 🔥 兼容处理：迁移数据
                        layer.normalize();
                        
                        // 确保 grid_rows/cols 更新到加载的地图尺寸
                        // 注意：这里取 floor_grid 的尺寸，如果是旧数据迁移过来的，它会有值
                        if !layer.floor_grid.is_empty() {
                            self.grid_rows = layer.floor_grid.len();
                            self.grid_cols = layer.floor_grid[0].len();
                        }
                        self.layers_data.insert(layer.major_z, layer);
                    }
                    // 确保所有层（包括刚刚可能没数据的 Wall/Ceiling）都被初始化到正确尺寸
                    self.resize_grids(); 
                }
            }
        }
    }

    fn import_buildings(&mut self) {
        if let Some(path) = FileDialog::new().set_directory("output").add_filter("JSON策略", &["json"]).pick_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str::<MapBuildingsExport>(&content) {
                    self.placed_buildings = data.buildings.iter().map(|b| {
                        let template = self.building_templates.iter().find(|t| t.name == b.name);
                        let color = template.map(|t| t.color).unwrap_or(Color32::GRAY);
                        PlacedBuilding { 
                            uid: b.uid, 
                            template_name: b.name.clone(), 
                            b_type: b.b_type,
                            grid_x: b.grid_x, grid_y: b.grid_y, width: b.width, height: b.height, 
                            color, wave_num: b.wave_num, is_late: b.is_late 
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
        let out = PathBuf::from("output").join(&self.map_filename);
        let meta = MapMeta { grid_pixel_size: self.grid_size, offset_x: self.offset_x, offset_y: self.offset_y, bottom: self.map_bottom };
        let mut layers: Vec<LayerData> = self.layers_data.values().cloned().collect();
        layers.sort_by_key(|l| l.major_z);
        if let Ok(json) = serde_json::to_string_pretty(&MapTerrainExport { map_name: "Ni-Zhan_Map".into(), meta, layers }) { let _ = fs::write(out, json); }
    }

    fn export_buildings(&self) {
        let _ = fs::create_dir_all("output");
        let b_exp: Vec<BuildingExport> = self.placed_buildings.iter().map(|b| BuildingExport { 
            uid: b.uid, 
            name: b.template_name.clone(),
            b_type: b.b_type,
            grid_x: b.grid_x, grid_y: b.grid_y, width: b.width, height: b.height, 
            wave_num: b.wave_num, is_late: b.is_late 
        }).collect();
        let out = PathBuf::from("output").join(&self.building_filename);
        if let Ok(json) = serde_json::to_string_pretty(&MapBuildingsExport { map_name: "Ni-Zhan_Map".into(), buildings: b_exp, upgrades: self.upgrade_events.clone(), demolishes: self.demolish_events.clone() }) { let _ = fs::write(out, json); }
    }
}

impl eframe::App for MapEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("control").resizable(false).default_width(320.0).show(ctx, |ui| {
            ui.style_mut().spacing.item_spacing.y = 8.0;
            ui.vertical_centered_justified(|ui| { ui.heading("MINKE 策略编辑器"); });

            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.label("当前状态监视:");
                ui.label(&self.hover_info);
            });

            // ... (预设、模式选择、时间轴控制代码保持不变) ...
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.label("关卡预设:");
                ui.vertical_centered_justified(|ui| {
                    for (i, preset) in self.presets.clone().iter().enumerate() {
                        ui.push_id(i, |ui| { if ui.button(format!("加载: {}", preset.name)).clicked() { self.apply_preset(ctx, preset); } });
                    }
                });
            });
            ui.separator();
            ui.columns(4, |cols| {
                cols[0].vertical_centered_justified(|ui| { ui.selectable_value(&mut self.mode, EditMode::Terrain, "地形"); });
                cols[1].vertical_centered_justified(|ui| { ui.selectable_value(&mut self.mode, EditMode::Building, "布局"); });
                cols[2].vertical_centered_justified(|ui| { ui.selectable_value(&mut self.mode, EditMode::Upgrade, "升级"); });
                cols[3].vertical_centered_justified(|ui| { ui.selectable_value(&mut self.mode, EditMode::Demolish, "拆除"); });
            });
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.label("时间轴控制:");
                ui.horizontal(|ui| {
                    ui.label("当前波次:");
                    ui.add(egui::DragValue::new(&mut self.current_wave_num).speed(1).clamp_range(1..=100));
                    ui.checkbox(&mut self.current_is_late, "后期");
                });
            });

            if self.mode == EditMode::Terrain {
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("地形编辑层级:");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.current_edit_layer_type, BuildingType::Floor, "地面");
                        ui.radio_value(&mut self.current_edit_layer_type, BuildingType::Wall, "墙壁");
                        ui.radio_value(&mut self.current_edit_layer_type, BuildingType::Ceiling, "吊顶");
                    });
                    ui.separator();

                    ui.label("地形笔刷:");
                    let brushes = [(-1, "障碍"), (0, "平地"), (1, "高台1"), (2, "高台2"), (3, "高台3")];
                    for (val, label) in brushes.iter() {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.current_brush, *val, *label);
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, get_layer_color(*val));
                        });
                    }
                    ui.add(egui::Slider::new(&mut self.brush_radius, 0..=10).text("笔刷半径"));
                });

            } else if self.mode == EditMode::Building {
                // ... (建筑模式 UI 保持不变) ...
                 ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("选择建筑物:");
                    egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                        ui.vertical_centered_justified(|ui| {
                            for (i, t) in self.building_templates.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.set_min_width(ui.available_width());
                                    let type_label = match t.b_type {
                                        BuildingType::Floor => "[地]",
                                        BuildingType::Wall => "[墙]",
                                        BuildingType::Ceiling => "[顶]",
                                    };
                                    ui.radio_value(&mut self.selected_building_idx, i, format!("{} {}", type_label, t.name));
                                    
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.add_space(5.0);
                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::hover());
                                        if let Some(icon) = &t.icon { ui.painter().image(icon.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
                                        else { ui.painter().rect_filled(rect, 2.0, t.color); }
                                    });
                                });
                            }
                        });
                    });
                });
            } else if self.mode == EditMode::Upgrade {
                // ... (升级模式 UI 保持不变) ...
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("添加全局升级:");
                    ui.vertical_centered_justified(|ui| {
                        egui::ComboBox::from_label("目标塔")
                            .selected_text(&self.building_templates[self.selected_upgrade_target_idx].name)
                            .show_ui(ui, |ui| {
                                for (i, t) in self.building_templates.iter().enumerate() {
                                    ui.selectable_value(&mut self.selected_upgrade_target_idx, i, &t.name);
                                }
                            });
                        if ui.button("[+] 添加升级指令").clicked() {
                            self.upgrade_events.push(UpgradeEvent { 
                                building_name: self.building_templates[self.selected_upgrade_target_idx].name.clone(), 
                                wave_num: self.current_wave_num, 
                                is_late: self.current_is_late 
                            });
                        }
                    });
                });
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("已配置的升级序列:");
                    let mut delete_idx = None;
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        if self.upgrade_events.is_empty() { ui.label("暂无升级记录"); }
                        for (i, ev) in self.upgrade_events.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.button("[X]").clicked() { delete_idx = Some(i); }
                                ui.label(format!("W{}{}: 升级 {}", ev.wave_num, if ev.is_late{"L"} else {""}, ev.building_name));
                            });
                        }
                    });
                    if let Some(idx) = delete_idx { self.upgrade_events.remove(idx); }
                });
            } else { 
                // ... (拆除模式 UI 保持不变) ...
                 ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("拆除任务预览:");
                    let mut delete_idx = None;
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        if self.demolish_events.is_empty() { ui.label("暂无拆除记录"); }
                        for (i, ev) in self.demolish_events.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.button("[X]").clicked() { delete_idx = Some(i); }
                                ui.label(format!("W{}{}: 拆除 {}", ev.wave_num, if ev.is_late{"L"} else {""}, ev.name));
                            });
                        }
                    });
                    if let Some(idx) = delete_idx { self.demolish_events.remove(idx); }
                });
            }

            ui.add_space(10.0);
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| { ui.label("格子大小:"); ui.add(egui::DragValue::new(&mut self.grid_size).speed(0.1)); });
                ui.horizontal(|ui| {
                    ui.label("偏移 X:"); ui.add(egui::DragValue::new(&mut self.offset_x).speed(1.0));
                    ui.label("偏移 Y:"); ui.add(egui::DragValue::new(&mut self.offset_y).speed(1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("底图高度:"); ui.add(egui::DragValue::new(&mut self.map_bottom).speed(1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("网格行列:");
                    if ui.add(egui::DragValue::new(&mut self.grid_rows)).changed() { self.resize_grids(); }
                    if ui.add(egui::DragValue::new(&mut self.grid_cols)).changed() { self.resize_grids(); }
                });
                ui.vertical_centered_justified(|ui| { if ui.button("加载自定义地图底图").clicked() { self.pick_and_load_image(ctx); } });
            });

            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.label("数据存取 (output/):");
                ui.vertical_centered_justified(|ui| {
                    ui.text_edit_singleline(&mut self.map_filename);
                    if ui.button("导出地形 JSON").clicked() { self.export_terrain(); }
                    ui.text_edit_singleline(&mut self.building_filename);
                    if ui.button("导出策略 JSON").clicked() { self.export_buildings(); }
                    ui.separator();
                    if ui.button("导入地形文件").clicked() { self.import_terrain(); } 
                    if ui.button("导入策略文件").clicked() { self.import_buildings(); } 
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let input = ui.input(|i| i.clone());
            let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let panel_rect = response.rect; 
            if input.pointer.button_down(egui::PointerButton::Middle) { self.pan += input.pointer.delta(); }
            if response.hovered() {
                let scroll = input.raw_scroll_delta.y;
                if scroll != 0.0 {
                    let old = self.zoom; self.zoom = (self.zoom * (1.0 + scroll * 0.001)).clamp(0.1, 10.0);
                    if let Some(pos) = input.pointer.hover_pos() { self.pan -= (pos - panel_rect.min - self.pan) * (self.zoom / old - 1.0); }
                }
            }

            let origin = panel_rect.min + self.pan + Vec2::new(self.offset_x * self.zoom, self.offset_y * self.zoom);
            let z_grid = self.grid_size * self.zoom;

            if let Some(tex) = &self.texture {
                painter.image(tex.id(), Rect::from_min_size(panel_rect.min + self.pan, tex.size_vec2() * self.zoom), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }

            let layer = self.layers_data.get(&self.current_major_z).unwrap();

            let draw_layer = |grid: &Vec<Vec<i8>>, layer_type: BuildingType, is_active: bool| {
                for r in 0..self.grid_rows {
                    for c in 0..self.grid_cols {
                        let val = grid[r][c];
                        // 🔥 修复1：允许绘制 -1 (障碍物)。假设 -2 或更小才是“空/透明”
                        // 如果 grid 初始化是 -1，那么整个地图默认是红的。如果不想这样，建议初始值改为 -2，或者用户手动刷 -1
                        // 这里改为只跳过小于 -1 的值
                        if val < -1 { continue; } 

                        let rect = Rect::from_min_size(origin + Vec2::new(c as f32 * z_grid, r as f32 * z_grid), Vec2::splat(z_grid)).shrink(0.5);
                        
                        if panel_rect.intersects(rect) { 
                            let mut color = get_layer_color(val); 
                            
                            match layer_type {
                                BuildingType::Floor => {}, 
                                BuildingType::Wall => { color = Color32::from_rgba_unmultiplied(color.r(), (color.g() as f32 * 0.5) as u8, color.b(), 220); }, 
                                BuildingType::Ceiling => { color = Color32::from_rgba_unmultiplied(color.r(), color.g(), (color.b() as f32 * 0.5) as u8, 220); }, 
                            }

                            if !is_active {
                                color = color.linear_multiply(0.2);
                            }

                            if is_active && self.mode == EditMode::Terrain {
                                painter.rect_filled(rect, 0.0, color);
                            } else {
                                if is_active { painter.rect_filled(rect, 0.0, color); }
                                else { painter.rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(1.0, color)); }
                            }
                        }
                    }
                }
            };

            for &l_type in &[BuildingType::Floor, BuildingType::Wall, BuildingType::Ceiling] {
                if l_type != self.current_edit_layer_type {
                    draw_layer(layer.get_grid(l_type), l_type, false);
                }
            }
            draw_layer(layer.get_grid(self.current_edit_layer_type), self.current_edit_layer_type, true);

            // ... (建筑绘制和鼠标交互代码保持不变) ...
             let t_current = get_time_value(self.current_wave_num, self.current_is_late);
            let highlight_target_name = if self.mode == EditMode::Upgrade {
                Some(self.building_templates[self.selected_upgrade_target_idx].name.clone())
            } else { None };

            for b in &self.placed_buildings {
                let t_create = get_time_value(b.wave_num, b.is_late);
                let t_demolish = self.get_building_demolish_time(b.uid);
                let alpha_mult = if t_current >= t_demolish { 0.05 } else if t_current < t_create { 0.3 } else { 1.0 };
                let rect = Rect::from_min_size(origin + Vec2::new(b.grid_x as f32 * z_grid, b.grid_y as f32 * z_grid), Vec2::new(b.width as f32 * z_grid, b.height as f32 * z_grid));
                
                let temp = self.building_templates.iter().find(|t| t.name == b.template_name);
                if let Some(t) = temp {
                    let tint = Color32::from_white_alpha((255.0 * alpha_mult) as u8);
                    if let Some(icon) = &t.icon { painter.image(icon.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint); }
                    else { painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(b.color.r(), b.color.g(), b.color.b(), (b.color.a() as f32 * alpha_mult) as u8)); }
                }
                
                if alpha_mult > 0.1 {
                    let stroke_alpha = (180.0 * alpha_mult) as u8;
                    painter.rect_stroke(rect, 1.5, Stroke::new(1.5, Color32::from_black_alpha(stroke_alpha)));
                    painter.text(rect.min + Vec2::new(2.0, 2.0), Align2::LEFT_TOP, format!("W{}{}", b.wave_num, if b.is_late { "L" } else { "" }), FontId::proportional(11.0 * self.zoom.max(1.0)), Color32::from_white_alpha(stroke_alpha));
                }

                if let Some(target) = &highlight_target_name {
                    if &b.template_name == target && alpha_mult > 0.5 {
                        painter.rect_stroke(rect.expand(2.0), 0.0, Stroke::new(2.5, Color32::GREEN));
                    }
                }

                if t_demolish != i32::MAX && alpha_mult > 0.1 {
                    painter.line_segment([rect.min, rect.max], Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 0, 0, (200.0 * alpha_mult) as u8)));
                    painter.line_segment([rect.left_bottom(), rect.right_top()], Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 0, 0, (200.0 * alpha_mult) as u8)));
                }
            }

            self.hover_info = "无".to_string(); 

            if let Some(pos) = input.pointer.hover_pos() {
                let rel = pos - origin; 
                let (cx, ry) = ((rel.x / z_grid).floor() as i32, (rel.y / z_grid).floor() as i32);
                
                if cx >= 0 && ry >= 0 && (cx as usize) < self.grid_cols && (ry as usize) < self.grid_rows {
                    let current_grid = layer.get_grid(self.current_edit_layer_type);
                    let terrain_h = current_grid[ry as usize][cx as usize];
                    
                    // 🔥 修复2：增加像素坐标显示
                    let px_x = cx as f32 * self.grid_size;
                    let px_y = ry as f32 * self.grid_size;
                    self.hover_info = format!("Grid: ({}, {})\nPixel: ({:.1}, {:.1})\n层级: {:?}\nID: {}", cx, ry, px_x, px_y, self.current_edit_layer_type, terrain_h);

                    let hovered_buildings: Vec<&PlacedBuilding> = self.placed_buildings.iter().filter(|b| {
                        cx >= b.grid_x as i32 && cx < (b.grid_x + b.width) as i32 && 
                        ry >= b.grid_y as i32 && ry < (b.grid_y + b.height) as i32 &&
                        t_current >= get_time_value(b.wave_num, b.is_late) && t_current < self.get_building_demolish_time(b.uid)
                    }).collect();

                    if !hovered_buildings.is_empty() {
                        self.hover_info += "\n\n[建筑]:";
                        for b in hovered_buildings {
                            let type_str = match b.b_type {
                                BuildingType::Floor => "地", BuildingType::Wall => "墙", BuildingType::Ceiling => "顶",
                            };
                            self.hover_info += &format!("\n- {} ({})", b.template_name, type_str);
                        }
                    }
                } else {
                    self.hover_info = "光标越界".to_string();
                }
                
                 if self.mode == EditMode::Terrain {
                    let (c, r) = (cx, ry);
                    if r >= 0 && c >= 0 && (r as usize) < self.grid_rows && (c as usize) < self.grid_cols {
                        if input.pointer.button_down(egui::PointerButton::Primary) || input.pointer.button_down(egui::PointerButton::Secondary) {
                            let layer_data = self.layers_data.get_mut(&self.current_major_z).unwrap();
                            let grid = layer_data.get_grid_mut(self.current_edit_layer_type);
                            
                            let val = if input.pointer.button_down(egui::PointerButton::Primary) { self.current_brush } else { -1 };
                            for dr in (r-self.brush_radius)..=(r+self.brush_radius) {
                                for dc in (c-self.brush_radius)..=(c+self.brush_radius) {
                                    if dr >= 0 && dc >= 0 && (dr as usize) < self.grid_rows && (dc as usize) < self.grid_cols { grid[dr as usize][dc as usize] = val; }
                                }
                            }
                        }
                    }
                } else if self.mode == EditMode::Building {
                    // ... (保持不变)
                     let t = &self.building_templates[self.selected_building_idx];
                    let c = ((rel.x / z_grid) - (t.width as f32 / 2.0)).round() as i32;
                    let r = ((rel.y / z_grid) - (t.height as f32 / 2.0)).round() as i32;
                    let ghost_rect = Rect::from_min_size(origin + Vec2::new(c as f32 * z_grid, r as f32 * z_grid), Vec2::new(t.width as f32 * z_grid, t.height as f32 * z_grid));
                    
                    let is_valid = r >= 0 && c >= 0 && self.can_place_building(r as usize, c as usize, t.width, t.height, t.b_type);
                    
                    painter.rect_stroke(ghost_rect, 0.0, Stroke::new(2.5, if is_valid { Color32::GREEN } else { Color32::RED }));
                    if response.clicked_by(egui::PointerButton::Primary) && is_valid {
                        self.placed_buildings.push(PlacedBuilding { 
                            uid: self.next_uid, 
                            template_name: t.name.clone(), 
                            b_type: t.b_type, 
                            grid_x: c as usize, grid_y: r as usize, width: t.width, height: t.height, 
                            color: t.color, wave_num: self.current_wave_num, is_late: self.current_is_late 
                        });
                        self.next_uid += 1;
                    } else if response.clicked_by(egui::PointerButton::Secondary) {
                        let (px, py) = (cx, ry);
                        self.placed_buildings.retain(|b| !(px >= b.grid_x as i32 && px < (b.grid_x + b.width) as i32 && py >= b.grid_y as i32 && py < (b.grid_y + b.height) as i32));
                        self.demolish_events.retain(|e| !self.placed_buildings.iter().any(|b| b.uid == e.uid));
                    }
                } else if self.mode == EditMode::Demolish {
                    // ... (保持不变)
                    let (px, py) = (cx, ry);
                    let target = self.placed_buildings.iter().find(|b| {
                        px >= b.grid_x as i32 && px < (b.grid_x + b.width) as i32 && py >= b.grid_y as i32 && py < (b.grid_y + b.height) as i32 &&
                        t_current >= get_time_value(b.wave_num, b.is_late) && t_current < self.get_building_demolish_time(b.uid)
                    });
                    if let Some(b) = target {
                        let r = Rect::from_min_size(origin + Vec2::new(b.grid_x as f32 * z_grid, b.grid_y as f32 * z_grid), Vec2::new(b.width as f32 * z_grid, b.height as f32 * z_grid));
                        painter.rect_stroke(r, 0.0, Stroke::new(3.0, Color32::YELLOW));
                        if response.clicked_by(egui::PointerButton::Primary) && !self.demolish_events.iter().any(|e| e.uid == b.uid) {
                            self.demolish_events.push(DemolishEvent { uid: b.uid, name: b.template_name.clone(), grid_x: b.grid_x, grid_y: b.grid_y, width: b.width, height: b.height, wave_num: self.current_wave_num, is_late: self.current_is_late });
                        }
                    }
                }
            }
        });
    }
}