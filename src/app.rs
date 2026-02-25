use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2, Align2, FontId, FontFamily};
use image::io::Reader as ImageReader;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use rfd::FileDialog;

use crate::models::*;
use crate::utils::*;

pub struct MapEditor {
    pub(crate) texture: Option<TextureHandle>,
    pub(crate) grid_width: f32,
    pub(crate) grid_height: f32,
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
    pub(crate) map_bottom: f32,
    pub(crate) map_right: f32,
    pub(crate) camera_speed_up: f32,
    pub(crate) camera_speed_down: f32,
    pub(crate) camera_speed_left: f32,
    pub(crate) camera_speed_right: f32,
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
    pub(crate) presets: Vec<MapPreset>,
    pub current_wave_num: i32,
    pub current_is_late: bool,
    pub(crate) upgrade_events: Vec<UpgradeEvent>,
    pub(crate) demolish_events: Vec<DemolishEvent>,
    pub(crate) hover_info: String,
    pub(crate) building_configs: Vec<BuildingConfig>,
    pub(crate) building_config_icons: Vec<Option<TextureHandle>>,
    pub(crate) editing_building_idx: Option<usize>,
    pub(crate) viewport_pos: Vec2,
    pub(crate) viewport_width: f32,
    pub(crate) viewport_height: f32,
    pub(crate) viewport_safe_areas: Vec<Rect>,
    pub(crate) prep_actions: Vec<PrepAction>,
}

impl MapEditor {
    fn load_icon(ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
        let full_path = fix_path(path);
        if let Ok(img_reader) = ImageReader::open(&full_path) {
            if let Ok(img) = img_reader.decode() {
                let size = [img.width() as _, img.height() as _];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.to_rgba8().as_flat_samples().as_slice());
                return Some(ctx.load_texture(&full_path, color_image, Default::default()));
            }
        }
        None
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut b_templates = Vec::new();
        let mut b_configs = Vec::new();
        let mut b_config_icons = Vec::new();
        if let Ok(config_str) = fs::read_to_string("maps/buildings_config.json") {
            if let Ok(configs) = serde_json::from_str::<Vec<BuildingConfig>>(&config_str) {
                b_configs = configs.clone();
                for cfg in configs {
                    let icon = Self::load_icon(&cc.egui_ctx, &cfg.icon_path);
                    b_templates.push(BuildingTemplate {
                        name: cfg.name,
                        b_type: cfg.b_type,
                        width: cfg.width, height: cfg.height,
                        color: Color32::from_rgba_unmultiplied(cfg.color[0], cfg.color[1], cfg.color[2], cfg.color[3]),
                        icon: icon.clone(),
                    });
                    b_config_icons.push(icon);
                }
            }
        }
        if b_templates.is_empty() {
            b_templates.push(BuildingTemplate { name: "默认 (1x1)".into(), b_type: BuildingType::Floor, width: 1, height: 1, color: Color32::GRAY, icon: None });
            b_config_icons.push(None);
        }

        let mut map_presets = Vec::new();
        if let Ok(pre_str) = fs::read_to_string("maps/map_presets.json") {
            if let Ok(presets) = serde_json::from_str::<Vec<MapPreset>>(&pre_str) { map_presets = presets; }
        }

        let mut editor = Self {
            texture: None, grid_width: 32.0, grid_height: 32.0, offset_x: 0.0, offset_y: 0.0, 
            map_bottom: 1080.0, map_right: 1920.0,
            camera_speed_up: 1.0, camera_speed_down: 1.0, camera_speed_left: 1.0, camera_speed_right: 1.0,
            grid_rows: 40, grid_cols: 40, current_major_z: 0,
            layers_data: HashMap::new(), 
            current_edit_layer_type: BuildingType::Floor,
            current_brush: 0, brush_radius: 0,
            zoom: 1.0, pan: Vec2::ZERO, mode: EditMode::Terrain,
            building_templates: b_templates, selected_building_idx: 0, selected_upgrade_target_idx: 0,
            placed_buildings: Vec::new(), next_uid: 1000,
            map_filename: "terrain_01.json".to_string(),
            presets: map_presets, current_wave_num: 1, current_is_late: false,
            upgrade_events: Vec::new(), demolish_events: Vec::new(),
            hover_info: String::new(),
            building_configs: b_configs,
            building_config_icons: b_config_icons,
            editing_building_idx: None,
            viewport_pos: Vec2::ZERO,
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            viewport_safe_areas: Vec::new(),
            prep_actions: Vec::new(),
        };

        let default_grid = vec![vec![-1; 40]; 40];
        editor.layers_data.insert(0, LayerData {
            major_z: 0,
            name: "Default Layer".into(),
            floor_grid: default_grid.clone(),
            wall_grid: default_grid.clone(),
            ceiling_grid: default_grid,
            elevation_grid: None, 
        });

