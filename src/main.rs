use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use image::io::Reader as ImageReader;
use std::fs;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "MINKE's Indexed Ni-Zhan Keypoint Environment (MINKE)",
        options,
        Box::new(|cc| {
            // --- 动态读取 Windows 系统字体 ---
            let mut fonts = egui::FontDefinitions::default();
            
            // 直接读取 C 盘 Windows 字体目录下的黑体 (simhei.ttf)
            // 这种方式不会增加 exe 的体积
            if let Ok(font_data) = fs::read("C:\\Windows\\Fonts\\simhei.ttf") {
                fonts.font_data.insert("system_font".to_owned(), egui::FontData::from_owned(font_data));
                
                // 将读取到的字体设置为全局首选
                fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "system_font".to_owned());
                fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().insert(0, "system_font".to_owned());
            } else {
                eprintln!("警告: 找不到 C:/Windows/Fonts/simhei.ttf，中文可能显示为方块。");
            }

            cc.egui_ctx.set_fonts(fonts);

            Box::new(MapEditor::new(cc))
        }),
    )
}

struct MapEditor {
    texture: Option<TextureHandle>,
    
    // 地图元数据 (相对于原始图片的尺寸)
    grid_size: f32,
    offset_x: f32,
    offset_y: f32,

    grid_rows: usize,
    grid_cols: usize,
    grid_data: Vec<Vec<bool>>, // true=可建, false=障碍

    // --- 新增：摄像机控制 (Pan & Zoom) ---
    zoom: f32,      // 缩放比例 (默认 1.0)
    pan: Vec2,      // 视口平移量
}

impl MapEditor {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let rows = 40;
        let cols = 40;
        let mut editor = Self {
            texture: None,
            grid_size: 32.0,
            offset_x: 0.0,
            offset_y: 0.0,
            grid_rows: rows,
            grid_cols: cols,
            grid_data: vec![vec![true; cols]; rows],
            
            // 默认无缩放，无偏移
            zoom: 1.0,
            pan: Vec2::ZERO, 
        };
        // 记得在项目根目录放一张 test_map.png 用于测试
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

    // [核心算法] 将屏幕像素坐标转换为游戏画布坐标
    fn screen_to_canvas(&self, screen_pos: Pos2, rect_min: Pos2) -> Pos2 {
        let rel = screen_pos - rect_min - self.pan;
        Pos2::new(rel.x / self.zoom, rel.y / self.zoom)
    }

    // [核心算法] 将游戏画布坐标转换为屏幕渲染坐标
    fn canvas_to_screen(&self, canvas_pos: Pos2, rect_min: Pos2) -> Pos2 {
        rect_min + self.pan + Vec2::new(canvas_pos.x * self.zoom, canvas_pos.y * self.zoom)
    }
}

