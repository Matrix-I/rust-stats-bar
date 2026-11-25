//! The pages' building blocks: rings, bars, legend rows and the process table. Each is built
//! once and updated in place, so a click is never lost to a rebuild.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, DrawingArea, Label, Orientation, cairo, gdk};

use crate::view::{Core, ProcessRow, QuitTarget, Ring, Row, Segment, Tone};

const RING_SIZE: i32 = 86;
const RING_WIDTH: f64 = 8.0;
const BAR_HEIGHT: i32 = 10;
const CORE_BAR_HEIGHT: i32 = 36;
const CORE_COLUMNS: u32 = 4;
/// The daemon sends six; this is the table's fixed size.
pub const PROCESS_ROWS: usize = 6;
const TRACK_ALPHA: f32 = 0.12;
const DIM_ALPHA: f32 = 0.45;
const FREE_ALPHA: f32 = 0.25;

enum Paint {
    /// Red, green, blue and alpha, 0–1.
    Fixed([f32; 4]),
    /// The theme's text colour at this alpha, so it follows dark and light styles.
    Foreground(f32),
}

/// One table for drawn shapes and for text, so a tone looks the same in both.
fn paint(tone: Tone) -> Paint {
    // GNOME HIG palette: green 4, orange 3, red 3, blue 3.
    match tone {
        Tone::Good => Paint::Fixed([0.180, 0.761, 0.494, 1.0]),
        Tone::Warning | Tone::Kernel => Paint::Fixed([1.0, 0.471, 0.0, 1.0]),
        Tone::Bad | Tone::System => Paint::Fixed([0.878, 0.106, 0.141, 1.0]),
        Tone::User => Paint::Fixed([0.208, 0.518, 0.894, 1.0]),
        Tone::Cached => Paint::Fixed([0.208, 0.518, 0.894, 0.6]),
        Tone::App => Paint::Foreground(1.0),
        Tone::Free => Paint::Foreground(FREE_ALPHA),
        Tone::Dim => Paint::Foreground(DIM_ALPHA),
    }
}

fn rgba(tone: Tone, foreground: gdk::RGBA) -> gdk::RGBA {
    match paint(tone) {
        Paint::Fixed([red, green, blue, alpha]) => gdk::RGBA::new(red, green, blue, alpha),
        Paint::Foreground(alpha) => foreground.with_alpha(foreground.alpha() * alpha),
    }
}

fn css_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Dim => "tone-dim",
        Tone::Good => "tone-good",
        Tone::Warning => "tone-warning",
        Tone::Bad => "tone-bad",
        Tone::System => "tone-system",
        Tone::User => "tone-user",
        Tone::App => "tone-app",
        Tone::Kernel => "tone-kernel",
        Tone::Cached => "tone-cached",
        Tone::Free => "tone-free",
    }
}

const TONES: [Tone; 10] = [
    Tone::Dim,
    Tone::Good,
    Tone::Warning,
    Tone::Bad,
    Tone::System,
    Tone::User,
    Tone::App,
    Tone::Kernel,
    Tone::Cached,
    Tone::Free,
];

/// The style sheet for text in a tone, from the same table as the drawn shapes.
pub fn tone_css() -> String {
    TONES
        .iter()
        .map(|&tone| {
            let color = match paint(tone) {
                Paint::Fixed([red, green, blue, alpha]) => format!(
                    "rgba({}, {}, {}, {alpha})",
                    (red * 255.0).round(),
                    (green * 255.0).round(),
                    (blue * 255.0).round()
                ),
                Paint::Foreground(alpha) => format!("alpha(currentColor, {alpha})"),
            };
            format!(".{} {{ color: {color}; }}\n", css_class(tone))
        })
        .collect()
}

fn set_tone(widget: &impl IsA<gtk::Widget>, tone: Option<Tone>) {
    for &other in &TONES {
        widget.remove_css_class(css_class(other));
    }
    if let Some(tone) = tone {
        widget.add_css_class(css_class(tone));
    }
}