        editor
    }

    fn apply_preset(&mut self, ctx: &egui::Context, preset: &MapPreset) {
        let image_p = fix_path(&preset.image_path);
        let terrain_p = fix_path(&preset.terrain_path);
        let building_configs_p = fix_path(&preset.building_configs_path);
        let strategy_p = fix_path(&preset.strategy_path);
        
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
                self.grid_width = data.meta.grid_pixel_width; self.grid_height = data.meta.grid_pixel_height; self.offset_x = data.meta.offset_x; self.offset_y = data.meta.offset_y;
                if data.meta.bottom > 0.0 { self.map_bottom = data.meta.bottom; }
                if data.meta.right > 0.0 { self.map_right = data.meta.right; }
                self.camera_speed_up = data.meta.camera_speed_up;
                self.camera_speed_down = data.meta.camera_speed_down;
                self.camera_speed_left = data.meta.camera_speed_left;
                self.camera_speed_right = data.meta.camera_speed_right;
                self.viewport_safe_areas = data.meta.viewport_safe_areas.iter().map(|a| (*a).into()).collect();
                self.prep_actions = data.meta.prep_actions;
                self.layers_data.clear();
                for mut layer in data.layers {
                    layer.normalize();
                    if !layer.floor_grid.is_empty() {
                        self.grid_rows = layer.floor_grid.len();
                        self.grid_cols = layer.floor_grid[0].len();
                    }
                    self.layers_data.insert(layer.major_z, layer);
                }
                self.resize_grids();
                self.map_filename = Path::new(&terrain_p).file_name().unwrap().to_string_lossy().into();
            }
        }
        
        // 加载建筑列表
        if let Ok(content) = fs::read_to_string(&building_configs_p) {
            if let Ok(data) = serde_json::from_str::<Vec<BuildingConfig>>(&content) {
                self.building_configs = data;
                self.building_config_icons.clear();
                self.building_templates = self.building_configs.iter().map(|config| {
                    let icon = Self::load_icon(ctx, &config.icon_path);
                    self.building_config_icons.push(icon.clone());
                    BuildingTemplate {
                        name: config.name.clone(),
                        b_type: config.b_type,
                        width: config.width,
                        height: config.height,
                        color: Color32::from_rgba_unmultiplied(
                            config.color[0], config.color[1], 
                            config.color[2], config.color[3]
                        ),
                        icon,
                    }
                }).collect();
            }
        }
        
        // 加载策略
        if let Ok(content) = fs::read_to_string(&strategy_p) {
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
                    self.grid_width = data.meta.grid_pixel_width; self.grid_height = data.meta.grid_pixel_height; self.offset_x = data.meta.offset_x; self.offset_y = data.meta.offset_y;
                    if data.meta.bottom > 0.0 { self.map_bottom = data.meta.bottom; }
                    if data.meta.right > 0.0 { self.map_right = data.meta.right; }
                    self.camera_speed_up = data.meta.camera_speed_up;
                    self.camera_speed_down = data.meta.camera_speed_down;
                    self.camera_speed_left = data.meta.camera_speed_left;
                    self.camera_speed_right = data.meta.camera_speed_right;
                    self.viewport_safe_areas = data.meta.viewport_safe_areas.iter().map(|a| (*a).into()).collect();
                    self.prep_actions = data.meta.prep_actions;
                    self.layers_data.clear();
                    for mut layer in data.layers {
                        layer.normalize();
                        if !layer.floor_grid.is_empty() {
                            self.grid_rows = layer.floor_grid.len();
                            self.grid_cols = layer.floor_grid[0].len();
                        }
                        self.layers_data.insert(layer.major_z, layer);
                    }
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

    fn import_building_configs(&mut self, ctx: &egui::Context) {
        if let Some(path) = FileDialog::new().set_directory("output").add_filter("JSON防御塔列表", &["json"]).pick_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str::<Vec<BuildingConfig>>(&content) {
                    self.building_configs = data;
                    self.building_config_icons.clear();
                    self.building_templates = self.building_configs.iter().map(|config| {
                        let icon = Self::load_icon(ctx, &config.icon_path);
                        self.building_config_icons.push(icon.clone());
                        BuildingTemplate {
                            name: config.name.clone(),
                            b_type: config.b_type,
                            width: config.width,
                            height: config.height,
                            color: Color32::from_rgba_unmultiplied(
                                config.color[0], config.color[1], 
                                config.color[2], config.color[3]
                            ),
                            icon,
                        }
                    }).collect();
                }
            }
        }
    }

    fn export_terrain(&self) {
        let map_name = self.map_filename.split('.').next().unwrap_or("地图");
        let export_dir = PathBuf::from("output").join(map_name);
        let _ = fs::create_dir_all(&export_dir);
        
        let out = export_dir.join(format!("{}地图.json", map_name));
        let meta = MapMeta { 
            grid_pixel_width: self.grid_width, 
            grid_pixel_height: self.grid_height, 
            offset_x: self.offset_x, 
            offset_y: self.offset_y, 
            bottom: self.map_bottom, 
            right: self.map_right,
            camera_speed_up: self.camera_speed_up,
            camera_speed_down: self.camera_speed_down,
            camera_speed_left: self.camera_speed_left,
            camera_speed_right: self.camera_speed_right,
            viewport_safe_areas: self.viewport_safe_areas.iter().map(|r| (*r).into()).collect(),
            prep_actions: self.prep_actions.clone(),
        };
        let mut layers: Vec<LayerData> = self.layers_data.values().cloned().collect();
        layers.sort_by_key(|l| l.major_z);
        if let Ok(json) = serde_json::to_string_pretty(&MapTerrainExport { map_name: map_name.to_string(), meta, layers }) { let _ = fs::write(out, json); }
    }

    fn export_buildings(&self) {
        // 从map_filename中提取地图名称（去除.json扩展名）
        let map_name = self.map_filename.split('.').next().unwrap_or("地图");
        let export_dir = PathBuf::from("output").join(map_name);
        let _ = fs::create_dir_all(&export_dir);
        
        let b_exp: Vec<BuildingExport> = self.placed_buildings.iter().map(|b| BuildingExport { 
            uid: b.uid, 
            name: b.template_name.clone(),
            b_type: b.b_type,
            grid_x: b.grid_x, grid_y: b.grid_y, width: b.width, height: b.height, 
            wave_num: b.wave_num, is_late: b.is_late 
        }).collect();
        let out = export_dir.join(format!("{}策略.json", map_name));
        if let Ok(json) = serde_json::to_string_pretty(&MapBuildingsExport { map_name: map_name.to_string(), buildings: b_exp, upgrades: self.upgrade_events.clone(), demolishes: self.demolish_events.clone() }) { let _ = fs::write(out, json); }
    }

    fn show_building_config_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("保存配置").clicked() {
                let map_name = self.map_filename.split('.').next().unwrap_or("地图");
                let export_dir = PathBuf::from("output").join(map_name);
                let _ = fs::create_dir_all(&export_dir);
                
                let out = export_dir.join(format!("{}防御塔列表.json", map_name));
                if let Ok(json) = serde_json::to_string_pretty(&self.building_configs) { let _ = fs::write(out, json); }
            }
            if ui.button("添加建筑").clicked() {
                self.building_configs.push(BuildingConfig {
                    name: "新建筑".to_string(),
                    b_type: BuildingType::Floor,
                    grid_index: [0, 0],
                    width: 2,
                    height: 1,
                    color: [128, 128, 128, 255],
                    icon_path: "maps/icons/默认.png".to_string(),
                    cost: 100,
                });
                self.building_config_icons.push(None);
            }
        });

        ui.separator();

        let mut delete_idx = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for b_type in &[BuildingType::Floor, BuildingType::Wall, BuildingType::Ceiling] {
                ui.group(|ui| {
                    let type_name = match b_type {
                        BuildingType::Floor => "地面建筑",
                        BuildingType::Wall => "墙壁建筑",
                        BuildingType::Ceiling => "吊顶建筑",
                    };
                    ui.label(type_name);

                    let mut configs: Vec<_> = self.building_configs.iter()
                        .enumerate()
                        .filter(|(_, c)| c.b_type == *b_type)
                        .collect();
                    
                    configs.sort_by(|a, b| {
                        if a.1.grid_index[1] != b.1.grid_index[1] {
                            a.1.grid_index[1].cmp(&b.1.grid_index[1])
                        } else {
                            a.1.grid_index[0].cmp(&b.1.grid_index[0])
                        }
                    });

                    let mut rows = Vec::new();
                    let mut current_row = Vec::new();
                    let mut current_row_idx = 0;

                    for (orig_idx, config) in configs.iter() {
                        if config.grid_index[1] != current_row_idx {
                            if !current_row.is_empty() {
                                rows.push(current_row);
                            }
                            current_row = Vec::new();
                            current_row_idx = config.grid_index[1];
                        }
                        current_row.push((*orig_idx, *config));
                    }
                    if !current_row.is_empty() {
                        rows.push(current_row);
                    }

                    for row in rows {
                        ui.horizontal(|ui| {
                            for (orig_idx, config) in row {
                                let card_width = 80.0;
                                let card_height = 110.0;
                                
                                ui.allocate_ui_with_layout(
                                    Vec2::new(card_width, card_height),
                                    egui::Layout::top_down(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("×").clicked() {
                                            delete_idx = Some(orig_idx);
                                        }
                                        
                                        let box_size = Vec2::new(60.0, 60.0);
                                        let (rect, response) = ui.allocate_exact_size(box_size, Sense::click());
                                        
                                        let color = Color32::from_rgba_unmultiplied(
                                            config.color[0], config.color[1], 
                                            config.color[2], config.color[3]
                                        );
                                        
                                        if let Some(icon) = &self.building_config_icons.get(orig_idx).and_then(|i| i.as_ref()) {
                                            ui.painter().image(icon.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                                        } else {
                                            ui.painter().rect_filled(rect, 4.0, color);
                                        }
                                        
                                        ui.label(&config.name);
                                        
                                        if response.clicked() {
                                            self.editing_building_idx = Some(orig_idx);
                                        }
                                    }
                                );
                            }
                        });
                    }

                    ui.add_space(5.0);
                });
            }
        });

        if let Some(idx) = delete_idx {
            self.building_configs.remove(idx);
            self.building_config_icons.remove(idx);
            if let Some(edit_idx) = self.editing_building_idx {
                if edit_idx >= idx {
                    self.editing_building_idx = Some(edit_idx - 1);
                }
            }
        }
    }

}

