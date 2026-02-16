use rand::Rng;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    widgets::Widget,
};

// Fixed face dimensions (in terminal cells)
// Terminal chars are roughly 1:2 aspect ratio (width:height), so we
// use half-block characters (▀▄█) for smoother rendering.
// The face is drawn in a fixed 40×20 cell region (looks ~40×40 visually).
pub const FACE_WIDTH: u16 = 40;
pub const FACE_HEIGHT: u16 = 20;

/// Dark blue face color matching reference design
const FACE_COLOR: Color = Color::Rgb(30, 50, 120);
/// White for eyes and mouth
const FEATURE_COLOR: Color = Color::White;

/// The visual state of the face animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceState {
    /// Idle / waiting — normal eyes, neutral
    Idle,
    /// Listening to user — squinted eyes, "?" indicator
    Listening,
    /// Speaking TTS — mouth bounces open/closed
    Speaking,
    /// Thinking / processing — squinted eyes, "..." bubbles
    Thinking,
}

/// Drives the face animation: current state, frame counter, blink timing.
pub struct FaceAnimator {
    state: FaceState,
    frame: u64,
    /// Frame number at which the current blink started (None = not blinking)
    blink_start: Option<u64>,
    /// Frame number at which the next blink should trigger
    next_blink_frame: u64,
    /// Idle animation sub-state: eye look direction offset
    idle_look_dx: i16,
    idle_look_dy: i16,
}

impl FaceAnimator {
    pub fn new() -> Self {
        Self {
            state: FaceState::Idle,
            frame: 0,
            blink_start: None,
            next_blink_frame: Self::random_blink_delay(0),
            idle_look_dx: 0,
            idle_look_dy: 0,
        }
    }

    pub fn set_state(&mut self, state: FaceState) {
        self.state = state;
    }

    pub fn state(&self) -> FaceState {
        self.state
    }

    /// Advance one frame (~67ms at 15 FPS).
    pub fn tick(&mut self) {
        self.frame += 1;

        // Handle blink timing
        if let Some(start) = self.blink_start {
            if self.frame - start >= 3 {
                self.blink_start = None;
                self.next_blink_frame = Self::random_blink_delay(self.frame);
            }
        } else if self.frame >= self.next_blink_frame {
            self.blink_start = Some(self.frame);
        }

        // Idle: gentle eye movement cycle (~4s period)
        if self.state == FaceState::Idle {
            let cycle = (self.frame % 60) as i16;
            if cycle < 15 {
                self.idle_look_dx = 0;
                self.idle_look_dy = 0;
            } else if cycle < 30 {
                self.idle_look_dx = -1;
                self.idle_look_dy = 0;
            } else if cycle < 45 {
                self.idle_look_dx = 1;
                self.idle_look_dy = -1;
            } else {
                self.idle_look_dx = 0;
                self.idle_look_dy = 0;
            }
        } else {
            self.idle_look_dx = 0;
            self.idle_look_dy = 0;
        }
    }

    pub fn is_blinking(&self) -> bool {
        self.blink_start.is_some()
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    fn random_blink_delay(current_frame: u64) -> u64 {
        let mut rng = rand::thread_rng();
        current_frame + rng.gen_range(45..=90)
    }

    pub fn widget(&self) -> FaceWidget {
        FaceWidget {
            state: self.state,
            is_blinking: self.is_blinking(),
            frame: self.frame,
            idle_look_dx: self.idle_look_dx,
            idle_look_dy: self.idle_look_dy,
        }
    }
}

/// A ratatui Widget that draws the cartoon face in a fixed-size region.
/// Uses half-block characters for smooth sub-cell rendering.
pub struct FaceWidget {
    state: FaceState,
    is_blinking: bool,
    frame: u64,
    idle_look_dx: i16,
    idle_look_dy: i16,
}

/// A 2D pixel buffer at double vertical resolution (using half-blocks).
/// Each "pixel" is half a terminal cell tall, giving smoother circles.
struct PixelCanvas {
    width: usize,
    height: usize, // in half-cell pixels (2× the terminal rows)
    pixels: Vec<Color>,
    bg: Color,
}

impl PixelCanvas {
    fn new(term_width: usize, term_height: usize, bg: Color) -> Self {
        let width = term_width;
        let height = term_height * 2; // double resolution vertically
        Self {
            width,
            height,
            pixels: vec![bg; width * height],
            bg,
        }
    }

