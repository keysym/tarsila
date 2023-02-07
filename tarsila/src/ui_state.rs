use crate::gui::Gui;
use crate::keyboard::KeyboardManager;
use crate::mouse::MouseManager;
use crate::wrapped_image::WrappedImage;
use crate::{graphics, Timer};
use lapix::primitives::*;
use lapix::{Bitmap, Canvas, CanvasEffect, Event, FreeImage, Selection, State, Tool};
use macroquad::prelude::Color as MqColor;
use macroquad::prelude::{DrawTextureParams, FilterMode, Texture2D, Vec2, BLACK, SKYBLUE};
use std::default::Default;

pub const WINDOW_W: i32 = 1000;
pub const WINDOW_H: i32 = 600;
pub const CANVAS_W: u16 = 64;
pub const CANVAS_H: u16 = 64;
const CANVAS_SCALE: f32 = 8.;
const LEFT_TOOLBAR_W: u16 = 300;
const CAMERA_SPEED: f32 = 12.;
const BG_COLOR: MqColor = SKYBLUE;
const GUI_REST_MS: u64 = 100;
const SPRITESHEET_LINE_THICKNESS: f32 = 1.;
const SPRITESHEET_LINE_COLOR: MqColor = BLACK;

// Center on the space after the toolbar
const CANVAS_X: f32 = LEFT_TOOLBAR_W as f32 + ((WINDOW_W as u16 - LEFT_TOOLBAR_W) / 2) as f32
    - (CANVAS_W as f32 * CANVAS_SCALE / 2.);
const CANVAS_Y: f32 = (WINDOW_H / 2) as f32 - (CANVAS_H as f32 * CANVAS_SCALE / 2.);

#[derive(Debug, Clone)]
pub enum Effect {
    Event(Event<WrappedImage>),
    UiEvent(UiEvent),
}

impl From<Event<WrappedImage>> for Effect {
    fn from(val: Event<WrappedImage>) -> Self {
        Self::Event(val)
    }
}

impl From<UiEvent> for Effect {
    fn from(val: UiEvent) -> Self {
        Self::UiEvent(val)
    }
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    ZoomIn,
    ZoomOut,
    MoveCamera(Direction),
    MouseOverGui,
    GuiInteraction,
    Paste,
}

pub struct UiState {
    inner: State<WrappedImage>,
    gui: Gui,
    camera: Position<f32>,
    canvas_pos: Position<f32>,
    zoom: f32,
    layer_textures: Vec<Texture2D>,
    keyboard: KeyboardManager,
    mouse: MouseManager,
    mouse_over_gui: bool,
    gui_interaction_rest: Timer,
    free_image_tex: Option<Texture2D>,
}

impl Default for UiState {
    fn default() -> Self {
        let state = State::<WrappedImage>::new(CANVAS_W, CANVAS_H);
        let drawing = Texture2D::from_image(&state.canvas().inner().0);
        drawing.set_filter(FilterMode::Nearest);

        Self {
            inner: state,
            gui: Gui::new((CANVAS_W, CANVAS_H).into()),
            camera: (0., 0.).into(),
            canvas_pos: (CANVAS_X, CANVAS_Y).into(),
            zoom: 8.,
            layer_textures: vec![drawing],
            keyboard: KeyboardManager::new(),
            mouse: MouseManager::new(),
            mouse_over_gui: false,
            gui_interaction_rest: Timer::new(),
            free_image_tex: None,
        }
    }
}

impl UiState {
    pub fn drawing_mut(&mut self) -> &mut Texture2D {
        &mut self.layer_textures[self.inner.active_layer()]
    }

    pub fn drawing(&self) -> &Texture2D {
        &self.layer_textures[self.inner.active_layer()]
    }