impl eframe::App for MapEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("control").resizable(false).default_width(320.0).show(ctx, |ui| {
            ui.style_mut().spacing.item_spacing.y = 8.0;
            ui.vertical_centered_justified(|ui| { ui.heading("MINKE 策略编辑器"); });

            // 侧边栏移除了 "当前状态监视"，改为悬浮绘制

            ui.separator();
            ui.columns(6, |cols| {
                cols[0].vertical_centered_justified(|ui| { ui.selectable_value(&mut self.mode, EditMode::Terrain, "地形"); });
                cols[1].vertical_centered_justified(|ui| { ui.selectable_value(&mut self.mode, EditMode::Building, "布局"); });
                cols[2].vertical_centered_justified(|ui| { ui.selectable_value(&mut self.mode, EditMode::Upgrade, "升级"); });
                cols[3].vertical_centered_justified(|ui| { ui.selectable_value(&mut self.mode, EditMode::Demolish, "拆除"); });
                cols[4].vertical_centered_justified(|ui| { ui.selectable_value(&mut self.mode, EditMode::BuildingConfig, "建筑"); });
                cols[5].vertical_centered_justified(|ui| { ui.selectable_value(&mut self.mode, EditMode::PrepActions, "准备"); });
            });

            if self.mode == EditMode::Terrain {
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

                ui.add_space(10.0);
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("网格和镜头设置:");
                    ui.horizontal(|ui| { 
                        ui.label("网格宽:"); ui.add(egui::DragValue::new(&mut self.grid_width).speed(0.1)); 
                        ui.label("网格高:"); ui.add(egui::DragValue::new(&mut self.grid_height).speed(0.1)); 
                    });
                    ui.horizontal(|ui| {
                        ui.label("偏移 X:"); ui.add(egui::DragValue::new(&mut self.offset_x).speed(1.0));
                        ui.label("偏移 Y:"); ui.add(egui::DragValue::new(&mut self.offset_y).speed(1.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("底图高度:"); ui.add(egui::DragValue::new(&mut self.map_bottom).speed(1.0));
                        ui.label("底图宽度:"); ui.add(egui::DragValue::new(&mut self.map_right).speed(1.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("网格行列:");
                        if ui.add(egui::DragValue::new(&mut self.grid_rows)).changed() { self.resize_grids(); }
                        if ui.add(egui::DragValue::new(&mut self.grid_cols)).changed() { self.resize_grids(); }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("镜头速度上:"); ui.add(egui::DragValue::new(&mut self.camera_speed_up).speed(0.1));
                        ui.label("镜头速度下:"); ui.add(egui::DragValue::new(&mut self.camera_speed_down).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("镜头速度左:"); ui.add(egui::DragValue::new(&mut self.camera_speed_left).speed(0.1));
                        ui.label("镜头速度右:"); ui.add(egui::DragValue::new(&mut self.camera_speed_right).speed(0.1));
                    });
                    ui.vertical_centered_justified(|ui| { if ui.button("加载自定义地图底图").clicked() { self.pick_and_load_image(ctx); } });
                    ui.separator();
                    ui.label("观察框安全区域 (多个矩形):");
                    ui.horizontal(|ui| {
                        if ui.button("添加区域").clicked() {
                            self.viewport_safe_areas.push(Rect::from_min_max(Pos2::ZERO, Pos2::ZERO));
                        }
                        if ui.button("清空区域").clicked() {
                            self.viewport_safe_areas.clear();
                        }
                    });
                    ui.separator();
                    let mut remove_idx = None;
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for i in 0..self.viewport_safe_areas.len() {
                            ui.horizontal(|ui| {
                                ui.label(format!("区域{}:", i));
                                ui.label("X1:"); ui.add(egui::DragValue::new(&mut self.viewport_safe_areas[i].min.x).speed(1.0));
                                ui.label("Y1:"); ui.add(egui::DragValue::new(&mut self.viewport_safe_areas[i].min.y).speed(1.0));
                                ui.label("X2:"); ui.add(egui::DragValue::new(&mut self.viewport_safe_areas[i].max.x).speed(1.0));
                                ui.label("Y2:"); ui.add(egui::DragValue::new(&mut self.viewport_safe_areas[i].max.y).speed(1.0));
                                if ui.button("×").clicked() { remove_idx = Some(i); }
                            });
                        }
                    });
                    if let Some(idx) = remove_idx {
                        self.viewport_safe_areas.remove(idx);
                    }
                });

                ui.add_space(10.0);

                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("数据存取:");
                    ui.vertical_centered_justified(|ui| {
                        ui.label("地图名称:");
                        ui.text_edit_singleline(&mut self.map_filename);
                        ui.separator();
                        
                        if ui.button("导出全部数据").clicked() {
                            self.export_terrain();
                            self.export_buildings();
                            let map_name = self.map_filename.split('.').next().unwrap_or("地图");
                            let export_dir = PathBuf::from("output").join(map_name);
                            let _ = fs::create_dir_all(&export_dir);
                            let out = export_dir.join(format!("{}防御塔列表.json", map_name));
                            if let Ok(json) = serde_json::to_string_pretty(&self.building_configs) { let _ = fs::write(out, json); }
                        }
                        if ui.button("导入地形文件").clicked() { self.import_terrain(); }
                        if ui.button("导入策略文件").clicked() { self.import_buildings(); }
                        if ui.button("导入防御塔列表").clicked() { self.import_building_configs(ctx); }
                    });
                });

            } else if self.mode == EditMode::Building {
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("波次设置:");
                    ui.horizontal(|ui| {
                        ui.label("当前波次:");
                        ui.add(egui::DragValue::new(&mut self.current_wave_num).clamp_range(1..=100));
                        ui.checkbox(&mut self.current_is_late, "后期");
                    });
                });
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
            } else if self.mode == EditMode::Demolish { 
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
            } else if self.mode == EditMode::BuildingConfig {
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("编辑建筑:");
                    
                    if let Some(idx) = self.editing_building_idx {
                        let config = &mut self.building_configs[idx];
                        
                        ui.label("名称:");
                        ui.text_edit_singleline(&mut config.name);
                        
                        ui.separator();
                        
                        ui.label("类型:");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut config.b_type, BuildingType::Floor, "地面");
                            ui.radio_value(&mut config.b_type, BuildingType::Wall, "墙壁");
                            ui.radio_value(&mut config.b_type, BuildingType::Ceiling, "吊顶");
                        });
                        
                        ui.separator();
                        
                        ui.label("网格位置 (列, 行):");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut config.grid_index[0]).clamp_range(0..=4));
                            ui.label(",");
                            ui.add(egui::DragValue::new(&mut config.grid_index[1]).clamp_range(0..=10));
                        });
                        
                        ui.separator();
                        
                        ui.label("尺寸:");
                        ui.horizontal(|ui| {
                            ui.label("宽:");
                            ui.add(egui::DragValue::new(&mut config.width).clamp_range(1..=10));
                            ui.label("高:");
                            ui.add(egui::DragValue::new(&mut config.height).clamp_range(1..=10));
                        });
                        
                        ui.separator();
                        
                        ui.label("费用:");
                        ui.add(egui::DragValue::new(&mut config.cost).clamp_range(0..=10000));
                        
                        ui.separator();
                        
                        ui.label("颜色 (RGBA):");
                        ui.horizontal(|ui| {
                            ui.label("R:");
                            ui.add(egui::DragValue::new(&mut config.color[0]).clamp_range(0..=255).speed(1.0));
                            ui.label("G:");
                            ui.add(egui::DragValue::new(&mut config.color[1]).clamp_range(0..=255).speed(1.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("B:");
                            ui.add(egui::DragValue::new(&mut config.color[2]).clamp_range(0..=255).speed(1.0));
                            ui.label("A:");
                            ui.add(egui::DragValue::new(&mut config.color[3]).clamp_range(0..=255).speed(1.0));
                        });
                        
                        ui.separator();
                        
                        ui.label("图标路径:");
                        ui.text_edit_singleline(&mut config.icon_path);
                        
                        ui.separator();
                        
                        if ui.button("完成编辑").clicked() {
                            self.editing_building_idx = None;
                        }
                    } else {
                        ui.label("点击右侧建筑卡片进行编辑");
                    }
                });
            } else if self.mode == EditMode::PrepActions {
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("准备动作序列:");
                    ui.label("在地图加载前执行的键盘操作序列");
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        if ui.button("添加 Log").clicked() {
                            self.prep_actions.push(PrepAction::Log { msg: String::new() });
                        }
                        if ui.button("添加 KeyDown").clicked() {
                            self.prep_actions.push(PrepAction::KeyDown { key: String::new() });
                        }
                        if ui.button("添加 KeyUp").clicked() {
                            self.prep_actions.push(PrepAction::KeyUp { key: String::new() });
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("添加 Wait").clicked() {
                            self.prep_actions.push(PrepAction::Wait { ms: 100 });
                        }
                        if ui.button("添加 KeyUpAll").clicked() {
                            self.prep_actions.push(PrepAction::KeyUpAll);
                        }
                    });
                });
                
                ui.separator();
                
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label("动作列表:");
                    
                    let mut delete_idx = None;
                    let mut move_up_idx = None;
                    let mut move_down_idx = None;
                    let actions_count = self.prep_actions.len();
                    
                    egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                        if self.prep_actions.is_empty() {
                            ui.label("暂无准备动作");
                        }
                        for i in 0..actions_count {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}.", i + 1));
                                
                                match &mut self.prep_actions[i] {
                                    PrepAction::Log { msg } => {
                                        ui.label("Log:");
                                        ui.text_edit_singleline(msg);
                                    }
                                    PrepAction::KeyDown { key } => {
                                        ui.label("KeyDown:");
                                        ui.add(egui::TextEdit::singleline(key).desired_width(40.0));
                                    }
                                    PrepAction::KeyUp { key } => {
                                        ui.label("KeyUp:");
                                        ui.add(egui::TextEdit::singleline(key).desired_width(40.0));
                                    }
                                    PrepAction::Wait { ms } => {
                                        ui.label("Wait:");
                                        ui.add(egui::DragValue::new(ms).speed(10.0));
                                        ui.label("ms");
                                    }
                                    PrepAction::KeyUpAll => {
                                        ui.label("KeyUpAll");
                                    }
                                }
                                
                                if ui.small_button("↑").clicked() && i > 0 {
                                    move_up_idx = Some(i);
                                }
                                if ui.small_button("↓").clicked() && i < actions_count - 1 {
                                    move_down_idx = Some(i);
                                }
                                if ui.small_button("×").clicked() {
                                    delete_idx = Some(i);
                                }
                            });
                        }
                    });
                    
                    if let Some(idx) = delete_idx {
                        self.prep_actions.remove(idx);
                    }
                    if let Some(idx) = move_up_idx {
                        self.prep_actions.swap(idx, idx - 1);
                    }
                    if let Some(idx) = move_down_idx {
                        self.prep_actions.swap(idx, idx + 1);
                    }
                });
            }
        });

        egui::SidePanel::right("help").resizable(false).default_width(280.0).show(ctx, |ui| {
                ui.style_mut().spacing.item_spacing.y = 8.0;
                ui.vertical_centered_justified(|ui| { ui.heading("帮助"); });
                ui.separator();

                match self.mode {
                EditMode::Terrain => {
                    ui.label("【地形模式】");
                    ui.label("• 关卡预设：快速加载预设地图配置");
                    ui.label("• 地形编辑层级：选择地面/墙壁/吊顶");
                    ui.label("• 地形笔刷：绘制不同类型的地形");
                    ui.label("  - 障碍：不可通行区域");
                    ui.label("  - 平地/高台：可通行区域");
                    ui.label("• 网格和镜头设置：");
                    ui.label("  - 调整网格大小和偏移");
                    ui.label("  - 设置镜头移动速度");
                    ui.label("  - 配置观察框安全区域");
                    ui.label("• 数据存取：导出/导入地图数据");
                    ui.separator();
                    ui.label("【操作说明】");
                    ui.label("• 左键：绘制地形");
                    ui.label("• 右键：擦除地形");
                    ui.label("• 滚轮：缩放地图");
                    ui.label("• 中键拖动：平移地图");
                    ui.label("• WASD/方向键：移动观察框");
                }
                EditMode::Building => {
                    ui.label("【布局模式】");
                    ui.label("• 波次设置：设置当前编辑波次");
                    ui.label("• 选择建筑物：选择要放置的塔");
                    ui.separator();
                    ui.label("【操作说明】");
                    ui.label("• 左键：放置建筑物");
                    ui.label("• 右键：删除建筑物");
                    ui.label("• 滚轮：缩放地图");
                    ui.label("• 中键拖动：平移地图");
                }
                EditMode::Upgrade => {
                    ui.label("【升级模式】");
                    ui.label("• 添加全局升级：配置塔的升级时机");
                    ui.label("• 已配置的升级序列：查看/删除升级");
                    ui.separator();
                    ui.label("【操作说明】");
                    ui.label("• 选择目标塔和波次");
                    ui.label("• 点击[+]添加升级指令");
                    ui.label("• 点击[X]删除升级");
                }
                EditMode::Demolish => {
                    ui.label("【拆除模式】");
                    ui.label("• 拆除任务预览：查看已配置的拆除");
                    ui.separator();
                    ui.label("【操作说明】");
                    ui.label("• 在地图上右键点击塔");
                    ui.label("• 添加拆除任务");
                    ui.label("• 点击[X]删除拆除");
                }
                EditMode::BuildingConfig => {
                    ui.label("【建筑配置模式】");
                    ui.label("• 管理建筑物的属性配置");
                    ui.label("• 设置名称、类型、颜色等");
                    ui.label("• 导出/导入配置列表");
                    ui.separator();
                    ui.label("【操作说明】");
                    ui.label("• 左侧：建筑列表");
                    ui.label("• 右侧：编辑建筑信息");
                    ui.label("• 点击卡片编辑建筑");
                }
                EditMode::PrepActions => {
                    ui.label("【准备动作模式】");
                    ui.label("• 配置地图加载前的键盘操作");
                    ui.label("• Log: 输出日志信息");
                    ui.label("• KeyDown: 按下按键");
                    ui.label("• KeyUp: 释放按键");
                    ui.label("• Wait: 等待指定毫秒");
                    ui.label("• KeyUpAll: 释放所有按键");
                    ui.separator();
                    ui.label("【操作说明】");
                    ui.label("• 点击按钮添加动作");
                    ui.label("• 使用↑↓调整顺序");
                    ui.label("• 点击×删除动作");
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.mode == EditMode::BuildingConfig {
                self.show_building_config_ui(ui);
                return;
            }

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
            
            // 观察框移动控制
            if let Some(tex) = &self.texture {
                let _map_width = tex.size_vec2().x;
                let _map_height = tex.size_vec2().y;
                
                // 获取时间增量（秒）
                let dt = ctx.input(|i| i.stable_dt);
                
                // 计算新的位置
                let mut new_pos = self.viewport_pos;
                if input.key_down(egui::Key::W) || input.key_down(egui::Key::ArrowUp) {
                    new_pos.y -= self.camera_speed_up * dt;
                }
                if input.key_down(egui::Key::S) || input.key_down(egui::Key::ArrowDown) {
                    new_pos.y += self.camera_speed_down * dt;
                }
                if input.key_down(egui::Key::A) || input.key_down(egui::Key::ArrowLeft) {
                    new_pos.x -= self.camera_speed_left * dt;
                }
                if input.key_down(egui::Key::D) || input.key_down(egui::Key::ArrowRight) {
                    new_pos.x += self.camera_speed_right * dt;
                }
                
                // 检查新位置是否在任何安全区域内
                let is_valid = self.viewport_safe_areas.iter().any(|area| {
                    new_pos.x >= area.min.x && new_pos.x <= area.max.x &&
                    new_pos.y >= area.min.y && new_pos.y <= area.max.y
                });
                
                // 如果有效，则更新位置
                if is_valid {
                    self.viewport_pos = new_pos;
                }
            }

            let origin = panel_rect.min + self.pan + Vec2::new(self.offset_x * self.zoom, self.offset_y * self.zoom);
            let z_grid_width = self.grid_width * self.zoom;
            let z_grid_height = self.grid_height * self.zoom;

            if let Some(tex) = &self.texture {
                painter.image(tex.id(), Rect::from_min_size(panel_rect.min + self.pan, tex.size_vec2() * self.zoom), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }

            let layer = self.layers_data.get(&self.current_major_z).unwrap();

            let draw_layer = |grid: &Vec<Vec<i8>>, layer_type: BuildingType, is_active: bool| {
                for r in 0..self.grid_rows {
                    for c in 0..self.grid_cols {
                        let val = grid[r][c];
                        if val < -1 { continue; } 

                        let rect = Rect::from_min_size(origin + Vec2::new(c as f32 * z_grid_width, r as f32 * z_grid_height), Vec2::new(z_grid_width, z_grid_height)).shrink(0.5);
                        
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

            let t_current = get_time_value(self.current_wave_num, self.current_is_late);
            let highlight_target_name = if self.mode == EditMode::Upgrade {
                Some(self.building_templates[self.selected_upgrade_target_idx].name.clone())
            } else { None };

            for b in &self.placed_buildings {
                let t_create = get_time_value(b.wave_num, b.is_late);
                let t_demolish = self.get_building_demolish_time(b.uid);
                let alpha_mult = if t_current >= t_demolish { 0.05 } else if t_current < t_create { 0.3 } else { 1.0 };
                let rect = Rect::from_min_size(origin + Vec2::new(b.grid_x as f32 * z_grid_width, b.grid_y as f32 * z_grid_height), Vec2::new(b.width as f32 * z_grid_width, b.height as f32 * z_grid_height));
                
                let temp = self.building_templates.iter().find(|t| t.name == b.template_name);
                if let Some(t) = temp {
                    let tint = Color32::from_white_alpha((255.0 * alpha_mult) as u8);
                    if let Some(icon) = &t.icon { painter.image(icon.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint); }
                    else { painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(b.color.r(), b.color.g(), b.color.b(), (b.color.a() as f32 * alpha_mult) as u8)); }
                }
                
                if alpha_mult > 0.1 {
                    let stroke_alpha = (180.0 * alpha_mult) as u8;
                    painter.rect_stroke(rect, 1.5, Stroke::new(1.5, Color32::from_black_alpha(stroke_alpha)));
                    painter.text(
    rect.min + Vec2::new(2.0, 2.0), 
    Align2::LEFT_TOP, 
    format!("W{}{}", b.wave_num, if b.is_late { "L" } else { "" }), 
    FontId::proportional(18.0 * self.zoom.max(1.0)), 
    Color32::BLACK // 改成红色
);
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

            // 🔥 核心修改：输入隔离与交互逻辑
            // 只有当鼠标悬停在中央画布区域时，才处理地图交互
            if response.hovered() {
                if let Some(pos) = input.pointer.hover_pos() {
                    let rel = pos - origin; 
                    let (cx, ry) = ((rel.x / z_grid_width).floor() as i32, (rel.y / z_grid_height).floor() as i32);
                    
                    if cx >= 0 && ry >= 0 && (cx as usize) < self.grid_cols && (ry as usize) < self.grid_rows {
                        let current_grid = layer.get_grid(self.current_edit_layer_type);
                        let terrain_h = current_grid[ry as usize][cx as usize];
                        
                        let px_x = cx as f32 * self.grid_width;
                        let px_y = ry as f32 * self.grid_height;
                        
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
                    
                    // 仅当 Hovered 时处理编辑逻辑
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
                        let t = &self.building_templates[self.selected_building_idx];
                        let c = ((rel.x / z_grid_width) - (t.width as f32 / 2.0)).round() as i32;
                        let r = ((rel.y / z_grid_height) - (t.height as f32 / 2.0)).round() as i32;
                        let ghost_rect = Rect::from_min_size(origin + Vec2::new(c as f32 * z_grid_width, r as f32 * z_grid_height), Vec2::new(t.width as f32 * z_grid_width, t.height as f32 * z_grid_height));
                        
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
                            // 1. 先从地图上移除被点击的建筑
                            self.placed_buildings.retain(|b| !(px >= b.grid_x as i32 && px < (b.grid_x + b.width) as i32 && py >= b.grid_y as i32 && py < (b.grid_y + b.height) as i32));
                            
                            // 2. 然后清理无效的拆除计划（只保留那些 UID 依然存在于 placed_buildings 中的事件）
                            self.demolish_events.retain(|e| self.placed_buildings.iter().any(|b| b.uid == e.uid));
                        }
                    } else if self.mode == EditMode::Demolish {
                        let (px, py) = (cx, ry);
                        let target = self.placed_buildings.iter().find(|b| {
                            px >= b.grid_x as i32 && px < (b.grid_x + b.width) as i32 && py >= b.grid_y as i32 && py < (b.grid_y + b.height) as i32 &&
                            t_current >= get_time_value(b.wave_num, b.is_late) && t_current < self.get_building_demolish_time(b.uid)
                        });
                        if let Some(b) = target {
                            let r = Rect::from_min_size(origin + Vec2::new(b.grid_x as f32 * z_grid_width, b.grid_y as f32 * z_grid_height), Vec2::new(b.width as f32 * z_grid_width, b.height as f32 * z_grid_height));
                            painter.rect_stroke(r, 0.0, Stroke::new(3.0, Color32::YELLOW));
                            if response.clicked_by(egui::PointerButton::Primary) && !self.demolish_events.iter().any(|e| e.uid == b.uid) {
                                self.demolish_events.push(DemolishEvent { uid: b.uid, name: b.template_name.clone(), grid_x: b.grid_x, grid_y: b.grid_y, width: b.width, height: b.height, wave_num: self.current_wave_num, is_late: self.current_is_late });
                            }
                        }
                    }
                }
            }

            // 绘制观察框
            if let Some(tex) = &self.texture {
                let _map_width = tex.size_vec2().x;
                let _map_height = tex.size_vec2().y;
                
                let map_origin = panel_rect.min + self.pan;
                
                // 绘制观察框的实际位置
                let viewport_rect = Rect::from_min_size(
                    map_origin + self.viewport_pos * self.zoom,
                    Vec2::new(self.viewport_width * self.zoom, self.viewport_height * self.zoom)
                );
                
                // 绘制观察框（半透明红色）
                painter.rect_stroke(viewport_rect, 2.0, Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 0, 0, 200)));
                painter.rect_filled(viewport_rect, 0.0, Color32::from_rgba_unmultiplied(255, 0, 0, 30));
                
                // 绘制观察框的可移动范围边缘线（多个黄色矩形）
                for safe_area in &self.viewport_safe_areas {
                    let area_rect = Rect::from_min_max(
                        map_origin + safe_area.min.to_vec2() * self.zoom,
                        map_origin + safe_area.max.to_vec2() * self.zoom
                    );
                    painter.rect_stroke(area_rect, 2.0, Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 0, 150)));
                }
            }

            // 🔥 悬浮信息栏绘制：独立在地图上方 (最后绘制以确保最上层)
            if !self.hover_info.is_empty() && self.hover_info != "无" {
                // 在左上角绘制
                let info_pos = panel_rect.min + Vec2::new(10.0, 10.0);
                let galley = painter.layout_no_wrap(self.hover_info.clone(), FontId::new(14.0, FontFamily::Monospace), Color32::WHITE);
                
                let bg_rect = Rect::from_min_size(info_pos, galley.size() + Vec2::new(10.0, 10.0));
                painter.rect_filled(bg_rect, 5.0, Color32::from_black_alpha(180));
                painter.galley(info_pos + Vec2::new(5.0, 5.0), galley, Color32::WHITE);
            }
        });
    }
}