    fn set(&mut self, x: i32, y: i32, color: Color) {
        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
            self.pixels[y as usize * self.width + x as usize] = color;
        }
    }

    fn get(&self, x: usize, y: usize) -> Color {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x]
        } else {
            self.bg
        }
    }

    /// Draw a filled ellipse with smooth anti-aliased edges.
    fn fill_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, color: Color) {
        let x_min = ((cx - rx).floor() as i32).max(0);
        let x_max = ((cx + rx).ceil() as i32).min(self.width as i32 - 1);
        let y_min = ((cy - ry).floor() as i32).max(0);
        let y_max = ((cy + ry).ceil() as i32).min(self.height as i32 - 1);

        for py in y_min..=y_max {
            for px in x_min..=x_max {
                let nx = (px as f64 - cx) / rx;
                let ny = (py as f64 - cy) / ry;
                if nx * nx + ny * ny <= 1.0 {
                    self.set(px, py, color);
                }
            }
        }
    }

    /// Draw a filled rounded rectangle.
    fn fill_rounded_rect(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: Color) {
        for py in y..(y + h) {
            for px in x..(x + w) {
                // Check if inside rounded corners
                let mut inside = true;
                // Top-left corner
                if px < x + radius && py < y + radius {
                    let dx = (x + radius) - px;
                    let dy = (y + radius) - py;
                    if dx * dx + dy * dy > radius * radius {
                        inside = false;
                    }
                }
                // Top-right corner
                if px >= x + w - radius && py < y + radius {
                    let dx = px - (x + w - radius - 1);
                    let dy = (y + radius) - py;
                    if dx * dx + dy * dy > radius * radius {
                        inside = false;
                    }
                }
                // Bottom-left corner
                if px < x + radius && py >= y + h - radius {
                    let dx = (x + radius) - px;
                    let dy = py - (y + h - radius - 1);
                    if dx * dx + dy * dy > radius * radius {
                        inside = false;
                    }
                }
                // Bottom-right corner
                if px >= x + w - radius && py >= y + h - radius {
                    let dx = px - (x + w - radius - 1);
                    let dy = py - (y + h - radius - 1);
                    if dx * dx + dy * dy > radius * radius {
                        inside = false;
                    }
                }
                if inside {
                    self.set(px, py, color);
                }
            }
        }
    }

    /// Render the pixel canvas onto a ratatui Buffer using half-block characters.
    /// ▀ (upper half block): fg = top pixel, bg = bottom pixel
    fn render_to_buffer(&self, buf: &mut Buffer, area: Rect) {
        for row in 0..area.height as usize {
            let top_y = row * 2;
            let bot_y = row * 2 + 1;
            for col in 0..area.width as usize {
                if col >= self.width {
                    break;
                }
                let top_color = self.get(col, top_y);
                let bot_color = self.get(col, bot_y);
                let x = area.x + col as u16;
                let y = area.y + row as u16;
                if x < area.x + area.width && y < area.y + area.height {
                    buf[(x, y)]
                        .set_char('▀')
                        .set_fg(top_color)
                        .set_bg(bot_color);
                }
            }
        }
    }
}

