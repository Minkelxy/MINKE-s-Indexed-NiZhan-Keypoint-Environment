use serde::{Deserialize, Serialize};
use eframe::egui::{Color32, TextureHandle};



#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum BuildingType {
    Floor,   // 地面
    Wall,    // 墙壁
    Ceiling, // 吊顶
}

// 兼容旧数据的默认值函数
fn default_building_type() -> BuildingType { BuildingType::Floor }

#[derive(Serialize, Deserialize, Clone)]
pub struct MapMeta {
    pub grid_pixel_size: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    #[serde(default)] // 🔥 新增：兼容旧文件，若无此字段则默认为 0.0
    pub bottom: f32,  // 🔥 新增：底图高度
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LayerData {
    pub major_z: i32,
    pub name: String,
    pub elevation_grid: Vec<Vec<i8>>, 
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BuildingExport {
    pub uid: usize,
    pub name: String,
    #[serde(default = "default_building_type")]
    pub b_type: BuildingType,
    pub grid_x: usize,
    pub grid_y: usize,
    pub width: usize,
    pub height: usize,
    pub wave_num: i32,
    pub is_late: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UpgradeEvent {
    pub building_name: String, 
    pub wave_num: i32,
    pub is_late: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DemolishEvent {
    pub uid: usize,          
    pub name: String,
    pub grid_x: usize,
    pub grid_y: usize,
    pub width: usize,
    pub height: usize,
    pub wave_num: i32,
    pub is_late: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MapTerrainExport {
    pub map_name: String,
    pub meta: MapMeta,
    pub layers: Vec<LayerData>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MapBuildingsExport {
    pub map_name: String,
    pub buildings: Vec<BuildingExport>,
    #[serde(default)]
    pub upgrades: Vec<UpgradeEvent>,
    #[serde(default)]
    pub demolishes: Vec<DemolishEvent>, 
}

#[derive(Deserialize, Clone)]
pub struct BuildingConfig {
    pub name: String,
    #[serde(default = "default_building_type")] 
    pub b_type: BuildingType,
    pub width: usize,
    pub height: usize,
    pub color: [u8; 4],
    pub icon_path: String,
}

#[derive(Deserialize, Clone)]
pub struct MapPreset {
    pub name: String,
    pub image_path: String,
    pub terrain_path: String,
}

#[derive(Clone)]
pub struct BuildingTemplate {
    pub name: String,
    pub b_type: BuildingType,
    pub width: usize,
    pub height: usize,
    pub color: Color32,
    pub icon: Option<TextureHandle>,
}

#[derive(Clone)]
pub struct PlacedBuilding {
    pub uid: usize,
    pub template_name: String,
    pub b_type: BuildingType,
    pub grid_x: usize,
    pub grid_y: usize,
    pub width: usize,
    pub height: usize,
    pub color: Color32,
    pub wave_num: i32,
    pub is_late: bool,
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum EditMode { Terrain, Building, Upgrade, Demolish }