impl eframe::App for MapEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        
        // --- 左侧控制面板 ---
        egui::SidePanel::left("control_panel").min_width(250.0).show(ctx, |ui| {
            ui.heading("MINKE 地图数据化引擎");
            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label("🛠 网格基准设定");
                ui.horizontal(|ui| { ui.label("网格大小:"); ui.add(egui::DragValue::new(&mut self.grid_size).speed(0.1)); });
                ui.horizontal(|ui| { ui.label("X 偏移:"); ui.add(egui::DragValue::new(&mut self.offset_x).speed(0.5)); });
                ui.horizontal(|ui| { ui.label("Y 偏移:"); ui.add(egui::DragValue::new(&mut self.offset_y).speed(0.5)); });
            });

            ui.add_space(10.0);
            ui.group(|ui| {
                ui.label("🔍 视图控制");
                ui.label(format!("当前缩放: {:.0}%", self.zoom * 100.0));
                if ui.button("重置视图 (100%)").clicked() {
                    self.zoom = 1.0;
                    self.pan = Vec2::ZERO;
                }
            });

            ui.add_space(20.0);
            ui.label("🎮 操作说明:");
            ui.label("• 鼠标中键按住: 拖动地图");
            ui.label("• 鼠标滚轮: 缩放地图");
            ui.label("• 左键涂抹: 设为障碍 (红色)");
            ui.label("• 右键涂抹: 设为可建 (绿色)");
        });

        // --- 中央画布区 ---
        egui::CentralPanel::default().show(ctx, |ui| {
            // 获取整个窗口的输入状态（用于滚轮缩放）
            let input = ui.input(|i| i.clone());
            let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let rect_min = response.rect.min;

            // --- 1. 处理拖动与缩放 (Pan & Zoom) ---
            // 鼠标悬停在画布上时，滚轮进行缩放
            if response.hovered() {
                let scroll_delta = input.raw_scroll_delta.y;
                if scroll_delta != 0.0 {
                    let old_zoom = self.zoom;
                    self.zoom *= 1.0 + (scroll_delta * 0.001); // 缩放灵敏度
                    self.zoom = self.zoom.clamp(0.1, 10.0);    // 限制缩放范围在 10% 到 1000%
                    
                    // 以鼠标当前位置为中心进行缩放修正
                    if let Some(mouse_pos) = input.pointer.hover_pos() {
                        let rel_mouse = mouse_pos - rect_min - self.pan;
                        self.pan -= rel_mouse * (self.zoom / old_zoom - 1.0);
                    }
                }
            }

            // 鼠标中键（滚轮按下）拖动平移
            if input.pointer.button_down(egui::PointerButton::Middle) {
                self.pan += input.pointer.delta();
            }


            // --- 2. 绘制底层游戏截图 ---
            if let Some(texture) = &self.texture {
                let img_size = Vec2::new(texture.size()[0] as f32, texture.size()[1] as f32);
                let img_canvas_pos = Pos2::ZERO; // 图片在画布的 (0,0) 位置
                let img_screen_min = self.canvas_to_screen(img_canvas_pos, rect_min);
                // 图片在屏幕上的尺寸也要乘以 zoom
                let img_screen_rect = Rect::from_min_size(img_screen_min, img_size * self.zoom); 
                painter.image(texture.id(), img_screen_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }

            // --- 3. 处理涂抹交互 (坐标转换) ---
            if response.dragged() || response.clicked() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    // 左键或右键才能涂抹（排除中键拖动）
                    let is_drawing = input.pointer.button_down(egui::PointerButton::Primary) || input.pointer.button_down(egui::PointerButton::Secondary);
                    
                    if is_drawing {
                        // 将屏幕鼠标坐标，逆算回原始尺寸的画布坐标
                        let canvas_pos = self.screen_to_canvas(pointer_pos, rect_min);
                        
                        // 计算网格索引 (扣除偏移量)
                        let rel_x = canvas_pos.x - self.offset_x;
                        let rel_y = canvas_pos.y - self.offset_y;

                        if rel_x >= 0.0 && rel_y >= 0.0 {
                            let col = (rel_x / self.grid_size).floor() as usize;
                            let row = (rel_y / self.grid_size).floor() as usize;

                            if row < self.grid_rows && col < self.grid_cols {
                                if input.pointer.button_down(egui::PointerButton::Primary) {
                                    self.grid_data[row][col] = false; // 左键=障碍
                                } else {
                                    self.grid_data[row][col] = true;  // 右键=可建
                                }
                            }
                        }
                    }
                }
            }

            // --- 4. 渲染网格数据与线框 (叠加 Zoom 和 Pan) ---
            let canvas_origin = Pos2::new(self.offset_x, self.offset_y);
            let screen_origin = self.canvas_to_screen(canvas_origin, rect_min);
            let zoomed_grid_size = self.grid_size * self.zoom; // 缩放后的格子大小

            // 绘制方块
            for r in 0..self.grid_rows {
                for c in 0..self.grid_cols {
                    let cell_screen_pos = screen_origin + Vec2::new(c as f32 * zoomed_grid_size, r as f32 * zoomed_grid_size);
                    let cell_rect = Rect::from_min_size(cell_screen_pos, Vec2::new(zoomed_grid_size, zoomed_grid_size));

                    // 仅在屏幕视口内的方块才绘制 (提升性能)
                    if response.rect.intersects(cell_rect) {
                        let color = if self.grid_data[r][c] {
                            Color32::from_rgba_unmultiplied(0, 255, 0, 30)
                        } else {
                            Color32::from_rgba_unmultiplied(255, 0, 0, 100)
                        };
                        painter.rect_filled(cell_rect, 0.0, color);
                    }
                }
            }

            // 绘制网格线
            let grid_color = Color32::from_white_alpha(50);
            let stroke = Stroke::new(1.0, grid_color);

            for r in 0..=self.grid_rows {
                let y = screen_origin.y + r as f32 * zoomed_grid_size;
                let start = Pos2::new(screen_origin.x, y);
                let end = Pos2::new(screen_origin.x + self.grid_cols as f32 * zoomed_grid_size, y);
                painter.line_segment([start, end], stroke);
            }

            for c in 0..=self.grid_cols {
                let x = screen_origin.x + c as f32 * zoomed_grid_size;
                let start = Pos2::new(x, screen_origin.y);
                let end = Pos2::new(x, screen_origin.y + self.grid_rows as f32 * zoomed_grid_size);
                painter.line_segment([start, end], stroke);
            }
        });
    }
}