impl Widget for FaceWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // We render into a fixed FACE_WIDTH × FACE_HEIGHT region, centered in area
        let render_w = FACE_WIDTH.min(area.width);
        let render_h = FACE_HEIGHT.min(area.height);
        let offset_x = area.x + (area.width.saturating_sub(render_w)) / 2;
        let offset_y = area.y + (area.height.saturating_sub(render_h)) / 2;
        let render_area = Rect::new(offset_x, offset_y, render_w, render_h);

        let w = render_w as usize;
        let h = render_h as usize;
        let bg = Color::Reset;

        let mut canvas = PixelCanvas::new(w, h * 2, bg);

        // Pixel coordinates in the double-resolution canvas
        let pw = w as f64;
        let ph = (h * 2) as f64;
        let cx = pw / 2.0;
        let cy = ph / 2.0;

        // --- Draw face circle ---
        // Radius: fill most of the fixed area
        let face_rx = pw / 2.0 - 1.0;
        let face_ry = ph / 2.0 - 1.0;
        canvas.fill_ellipse(cx, cy, face_rx, face_ry, FACE_COLOR);

        // --- Draw eyes ---
        let eye_spacing = (pw * 0.30) as f64; // distance from center to each eye
        let eye_y_offset = ph * 0.08; // eyes slightly above center
        let eye_y = cy - eye_y_offset;

        let eye_rx: f64;
        let eye_ry: f64;

        match self.state {
            FaceState::Listening | FaceState::Thinking => {
                // Squinted eyes: narrower width
                eye_rx = pw * 0.10;
                eye_ry = pw * 0.18;
            }
            _ => {
                // Normal large circular eyes
                eye_rx = pw * 0.18;
                eye_ry = pw * 0.18;
            }
        }

        if self.is_blinking {
            // Blink: flatten eyes to thin horizontal lines
            let blink_ry = 1.0_f64;
            canvas.fill_ellipse(
                cx - eye_spacing + self.idle_look_dx as f64,
                eye_y,
                eye_rx,
                blink_ry,
                FEATURE_COLOR,
            );
            canvas.fill_ellipse(
                cx + eye_spacing + self.idle_look_dx as f64,
                eye_y,
                eye_rx,
                blink_ry,
                FEATURE_COLOR,
            );
        } else {
            // Normal eyes
            canvas.fill_ellipse(
                cx - eye_spacing + self.idle_look_dx as f64,
                eye_y + self.idle_look_dy as f64,
                eye_rx,
                eye_ry,
                FEATURE_COLOR,
            );
            canvas.fill_ellipse(
                cx + eye_spacing + self.idle_look_dx as f64,
                eye_y + self.idle_look_dy as f64,
                eye_rx,
                eye_ry,
                FEATURE_COLOR,
            );
        }

        // --- Draw mouth ---
        let mouth_y = cy + ph * 0.22;
        match self.state {
            FaceState::Speaking => {
                // Mouth bounces open/closed (like LVGL reference)
                let bounce = ((self.frame % 8) as f64 / 8.0 * std::f64::consts::PI * 2.0).sin();
                let mouth_ry = pw * 0.06 + (bounce.abs() * pw * 0.06);
                let mouth_rx = pw * 0.10;
                canvas.fill_ellipse(cx, mouth_y, mouth_rx, mouth_ry, FEATURE_COLOR);
            }
            FaceState::Idle => {
                // Small gentle smile — thin horizontal ellipse
                let mouth_rx = pw * 0.10;
                let mouth_ry = 1.5;
                canvas.fill_ellipse(cx, mouth_y, mouth_rx, mouth_ry, FEATURE_COLOR);
            }
            FaceState::Listening => {
                // Small "o" mouth
                let mouth_rx = pw * 0.05;
                let mouth_ry = pw * 0.05;
                canvas.fill_ellipse(cx, mouth_y, mouth_rx, mouth_ry, FEATURE_COLOR);
            }
            FaceState::Thinking => {
                // Wavy mouth "～"
                let mouth_rx = pw * 0.08;
                let mouth_ry = 1.5;
                canvas.fill_ellipse(cx, mouth_y + 1.0, mouth_rx, mouth_ry, FEATURE_COLOR);
            }
        }

        // --- Draw state indicators outside the face ---
        if self.state == FaceState::Thinking {
            // Animated dots "•••" to the upper right
            let num_dots = ((self.frame / 6) % 4) as usize;
            let dot_x = (cx + face_rx * 0.6) as i32;
            let dot_y = (cy - face_ry * 0.7) as i32;
            for i in 0..num_dots.min(3) {
                canvas.fill_ellipse(
                    (dot_x + i as i32 * 3) as f64,
                    dot_y as f64,
                    1.0,
                    1.0,
                    Color::Rgb(200, 200, 255),
                );
            }
        }

        if self.state == FaceState::Listening {
            // "?" indicator floating near top-right
            let q_x = (cx + face_rx * 0.55) as i32;
            let q_y = (cy - face_ry * 0.65) as i32;
            // Draw a small "?" using pixels
            // Wobble animation
            let wobble = ((self.frame as f64 / 4.0).sin() * 2.0) as i32;
            let qx = q_x + wobble;
            // Simple ? shape
            for dx in -1..=1 {
                canvas.set(qx + dx, q_y - 3, Color::Rgb(200, 200, 255));
            }
            canvas.set(qx + 1, q_y - 2, Color::Rgb(200, 200, 255));
            canvas.set(qx, q_y - 1, Color::Rgb(200, 200, 255));
            canvas.set(qx, q_y + 1, Color::Rgb(200, 200, 255));
        }

        // Render canvas to buffer
        canvas.render_to_buffer(buf, render_area);
    }
}
