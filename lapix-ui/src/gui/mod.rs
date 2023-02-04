use crate::wrapped_image::WrappedImage;
use crate::{Effect, UiEvent};
use lapix_core::{Event, Point, Size, Tool};
use macroquad::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

mod toolbar;
use toolbar::Toolbar;

mod layers;
use layers::LayersPanel;

pub struct Resources;

impl Resources {
    pub fn tool_icon(tool: Tool) -> &'static [u8] {
        match tool {
            Tool::Brush => include_bytes!("../../res/icon/pencil.png"),
            Tool::Bucket => include_bytes!("../../res/icon/bucket.png"),
            Tool::Eraser => include_bytes!("../../res/icon/eraser.png"),
            Tool::Eyedropper => include_bytes!("../../res/icon/eyedropper.png"),
            Tool::Line => include_bytes!("../../res/icon/line.png"),
        }
    }
}

fn rgba_to_rgb_u8(color: [u8; 4]) -> [u8; 3] {
    [color[0], color[1], color[2]]
}

fn draw_texture_helper(texture: Texture2D, x: f32, y: f32, scale: f32) {
    let w = texture.width();
    let h = texture.height();

    let params = DrawTextureParams {
        dest_size: Some(Vec2 {
            x: w * scale,
            y: h * scale,
        }),
        ..Default::default()
    };

    draw_texture_ex(texture, x, y, WHITE, params);
}

pub struct Gui {
    toolbar: Toolbar,
    layers_panel: LayersPanel,
    cursors: CursorSet,
    canvas_w_str: String,
    canvas_h_str: String,
    brush: [u8; 3],
    brush_alpha: String,
    last_file: Option<PathBuf>,
}

impl Gui {
    pub fn new(canvas_size: Size<u16>) -> Self {
        Self {
            toolbar: Toolbar::new(),
            layers_panel: LayersPanel::new(),
            cursors: CursorSet::new(),
            canvas_w_str: canvas_size.x.to_string(),
            canvas_h_str: canvas_size.y.to_string(),
            brush: [0, 0, 0],
            brush_alpha: "255".to_owned(),
            last_file: None,
        }
    }

    pub fn draw_cursor(&self, selected_tool: Tool) {
        if let Some(cursor) = self.cursors.0.get(&selected_tool) {
            cursor.draw();
        }
    }

    pub fn sync(
        &mut self,
        main_color: [u8; 4],
        num_layers: usize,
        active_layer: usize,
        layers_vis: Vec<bool>,
        layers_alpha: Vec<u8>,
    ) {
        self.brush = rgba_to_rgb_u8(main_color);
        self.brush_alpha = main_color[3].to_string();
        self.layers_panel
            .sync(num_layers, active_layer, layers_vis, layers_alpha);
    }

    pub fn update(&mut self) -> Vec<Effect> {
        let mut events = Vec::new();

        egui_macroquad::ui(|egui_ctx| {
            let mut canvas_panel_events = self.update_canvas_panel(egui_ctx);
            events.append(&mut canvas_panel_events);

            let mut toolbar_events = self.toolbar.update(egui_ctx);
            events.append(&mut toolbar_events);

            let mut layers_events = self.layers_panel.update(egui_ctx);
            events.append(&mut layers_events);

            egui_ctx.output().cursor_icon = egui::CursorIcon::None;
        });

        if !events.is_empty() {
            events.push(UiEvent::GuiInteraction.into());
        }

        events
    }

    fn update_canvas_panel(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();

        egui::Window::new("Canvas").show(egui_ctx, |ui| {
            ui.horizontal(|ui| {
                let label = ui.label("w:");
                ui.add(
                    egui::widgets::TextEdit::singleline(&mut self.canvas_w_str).desired_width(30.0),
                )
                .labelled_by(label.id);
                let label = ui.label("h:");
                ui.add(
                    egui::widgets::TextEdit::singleline(&mut self.canvas_h_str).desired_width(30.0),
                )
                .labelled_by(label.id);
            });

            let btn = ui.button("New canvas");
            if btn.clicked() {
                if let (Ok(w), Ok(h)) = (self.canvas_w_str.parse(), self.canvas_h_str.parse()) {
                    events.push(Event::ResizeCanvas(w, h).into());
                }
            }

            ui.horizontal(|ui| {
                let colorpicker = ui.color_edit_button_srgb(&mut self.brush);
                let label = ui.label("a:");
                let text_edit = ui
                    .add(
                        egui::widgets::TextEdit::singleline(&mut self.brush_alpha)
                            .desired_width(30.0),
                    )
                    .labelled_by(label.id);

                if colorpicker.changed() || text_edit.changed() {
                    let color = [
                        self.brush[0],
                        self.brush[1],
                        self.brush[2],
                        self.brush_alpha.parse().unwrap_or(255),
                    ];
                    events.push(Event::SetMainColor(color).into());
                }
            });

            let btn = ui.button("Erase canvas");
            if btn.clicked() {
                events.push(Event::ClearCanvas.into());
            }
            let btn = ui.button("Save");
            if btn.clicked() {
                let mut dialog = rfd::FileDialog::new();

                if let Some(dir) = self.last_file.as_ref().and_then(|p| p.parent()) {
                    dialog = dialog.set_directory(dir);
                }

                if let Some(path) = dialog.save_file() {
                    self.last_file = Some(path.clone());
                    events.push(Event::Save(path).into());
                }
            }
            let btn = ui.button("Open");
            if btn.clicked() {
                let mut dialog = rfd::FileDialog::new();

                if let Some(dir) = self.last_file.as_ref().and_then(|p| p.parent()) {
                    dialog = dialog.set_directory(dir);
                }

                if let Some(path) = dialog.pick_file() {
                    self.last_file = Some(path.clone());
                    events.push(Event::OpenFile(path).into());
                }
            }
        });

        if egui_ctx.is_pointer_over_area() {
            events.push(Effect::UiEvent(UiEvent::MouseOverGui));
        }

        events
    }
}

pub struct CursorSet(HashMap<Tool, ToolCursor>);

impl CursorSet {
    pub fn new() -> Self {
        let tools = [
            (Tool::Brush, (0., 0.).into()),
            (Tool::Bucket, (0., 3.).into()),
            (Tool::Eraser, (0., 0.).into()),
            (Tool::Eyedropper, (0., 0.).into()),
            (Tool::Line, (0., 0.).into()),
        ];

        Self(
            tools
                .iter()
                .map(|(t, offset)| (*t, ToolCursor::new(*t, *offset)))
                .collect(),
        )
    }
}

pub struct ToolCursor {
    texture: Texture2D,
    offset: Point<f32>,
}

impl ToolCursor {
    pub fn new(tool: Tool, offset: Point<f32>) -> Self {
        let bytes = Resources::tool_icon(tool);
        let texture = Texture2D::from_file_with_format(bytes, None);

        Self { texture, offset }
    }

    pub fn draw(&self) {
        let (x, y) = mouse_position();
        draw_texture_helper(
            self.texture,
            x + self.offset.x,
            y - self.texture.height() + self.offset.y,
            1.,
        )
    }
}