/// Segments laid end to end from `start`, each `fraction` of `span` long.
fn stroke_segments(
    context: &cairo::Context,
    foreground: gdk::RGBA,
    segments: &[Segment],
    mut draw: impl FnMut(&cairo::Context, f64, f64),
) {
    let mut start = 0.0;
    for segment in segments {
        let end = (start + segment.fraction).min(1.0);
        if end > start {
            context.set_source_color(&rgba(segment.tone, foreground));
            draw(context, start, end);
        }
        start = end;
    }
}

pub struct RingWidget {
    pub root: GtkBox,
    area: DrawingArea,
    center: Label,
    caption: Label,
    segments: Rc<RefCell<Vec<Segment>>>,
}

impl RingWidget {
    pub fn new() -> RingWidget {
        let segments: Rc<RefCell<Vec<Segment>>> = Rc::default();
        let area = DrawingArea::builder()
            .content_width(RING_SIZE)
            .content_height(RING_SIZE)
            .build();
        let drawn = Rc::clone(&segments);
        area.set_draw_func(move |area, context, width, height| {
            let foreground = area.color();
            let (x, y) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
            let radius = x.min(y) - RING_WIDTH / 2.0;
            context.set_line_width(RING_WIDTH);
            context.set_source_color(&foreground.with_alpha(TRACK_ALPHA));
            context.arc(x, y, radius, 0.0, std::f64::consts::TAU);
            // A failed stroke leaves the ring blank until the next frame; there is nothing to do.
            let _ = context.stroke();
            let top = -std::f64::consts::FRAC_PI_2;
            stroke_segments(context, foreground, &drawn.borrow(), |context, from, to| {
                context.arc(
                    x,
                    y,
                    radius,
                    top + from * std::f64::consts::TAU,
                    top + to * std::f64::consts::TAU,
                );
                let _ = context.stroke();
            });
        });
        let center = Label::new(None);
        center.add_css_class("title-3");
        center.add_css_class("numeric");
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&area));
        overlay.add_overlay(&center);
        let caption = Label::new(None);
        caption.add_css_class("caption-heading");
        let root = GtkBox::new(Orientation::Vertical, 8);
        root.append(&overlay);
        root.append(&caption);
        RingWidget {
            root,
            area,
            center,
            caption,
            segments,
        }
    }

    pub fn show(&self, ring: &Ring) {
        self.segments.replace(ring.segments.clone());
        self.area.queue_draw();
        self.center.set_label(&ring.center);
        set_tone(&self.center, ring.center_tone);
        self.caption.set_label(ring.caption);
    }
}

/// A horizontal bar of segments, as the memory breakdown.
pub struct BarWidget {
    pub area: DrawingArea,
    segments: Rc<RefCell<Vec<Segment>>>,
}

impl BarWidget {
    pub fn new() -> BarWidget {
        let segments: Rc<RefCell<Vec<Segment>>> = Rc::default();
        let area = DrawingArea::builder()
            .content_height(BAR_HEIGHT)
            .hexpand(true)
            .build();
        let drawn = Rc::clone(&segments);
        area.set_draw_func(move |area, context, width, height| {
            let foreground = area.color();
            let (width, height) = (f64::from(width), f64::from(height));
            context.set_source_color(&foreground.with_alpha(TRACK_ALPHA));
            context.rectangle(0.0, 0.0, width, height);
            let _ = context.fill();
            stroke_segments(context, foreground, &drawn.borrow(), |context, from, to| {
                context.rectangle(from * width, 0.0, (to - from) * width, height);
                let _ = context.fill();
            });
        });
        BarWidget { area, segments }
    }

    pub fn show(&self, segments: &[Segment]) {
        self.segments.replace(segments.to_vec());
        self.area.queue_draw();
    }
}

/// A vertical bar filled from the bottom, one per core.
struct CoreWidget {
    root: GtkBox,
    temperature: Label,
    area: DrawingArea,
    busy: Rc<RefCell<f64>>,
    busy_text: Label,
}

