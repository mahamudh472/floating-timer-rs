use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    Margin, Painter, PointerButton, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui,
    Vec2, ViewportCommand,
};

use crate::settings::Settings;
use crate::sticky;
use crate::tray::{self, Command, TimerTray};

const BG: Color32 = Color32::from_rgb(0x23, 0x26, 0x29);
const BORDER: Color32 = Color32::from_rgb(0x4c, 0x56, 0x6a);
const FIELD_BG: Color32 = Color32::from_rgb(0x31, 0x36, 0x3b);
const FIELD_BORDER: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
const HOVER: Color32 = Color32::from_rgb(0x3b, 0x42, 0x52);
const WHITE: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xf2);
const HINT: Color32 = Color32::from_rgb(0x8b, 0x91, 0x98);

const EXPANDED_SIZE: Vec2 = Vec2::new(340.0, 170.0);
const COMPACT_SIZE: Vec2 = Vec2::new(120.0, 44.0);
const SIZE_ANIMATION: Duration = Duration::from_millis(220);
const OPACITY_ANIMATION: Duration = Duration::from_millis(150);
const AUTO_COLLAPSE_AFTER: Duration = Duration::from_secs(10);
const COMPACT_IDLE_OPACITY: f32 = 0.35;

fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn bold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

struct Fade {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
}

impl Fade {
    fn new(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            start: Instant::now() - Duration::from_secs(1),
            duration: OPACITY_ANIMATION,
        }
    }

    fn target(&mut self, target: f32) {
        if (self.to - target).abs() > 0.001 {
            self.from = self.value();
            self.to = target;
            self.start = Instant::now();
        }
    }

    fn value(&self) -> f32 {
        let t = self.start.elapsed().as_secs_f32() / self.duration.as_secs_f32();
        self.from + (self.to - self.from) * ease_out_cubic(t)
    }

    fn finished(&self) -> bool {
        self.start.elapsed() >= self.duration
    }
}

struct WindowResize {
    from: Vec2,
    to: Vec2,
    start: Instant,
    active: bool,
}

impl WindowResize {
    fn new(size: Vec2) -> Self {
        Self {
            from: size,
            to: size,
            start: Instant::now() - SIZE_ANIMATION,
            active: false,
        }
    }

    fn animate_to(&mut self, from: Vec2, to: Vec2) {
        self.from = from;
        self.to = to;
        self.start = Instant::now();
        self.active = (from - to).length() > 0.5;
    }

    /// Returns the size that the native window should have this frame.
    fn next(&mut self) -> Option<Vec2> {
        if !self.active {
            return None;
        }
        let t = self.start.elapsed().as_secs_f32() / SIZE_ANIMATION.as_secs_f32();
        if t >= 1.0 {
            self.active = false;
            Some(self.to)
        } else {
            let eased = ease_out_cubic(t);
            Some(self.from + (self.to - self.from) * eased)
        }
    }
}

pub struct TimerApp {
    task: String,
    minutes: u32,
    /// Seconds left while paused. While running, `deadline` is authoritative.
    remaining: f64,
    deadline: Option<Instant>,
    running: bool,

    compact: bool,
    hovered: bool,
    last_hover: Instant,

    opacity: Fade,
    resize: WindowResize,
    last_size: Vec2,

    settings: Settings,
    last_pos: Option<[f32; 2]>,
    commands: Receiver<Command>,
    tray: Option<ksni::blocking::Handle<TimerTray>>,
    quitting: bool,
}

impl TimerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, settings: Settings) -> Self {
        install_fonts(&cc.egui_ctx);

        let minutes = settings.minutes.clamp(1, 999);
        let mut app = Self {
            task: settings.task.clone(),
            minutes,
            remaining: f64::from(minutes) * 60.0,
            deadline: None,
            running: false,
            compact: false,
            hovered: false,
            last_hover: Instant::now(),
            opacity: Fade::new(1.0),
            resize: WindowResize::new(EXPANDED_SIZE),
            last_size: EXPANDED_SIZE,
            last_pos: settings.pos,
            settings,
            commands: std::sync::mpsc::channel().1,
            tray: None,
            quitting: false,
        };

        let (commands, tray) = tray::spawn(cc.egui_ctx.clone());
        app.commands = commands;
        app.tray = tray;

        let window_id = cc.winit_window().and_then(|window| x11_window_id(window));
        sticky::apply(window_id);