    pub fn update(&mut self) {
        self.mouse_over_gui = false;

        self.sync_gui();
        let fx = self.gui.update();
        self.process_fx(fx);

        self.sync_mouse();
        let fx = self.mouse.update();
        self.process_fx(fx);

        let fx = self.keyboard.update();
        self.process_fx(fx);
    }

    fn process_fx(&mut self, fx: Vec<Effect>) {
        for effect in fx {
            match effect {
                Effect::UiEvent(event) => self.process_event(event),
                Effect::Event(event) => {
                    if !event.is_drawing_event() || !self.is_canvas_blocked() {
                        self.execute(event);
                    }
                }
            }
        }
    }

    fn is_canvas_blocked(&self) -> bool {
        self.mouse_over_gui || !self.gui_interaction_rest.expired()
    }

    pub fn draw(&mut self) {
        macroquad::prelude::clear_background(BG_COLOR);
        self.draw_canvas_bg();
        self.draw_canvas();
        self.draw_spritesheet_boundaries();

        if self.inner.selection().is_some() {
            self.draw_selection();
        }

        // TODO: most of this logic should be in some update method, not a draw one
        if let Some(img) = self.inner.free_image() {
            if self.free_image_tex.is_none() {
                let tex = Texture2D::from_image(&img.texture.0);
                tex.set_filter(FilterMode::Nearest);
                self.free_image_tex = Some(tex);
            }

            self.draw_free_image(&img);
        } else {
            self.free_image_tex = None;
        }

        egui_macroquad::draw();
        self.gui.draw_cursor(self.selected_tool());
    }

    /// Pass relevant UI state info to the GUI
    pub fn sync_gui(&mut self) {
        let n_layers = self.inner.num_layers();
        self.gui.sync(
            self.inner.main_color(),
            n_layers,
            self.inner.active_layer(),
            (0..n_layers)
                .map(|i| self.inner.layer(i).visible())
                .collect(),
            (0..n_layers)
                .map(|i| self.inner.layer(i).opacity())
                .collect(),
            self.inner.spritesheet(),
            self.inner
                .sprite_images()
                .into_iter()
                .map(|i| i.0)
                .collect(),
            self.inner.palette().iter().map(|c| *c).collect(),
        );
    }

    pub fn sync_mouse(&mut self) {
        let (x, y) = macroquad::prelude::mouse_position();
        let (x, y) = self.screen_to_canvas(x, y);
        let in_canvas = x >= 0
            && y >= 0
            && (x as u16) < self.canvas().width()
            && (y as u16) < self.canvas().height();
        let visible_pixel = if in_canvas {
            Some(self.visible_pixel(x as u16, y as u16))
        } else {
            None
        };

        self.mouse.sync(
            (x, y).into(),
            in_canvas,
            self.is_mouse_on_selection(),
            self.selected_tool(),
            visible_pixel,
        );
    }

    pub fn execute(&mut self, event: Event<WrappedImage>) {
        let effect = self.inner.execute(event);

        // TODO: resize and new canvas events now need to affect all layers

        match effect {
            // TODO: Texture2D is copy, so we don't need `drawing_mut` here, but
            // it would be better.
            CanvasEffect::Update => self.drawing().update(&self.canvas().inner().0),
            CanvasEffect::New | CanvasEffect::Layer => self.sync_layer_textures(),
            CanvasEffect::None => (),
        };
    }

    pub fn sync_layer_textures(&mut self) {
        for layer in 0..self.inner.num_layers() {
            self.sync_layer_texture(layer);
        }
    }

    pub fn sync_layer_texture(&mut self, index: usize) {
        let layer_img = &self.inner.layer_canvas(index).inner().0;
        let texture = Texture2D::from_image(layer_img);
        texture.set_filter(FilterMode::Nearest);

        match self.layer_textures.get_mut(index) {
            Some(tex) => *tex = texture,
            None => self.layer_textures.push(texture),
        }
    }