impl CoreWidget {
    fn new() -> CoreWidget {
        let busy: Rc<RefCell<f64>> = Rc::default();
        let area = DrawingArea::builder()
            .content_height(CORE_BAR_HEIGHT)
            .hexpand(true)
            .build();
        let drawn = Rc::clone(&busy);
        area.set_draw_func(move |area, context, width, height| {
            let foreground = area.color();
            let (width, height) = (f64::from(width), f64::from(height));
            context.set_source_color(&foreground.with_alpha(TRACK_ALPHA));
            context.rectangle(0.0, 0.0, width, height);
            let _ = context.fill();
            let filled = height * drawn.borrow().clamp(0.0, 1.0);
            context.set_source_color(&rgba(Tone::User, foreground));
            context.rectangle(0.0, height - filled, width, filled);
            let _ = context.fill();
        });
        let temperature = small_label();
        let busy_text = small_label();
        busy_text.add_css_class("dim-label");
        let root = GtkBox::new(Orientation::Vertical, 3);
        root.append(&temperature);
        root.append(&area);
        root.append(&busy_text);
        CoreWidget {
            root,
            temperature,
            area,
            busy,
            busy_text,
        }
    }

    fn show(&self, core: &Core) {
        self.busy.replace(core.busy);
        self.area.queue_draw();
        self.busy_text.set_label(&core.busy_text);
        match &core.temperature {
            Some((text, tone)) => {
                self.temperature.set_label(text);
                set_tone(&self.temperature, Some(*tone));
            }
            None => self.temperature.set_label(""),
        }
    }
}

fn small_label() -> Label {
    let label = Label::new(None);
    label.add_css_class("caption");
    label.add_css_class("numeric");
    label
}

/// One bar per logical CPU, four to a row. Rebuilt only when the number of CPUs changes.
pub struct CoresWidget {
    pub root: gtk::Grid,
    cells: RefCell<Vec<CoreWidget>>,
}

impl CoresWidget {
    pub fn new() -> CoresWidget {
        let root = gtk::Grid::builder()
            .column_spacing(8)
            .row_spacing(8)
            .column_homogeneous(true)
            .build();
        CoresWidget {
            root,
            cells: RefCell::default(),
        }
    }

    pub fn show(&self, cores: &[Core]) {
        let mut cells = self.cells.borrow_mut();
        if cells.len() != cores.len() {
            for cell in cells.drain(..) {
                self.root.remove(&cell.root);
            }
            for index in 0..cores.len() {
                let cell = CoreWidget::new();
                let index = u32::try_from(index).unwrap_or(u32::MAX);
                let (column, row) = (index % CORE_COLUMNS, index / CORE_COLUMNS);
                self.root.attach(
                    &cell.root,
                    i32::try_from(column).unwrap_or(0),
                    i32::try_from(row).unwrap_or(0),
                    1,
                    1,
                );
                cells.push(cell);
            }
        }
        for (cell, core) in cells.iter().zip(cores) {
            cell.show(core);
        }
    }
}

/// A caption above a section, as upstream's "DETAILS".
pub fn section(title: &str) -> Label {
    let label = Label::builder().label(title).xalign(0.0).build();
    label.add_css_class("heading");
    label.add_css_class("dim-label");
    label
}

struct RowWidget {
    root: GtkBox,
    dot: Label,
    label: Label,
    value: Label,
}

/// Legend and info rows. The list grows as needed; unused rows are hidden, not removed.
pub struct RowsWidget {
    pub root: GtkBox,
    rows: RefCell<Vec<RowWidget>>,
}

impl RowsWidget {
    pub fn new() -> RowsWidget {
        RowsWidget {
            root: GtkBox::new(Orientation::Vertical, 6),
            rows: RefCell::default(),
        }
    }

    pub fn show(&self, rows: &[Row]) {
        let mut widgets = self.rows.borrow_mut();
        while widgets.len() < rows.len() {
            let dot = Label::new(Some("●"));
            let label = Label::builder().xalign(0.0).hexpand(true).build();
            let value = Label::builder().xalign(1.0).build();
            value.add_css_class("numeric");
            let root = GtkBox::new(Orientation::Horizontal, 8);
            root.append(&dot);
            root.append(&label);
            root.append(&value);
            self.root.append(&root);
            widgets.push(RowWidget {
                root,
                dot,
                label,
                value,
            });
        }
        for (index, widget) in widgets.iter().enumerate() {
            let Some(row) = rows.get(index) else {
                widget.root.set_visible(false);
                continue;
            };
            widget.root.set_visible(true);
            widget.label.set_label(row.label);
            widget.value.set_label(&row.value);
            // A row without a dot keeps the dot's width, so the labels line up.
            widget
                .dot
                .set_opacity(if row.dot.is_some() { 1.0 } else { 0.0 });
            set_tone(&widget.dot, row.dot);
        }
    }
}