        app
    }

    fn duration_secs(&self) -> f64 {
        f64::from(self.minutes) * 60.0
    }

    fn remaining_secs(&self) -> f64 {
        if self.running
            && let Some(deadline) = self.deadline
        {
            return deadline
                .saturating_duration_since(Instant::now())
                .as_secs_f64();
        }
        self.remaining
    }

    fn display_time(&self) -> String {
        let total = self.remaining_secs().ceil() as u64;
        let (minutes, seconds) = (total / 60, total % 60);
        format!("{minutes:02}:{seconds:02}")
    }

    fn toggle(&mut self, ctx: &Context) {
        if self.running {
            self.remaining = self.remaining_secs();
            self.running = false;
            self.deadline = None;
            self.set_compact(ctx, false);
        } else {
            if self.remaining <= 0.0 {
                self.remaining = self.duration_secs();
            }
            self.deadline = Some(Instant::now() + Duration::from_secs_f64(self.remaining));
            self.running = true;
            self.set_compact(ctx, true);
        }
        ctx.request_repaint();
    }

    fn reset(&mut self, ctx: &Context) {
        self.running = false;
        self.deadline = None;
        self.remaining = self.duration_secs();
        self.set_compact(ctx, false);
        ctx.request_repaint();
    }

    fn finish(&mut self, ctx: &Context) {
        self.running = false;
        self.deadline = None;
        self.remaining = 0.0;
        self.set_compact(ctx, false);

        let task = self.task.trim().to_owned();
        std::thread::spawn(move || {
            let body = if task.is_empty() {
                "Time is up!"
            } else {
                task.as_str()
            };
            let _ = notify_rust::Notification::new()
                .summary("Timer Finished")
                .body(body)
                .appname("Floating Timer")
                .timeout(notify_rust::Timeout::Milliseconds(8000))
                .show();
        });

        ctx.request_repaint();
    }

    fn set_compact(&mut self, ctx: &Context, compact: bool) {
        if self.compact == compact {
            return;
        }
        self.compact = compact;
        let current = self.current_window_size(ctx);
        self.last_size = current;
        let target = if compact { COMPACT_SIZE } else { EXPANDED_SIZE };
        self.resize.animate_to(current, target);
        if compact {
            self.last_hover = Instant::now();
        }
        ctx.request_repaint();
    }

    fn current_window_size(&self, ctx: &Context) -> Vec2 {
        ctx.input(|input| {
            input
                .viewport()
                .inner_rect
                .map(|rect| rect.size())
                .unwrap_or(self.last_size)
        })
    }

    fn adjust_minutes(&mut self, delta: i32, ctx: &Context) {
        let next = (i64::from(self.minutes) + i64::from(delta)).clamp(1, 999) as u32;
        if next != self.minutes {
            self.minutes = next;
            if !self.running {
                self.remaining = self.duration_secs();
            }
            self.save();
            ctx.request_repaint();
        }
    }

    fn save(&mut self) {
        self.settings.task = self.task.clone();
        self.settings.minutes = self.minutes;
        self.settings.pos = self.last_pos;
        self.settings.save();
    }

    fn quit(&mut self, ctx: &Context) {
        self.save();
        self.quitting = true;
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    fn poll_tray(&mut self, ctx: &Context) {
        while let Ok(command) = self.commands.try_recv() {
            match command {
                Command::Show => {
                    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(ViewportCommand::Focus);
                }
                Command::Quit => self.quit(ctx),
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Space)) {
            self.toggle(ctx);
        }
        if ctx.input(|input| input.modifiers.ctrl && input.key_pressed(egui::Key::R)) {
            self.reset(ctx);
        }
    }

    fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let card = ui.max_rect();
        let radius = if self.compact {
            CornerRadius::same(22)
        } else {
            CornerRadius::same(14)
        };

        ui.set_opacity(self.opacity.value());
        let painter = ui.painter().clone();
        painter.rect_filled(card, radius, BG);
        painter.rect_stroke(card, radius, Stroke::new(1.0, BORDER), StrokeKind::Inside);

        let drag = ui.interact(card, ui.scope_id().with("card"), Sense::click_and_drag());

        if self.compact {
            self.draw_compact(&painter, card);
        } else {
            self.draw_expanded(ui, &painter, card);
        }

        if drag.clicked() && self.compact {
            self.set_compact(&ctx, false);
        }
        if drag.drag_started_by(PointerButton::Primary) {
            // Let the compositor move the window natively.
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if drag.drag_stopped_by(PointerButton::Primary) {
            self.save();
        }
    }

    fn draw_compact(&self, painter: &Painter, card: Rect) {
        painter.text(
            card.center(),
            Align2::CENTER_CENTER,
            self.display_time(),
            bold_font(24.0),
            WHITE,
        );
    }

    fn draw_expanded(&mut self, ui: &mut Ui, painter: &Painter, card: Rect) {
        let inner = card.shrink2(Vec2::new(12.0, 12.0));
        let title_height = 30.0;
        let gap = 6.0;

        let close_rect = Rect::from_min_size(
            Pos2::new(inner.max.x - 28.0, inner.min.y),
            Vec2::new(28.0, 28.0),
        );
        let task_rect = Rect::from_min_max(
            inner.min,
            Pos2::new(close_rect.min.x - 8.0, inner.min.y + title_height),
        );
        let time_rect = Rect::from_min_max(
            Pos2::new(inner.min.x, task_rect.max.y + gap),
            Pos2::new(inner.max.x, task_rect.max.y + gap + 74.0),
        );
        let controls_y = time_rect.max.y + gap;
        let button = 30.0;

        let start_rect = Rect::from_min_size(
            Pos2::new(inner.min.x, controls_y),
            Vec2::new(button, button),
        );
        let reset_rect = start_rect.translate(Vec2::new(button + 4.0, 0.0));
        let compact_rect = reset_rect.translate(Vec2::new(button + 4.0, 0.0));
        let minutes_rect = Rect::from_min_size(
            Pos2::new(inner.max.x - 96.0, controls_y),
            Vec2::new(96.0, button),
        );

        // Task name text field.
        painter.rect(
            task_rect,
            CornerRadius::same(8),
            FIELD_BG,
            Stroke::new(1.0, FIELD_BORDER),
            StrokeKind::Inside,
        );
        let task_response = ui.put(
            task_rect,
            egui::TextEdit::singleline(&mut self.task)
                .hint_text("Task name...")
                .font(FontId::proportional(14.0))
                .text_color(WHITE)
                .frame(egui::Frame::NONE.inner_margin(Margin::symmetric(8, 4)))
                .align(Align2::LEFT_CENTER),
        );
        if task_response.changed() {
            self.save();
        }

        // Countdown display.
        painter.text(
            time_rect.center(),
            Align2::CENTER_CENTER,
            self.display_time(),
            bold_font(46.0),
            WHITE,
        );

        let start = self
            .icon_button(
                ui,
                start_rect,
                "start",
                if self.running {
                    Icon::Pause
                } else {
                    Icon::Play
                },
            )
            .on_hover_text(if self.running { "Pause" } else { "Start" });
        if start.clicked() {
            self.toggle(ui.ctx());
        }

        let reset = self
            .icon_button(ui, reset_rect, "reset", Icon::Reset)
            .on_hover_text("Reset");
        if reset.clicked() {
            self.reset(ui.ctx());
        }

        if self.running {
            let compact = self
                .icon_button(ui, compact_rect, "compact", Icon::Compact)
                .on_hover_text("Compact View");
            if compact.clicked() {
                self.set_compact(ui.ctx(), true);
            }
        }

        self.minutes_control(ui, painter, minutes_rect);

        let close = self
            .icon_button(ui, close_rect, "close", Icon::Close)
            .on_hover_text("Quit");
        if close.clicked() {
            self.quit(ui.ctx());
        }
    }

    fn icon_button(&self, ui: &mut Ui, rect: Rect, id: &str, icon: Icon) -> Response {
        let response = ui.interact(rect, ui.scope_id().with(id), Sense::click());
        let painter = ui.painter();
        if response.hovered() {
            painter.rect_filled(rect, CornerRadius::same(6), HOVER);
        }
        paint_icon(painter, rect, icon, WHITE);
        response
    }

    fn minutes_control(&mut self, ui: &mut Ui, painter: &Painter, rect: Rect) {
        painter.rect(
            rect,
            CornerRadius::same(8),
            FIELD_BG,
            Stroke::new(1.0, FIELD_BORDER),
            StrokeKind::Inside,
        );

        let side = 24.0;
        let minus_rect = Rect::from_min_size(rect.min, Vec2::new(side, rect.height()));
        let plus_rect = Rect::from_min_size(
            Pos2::new(rect.max.x - side, rect.min.y),
            Vec2::new(side, rect.height()),
        );
        let minus = ui.interact(
            minus_rect,
            ui.scope_id().with("minutes-minus"),
            Sense::click(),
        );
        let plus = ui.interact(
            plus_rect,
            ui.scope_id().with("minutes-plus"),
            Sense::click(),
        );

        for (rect, response, label) in [(&minus_rect, &minus, "-"), (&plus_rect, &plus, "+")] {
            if response.hovered() {
                painter.rect_filled(rect.shrink(3.0), CornerRadius::same(6), HOVER);
            }
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(16.0),
                WHITE,
            );
        }

        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!("{} min", self.minutes),
            FontId::proportional(13.0),
            WHITE,
        );

        if minus.clicked() {
            self.adjust_minutes(-1, ui.ctx());
        }
        if plus.clicked() {
            self.adjust_minutes(1, ui.ctx());
        }
    }
}