    pub fn process_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::ZoomIn => self.zoom_in(),
            UiEvent::ZoomOut => self.zoom_out(),
            UiEvent::MoveCamera(dir) => self.move_camera(dir),
            UiEvent::MouseOverGui => self.mouse_over_gui = true,
            UiEvent::GuiInteraction => self.gui_interaction_rest.start(GUI_REST_MS),
            UiEvent::Paste => {
                let (x, y) = macroquad::prelude::mouse_position();
                let (x, y) = self.screen_to_canvas(x, y);
                self.execute(Event::Paste(x as u16, y as u16));
            }
        }
    }

    pub fn visible_pixel(&self, x: u16, y: u16) -> [u8; 4] {
        self.inner.visible_pixel(x, y)
    }

    pub fn camera(&self) -> Position<f32> {
        self.camera
    }

    pub fn canvas(&self) -> &Canvas<WrappedImage> {
        self.inner.canvas()
    }

    pub fn canvas_pos(&self) -> Position<f32> {
        self.canvas_pos
    }

    pub fn canvas_actual_size(&self) -> Size<f32> {
        (
            self.inner.canvas().width() as f32 * self.zoom,
            self.inner.canvas().height() as f32 * self.zoom,
        )
            .into()
    }

    pub fn selected_tool(&self) -> Tool {
        self.inner.selected_tool()
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn zoom_in(&mut self) {
        self.zoom *= 2.;
        self.camera.x *= 2.;
        self.camera.y *= 2.;
    }

    pub fn zoom_out(&mut self) {
        self.zoom /= 2.;
        self.camera.x /= 2.;
        self.camera.y /= 2.;
    }

    pub fn move_camera(&mut self, direction: Direction) {
        let speed = CAMERA_SPEED;

        if !self.is_camera_off(direction) {
            match direction {
                Direction::Up => self.camera.y -= speed,
                Direction::Down => self.camera.y += speed,
                Direction::Left => self.camera.x -= speed,
                Direction::Right => self.camera.x += speed,
            }
        }
    }

    fn is_camera_off(&self, direction: Direction) -> bool {
        let buffer = 20.;
        let canvas_size = self.canvas_actual_size();
        let canvas_pos = self.canvas_pos;
        let camera = self.camera;
        let win_w = WINDOW_W as f32;
        let win_h = WINDOW_H as f32;

        match direction {
            Direction::Up => canvas_pos.y - camera.y > win_h - buffer,
            Direction::Down => camera.y > canvas_pos.y + canvas_size.y - buffer,
            Direction::Left => canvas_pos.x - camera.x > win_w - buffer,
            Direction::Right => camera.x > canvas_pos.x + canvas_size.x - buffer,
        }
    }

    pub fn draw_canvas_bg(&self) {
        let scale = self.zoom();

        let x = self.canvas_pos().x - self.camera().x;
        let y = self.canvas_pos().y - self.camera().y;
        let w = self.drawing().width() * scale;
        let h = self.drawing().height() * scale;

        let side = 4. * scale;

        // TODO: optimize this by storing a rendered texture that contains all
        // BG rectangles
        let bg1 = MqColor::new(0.875, 0.875, 0.875, 1.);
        let bg2 = MqColor::new(0.75, 0.75, 0.75, 1.);
        for i in 0..(w / side + 1.) as usize {
            for j in 0..(h / side + 1.) as usize {
                let cur_w = i as f32 * side;
                let cur_h = j as f32 * side;
                let next_w = (i + 1) as f32 * side;
                let next_h = (j + 1) as f32 * side;
                let x = x + i as f32 * side;
                let y = y + j as f32 * side;
                let w = if next_w <= w { side } else { w - cur_w };
                let h = if next_h <= h { side } else { h - cur_h };
                let color = if (i + j) % 2 == 0 { bg1 } else { bg2 };
                macroquad::prelude::draw_rectangle(x, y, w, h, color);
            }
        }
    }

    pub fn draw_canvas(&self) {
        for i in 0..self.inner.num_layers() {
            if !self.inner.layer(i).visible() {
                continue;
            }

            let texture = self.layer_textures[i];
            let w = texture.width();
            let h = texture.height();

            let x = self.canvas_pos().x - self.camera().x;
            let y = self.canvas_pos().y - self.camera().y;
            let scale = self.zoom();

            let params = DrawTextureParams {
                dest_size: Some(Vec2 {
                    x: w * scale,
                    y: h * scale,
                }),
                ..Default::default()
            };

            let color = [255, 255, 255, self.inner.layer(i).opacity()];
            macroquad::prelude::draw_texture_ex(texture, x, y, color.into(), params);
        }
    }

    pub fn draw_spritesheet_boundaries(&self) {
        for i in 0..self.inner.spritesheet().x {
            for j in 0..self.inner.spritesheet().y {
                let x0 = self.canvas_pos().x - self.camera().x;
                let y0 = self.canvas_pos().y - self.camera().y;
                let scale = self.zoom();
                let w = self.canvas().width() as f32 / self.inner.spritesheet().x as f32 * scale;
                let h = self.canvas().height() as f32 / self.inner.spritesheet().y as f32 * scale;
                let x = x0 + i as f32 * w;
                let y = y0 + j as f32 * h;

                macroquad::prelude::draw_rectangle_lines(
                    x,
                    y,
                    w,
                    h,
                    SPRITESHEET_LINE_THICKNESS,
                    SPRITESHEET_LINE_COLOR,
                );
            }
        }
    }

    pub fn draw_selection(&self) {
        let rect = match self.inner.selection() {
            Some(Selection::FreeImage) => self.inner.free_image().unwrap().rect,
            Some(Selection::Canvas(rect)) => rect.into(),
            _ => return,
        };

        let x0 = self.canvas_pos().x - self.camera().x;
        let y0 = self.canvas_pos().y - self.camera().y;
        let scale = self.zoom();
        let r = Rect {
            x: (x0 + rect.x as f32 * scale) as i32,
            y: (y0 + rect.y as f32 * scale) as i32,
            w: (rect.w as f32 * scale) as i32,
            h: (rect.h as f32 * scale) as i32,
        };
        graphics::draw_animated_dashed_rect(r);
    }

    pub fn draw_free_image(&self, img: &FreeImage<WrappedImage>) {
        let w = img.texture.width() as f32;
        let h = img.texture.height() as f32;

        let scale = self.zoom();
        let x = self.canvas_pos().x - self.camera().x + img.rect.x as f32 * scale;
        let y = self.canvas_pos().y - self.camera().y + img.rect.y as f32 * scale;

        let params = DrawTextureParams {
            dest_size: Some(Vec2 {
                x: w * scale,
                y: h * scale,
            }),
            ..Default::default()
        };

        let layer_ix = self.inner.active_layer();
        let color = [255, 255, 255, self.inner.layer(layer_ix).opacity()];
        macroquad::prelude::draw_texture_ex(
            self.free_image_tex.unwrap(),
            x,
            y,
            color.into(),
            params,
        );
    }

    pub fn screen_to_canvas(&self, x: f32, y: f32) -> (i32, i32) {
        let canvas_x = self.canvas_pos().x - self.camera().x;
        let canvas_y = self.canvas_pos().y - self.camera().y;
        let scale = self.zoom();

        (
            ((x - canvas_x) / scale) as i32,
            ((y - canvas_y) / scale) as i32,
        )
    }

    pub fn is_mouse_on_selection(&self) -> bool {
        let (x, y) = macroquad::prelude::mouse_position();
        let (x, y) = self.screen_to_canvas(x, y);

        let rect = match self.inner.selection() {
            Some(Selection::FreeImage) => self.inner.free_image().unwrap().rect,
            Some(Selection::Canvas(rect)) => rect.into(),
            _ => return false,
        };

        rect.contains(x, y)
    }
}