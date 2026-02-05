use serde::{Deserialize, Serialize};
use eframe::egui::{Color32, TextureHandle};

#[derive(Serialize, Deserialize, Clone)]
pub struct MapMeta {
    pub grid_pixel_size: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    #[serde(default)]
    pub bottom: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Hash, Eq)]
pub enum BuildingType {
    Floor,   // 地面
    Wall,    // 墙壁
    Ceiling, // 吊顶
}

fn default_building_type() -> BuildingType { BuildingType::Floor }
fn default_grid() -> Vec<Vec<i8>> { Vec::new() }

#[derive(Serialize, Deserialize, Clone)]
pub struct LayerData {
    pub major_z: i32,
    pub name: String,
    
    #[serde(default = "default_grid")]
    pub floor_grid: Vec<Vec<i8>>,
    
    #[serde(default = "default_grid")]
    pub wall_grid: Vec<Vec<i8>>,
    
    #[serde(default = "default_grid")]
    pub ceiling_grid: Vec<Vec<i8>>,

    // 🔥 新增：兼容旧版本 JSON 的字段
    // 标记为 Option 且跳过序列化（只读不存）
    #[serde(default, skip_serializing)]
    pub elevation_grid: Option<Vec<Vec<i8>>>,
}

impl LayerData {
    // 辅助函数：根据类型获取只读网格
    pub fn get_grid(&self, b_type: BuildingType) -> &Vec<Vec<i8>> {
        match b_type {
            BuildingType::Floor => &self.floor_grid,
            BuildingType::Wall => &self.wall_grid,
            BuildingType::Ceiling => &self.ceiling_grid,
        }
    }

    // 辅助函数：根据类型获取可变网格
    pub fn get_grid_mut(&mut self, b_type: BuildingType) -> &mut Vec<Vec<i8>> {
        match b_type {
            BuildingType::Floor => &mut self.floor_grid,
            BuildingType::Wall => &mut self.wall_grid,
            BuildingType::Ceiling => &mut self.ceiling_grid,
        }
    }

    // 🔥 新增：数据迁移函数
    // 如果读取到了旧版的 elevation_grid，将其移动到 floor_grid
    pub fn normalize(&mut self) {
        if let Some(old_grid) = self.elevation_grid.take() {
            // 如果 floor_grid 是空的（说明是旧文件），则迁移
            if self.floor_grid.is_empty() {
                self.floor_grid = old_grid;
                // 初始化其他层为空网格，大小将在 App 中 resize_grids 时或逻辑中统一
            }
        }
    }
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