impl eframe::App for TimerApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_tray(ctx);
        if self.quitting {
            return;
        }

        let now = Instant::now();
        if self.running
            && let Some(deadline) = self.deadline
            && now >= deadline
        {
            self.finish(ctx);
        }

        if let Some(position) = ctx.input(|input| input.viewport().outer_rect.map(|rect| rect.min))
        {
            self.last_pos = Some([position.x, position.y]);
        }

        let card = ctx.content_rect();
        let hovered = ctx.input(|input| {
            input
                .pointer
                .hover_pos()
                .is_some_and(|pos| card.contains(pos))
        });
        if hovered {
            self.last_hover = now;
        }
        if hovered != self.hovered {
            self.hovered = hovered;
            ctx.request_repaint();
        }

        if self.running
            && !self.compact
            && !self.hovered
            && now.duration_since(self.last_hover) >= AUTO_COLLAPSE_AFTER
        {
            self.set_compact(ctx, true);
        }

        let target_opacity = if self.compact && !self.hovered {
            COMPACT_IDLE_OPACITY
        } else {
            1.0
        };
        self.opacity.target(target_opacity);

        if let Some(size) = self.resize.next() {
            self.last_size = size;
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
            ctx.request_repaint();
        }

        self.handle_shortcuts(ctx);

        if self.running {
            ctx.request_repaint_after(Duration::from_millis(100));
            if !self.compact && !self.hovered {
                let elapsed = self.last_hover.elapsed();
                if elapsed < AUTO_COLLAPSE_AFTER {
                    ctx.request_repaint_after(AUTO_COLLAPSE_AFTER - elapsed);
                }
            }
        }
        if !self.opacity.finished() || self.resize.active {
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}