struct ProcessWidget {
    root: GtkBox,
    name: Label,
    value: Label,
    quit: gtk::Button,
    target: Rc<RefCell<Option<QuitTarget>>>,
}

/// The top-process table: six fixed rows, each with a ✕ for a process the user may quit.
pub struct ProcessesWidget {
    pub root: GtkBox,
    reading: Label,
    rows: Vec<ProcessWidget>,
}

impl ProcessesWidget {
    /// `on_quit` gets the target the row showed when its ✕ was clicked.
    pub fn new(value_header: &str, on_quit: Rc<dyn Fn(QuitTarget)>) -> ProcessesWidget {
        let root = GtkBox::new(Orientation::Vertical, 6);
        let reading = Label::builder().label("Reading…").xalign(0.0).build();
        reading.add_css_class("caption");
        reading.add_css_class("dim-label");
        root.append(&reading);
        let header = GtkBox::new(Orientation::Horizontal, 8);
        let process = Label::builder()
            .label("Process")
            .xalign(0.0)
            .hexpand(true)
            .build();
        let value = Label::builder().label(value_header).xalign(1.0).build();
        for label in [&process, &value] {
            label.add_css_class("caption-heading");
            label.add_css_class("dim-label");
            header.append(label);
        }
        // The ✕ column's width, so the header's value lines up with the rows'.
        let spacer = gtk::Image::from_icon_name("window-close-symbolic");
        spacer.set_opacity(0.0);
        header.append(&spacer);
        root.append(&header);
        let rows = (0..PROCESS_ROWS)
            .map(|_| {
                let name = Label::builder()
                    .xalign(0.0)
                    .hexpand(true)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();
                let value = Label::builder().xalign(1.0).build();
                value.add_css_class("numeric");
                let quit = gtk::Button::from_icon_name("window-close-symbolic");
                quit.add_css_class("flat");
                quit.add_css_class("circular");
                quit.set_valign(Align::Center);
                let target: Rc<RefCell<Option<QuitTarget>>> = Rc::default();
                let clicked = Rc::clone(&target);
                let on_quit = Rc::clone(&on_quit);
                quit.connect_clicked(move |_| {
                    if let Some(target) = clicked.borrow().clone() {
                        on_quit(target);
                    }
                });
                let row = GtkBox::new(Orientation::Horizontal, 8);
                row.append(&name);
                row.append(&value);
                row.append(&quit);
                root.append(&row);
                ProcessWidget {
                    root: row,
                    name,
                    value,
                    quit,
                    target,
                }
            })
            .collect();
        let widget = ProcessesWidget {
            root,
            reading,
            rows,
        };
        widget.show(None);
        widget
    }

    pub fn show(&self, processes: Option<&[ProcessRow]>) {
        self.reading.set_visible(processes.is_none());
        let processes = processes.unwrap_or_default();
        for (index, widget) in self.rows.iter().enumerate() {
            let Some(process) = processes.get(index) else {
                widget.root.set_visible(false);
                widget.target.replace(None);
                continue;
            };
            widget.root.set_visible(true);
            widget.name.set_label(&process.name);
            widget.value.set_label(&process.value);
            let quittable = process.quit.is_some();
            // Hidden by opacity, not removed, so the values stay in one column.
            widget.quit.set_opacity(if quittable { 1.0 } else { 0.0 });
            widget.quit.set_sensitive(quittable);
            widget.quit.set_can_target(quittable);
            widget.quit.set_tooltip_text(
                quittable
                    .then(|| format!("Quit {}…", process.name))
                    .as_deref(),
            );
            widget.target.replace(process.quit.clone());
        }
    }
}