impl Drop for TimerApp {
    fn drop(&mut self) {
        self.save();
        // Dropping the handle shuts the tray service down.
        self.tray.take();
    }
}

fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "bold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/DejaVuSans-Bold.ttf"
        ))),
    );
    fonts
        .families
        .insert(FontFamily::Name("bold".into()), vec!["bold".to_owned()]);
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        style.visuals = egui::Visuals::dark();
        style.visuals.override_text_color = Some(WHITE);
        style.visuals.weak_text_color = Some(HINT);
        style.visuals.selection.bg_fill = Color32::from_rgb(0x4c, 0x56, 0x6a);
        style.visuals.selection.stroke = Stroke::new(1.0, WHITE);
        style.spacing.item_spacing = Vec2::new(6.0, 6.0);
    });
}

fn x11_window_id(window: &winit::window::Window) -> Option<u64> {
    use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};

    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(handle) => Some(handle.window),
        RawWindowHandle::Xcb(handle) => Some(u64::from(handle.window.get())),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum Icon {
    Play,
    Pause,
    Reset,
    Compact,
    Close,
}

fn paint_icon(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let center = rect.center();
    match icon {
        Icon::Play => {
            let points = vec![
                Pos2::new(center.x - 4.0, center.y - 7.0),
                Pos2::new(center.x + 7.0, center.y),
                Pos2::new(center.x - 4.0, center.y + 7.0),
            ];
            painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
        }
        Icon::Pause => {
            for dx in [-3.5_f32, 3.5] {
                painter.rect_filled(
                    Rect::from_center_size(center + Vec2::new(dx, 0.0), Vec2::new(3.4, 13.0)),
                    CornerRadius::same(1),
                    color,
                );
            }
        }
        Icon::Reset => {
            let radius = 7.0;
            let start = -0.9_f32;
            let sweep = std::f32::consts::TAU * 0.78;
            let steps = 32;
            let points: Vec<Pos2> = (0..=steps)
                .map(|i| {
                    let angle = start + sweep * (i as f32 / steps as f32);
                    Pos2::new(
                        center.x + radius * angle.cos(),
                        center.y + radius * angle.sin(),
                    )
                })
                .collect();
            painter.add(Shape::line(points, Stroke::new(1.7, color)));

            let angle = start + sweep;
            let end = Pos2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            );
            let tangent = Vec2::new(-angle.sin(), angle.cos());
            let normal = Vec2::new(tangent.y, -tangent.x);
            painter.add(Shape::convex_polygon(
                vec![
                    end + tangent * 4.0,
                    end - tangent * 1.5 + normal * 3.0,
                    end - tangent * 1.5 - normal * 3.0,
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Compact => {
            let end = center + Vec2::new(-3.0, 3.5);
            painter.line_segment(
                [center + Vec2::new(4.5, -4.5), end],
                Stroke::new(1.7, color),
            );
            painter.line_segment([end, end + Vec2::new(0.0, -5.0)], Stroke::new(1.7, color));
            painter.line_segment([end, end + Vec2::new(5.0, 0.0)], Stroke::new(1.7, color));
        }
        Icon::Close => {
            let d = 4.5;
            painter.line_segment(
                [center + Vec2::new(-d, -d), center + Vec2::new(d, d)],
                Stroke::new(1.6, color),
            );
            painter.line_segment(
                [center + Vec2::new(d, -d), center + Vec2::new(-d, d)],
                Stroke::new(1.6, color),
            );
        }
    }
}
