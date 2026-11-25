//! The window: a CPU | Memory switcher, the two pages, a Refresh footer, and an error page with
//! Retry while the daemon cannot be reached. One window per session: a second launch only
//! brings it forward on the page it names.

use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::path::Path;
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gtk::{Align, Box as GtkBox, Label, Orientation, gio, glib};
use statsbar_core::ipc::{CpuPage, Event, MemoryPage, Page, Request};

use crate::cli;
use crate::link::{Incoming, Link, LinkError, socket_path};
use crate::quit::quit;
use crate::view::{QuitTarget, cpu_view, memory_view};
use crate::widgets::{
    BarWidget, CoresWidget, ProcessesWidget, RingWidget, RowsWidget, section, tone_css,
};

/// The D-Bus name GTK uses to find a running panel; a second launch hands it its arguments.
pub const APP_ID: &str = "io.github.MatrixI.StatsBar";
/// For the handshake and each request. The daemon answers from its main loop within a tick.
const LINK_TIMEOUT: Duration = Duration::from_secs(2);
const WIDTH: i32 = 340;
/// Tall enough for the CPU page of an eight-thread CPU. The window is sized before the first
/// event fills the page, so it cannot take its height from the content.
const HEIGHT: i32 = 860;
const PROC: &str = "/proc";

fn page_name(page: Page) -> &'static str {
    match page {
        Page::Cpu => "cpu",
        Page::Memory => "memory",
    }
}

fn page_of_name(name: &str) -> Option<Page> {
    match name {
        "cpu" => Some(Page::Cpu),
        "memory" => Some(Page::Memory),
        _ => None,
    }
}

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_startup(|_| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&tone_css());
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
    let panel: Rc<RefCell<Option<Rc<Panel>>>> = Rc::default();
    app.connect_command_line(move |app, line| {
        let args: Vec<OsString> = line.arguments().into_iter().skip(1).collect();
        match cli::page_from_args(&args) {
            Ok(page) => {
                let panel = Rc::clone(panel.borrow_mut().get_or_insert_with(|| Panel::new(app)));
                panel.open(page);
                glib::ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("statsbar-panel: {message}");
                glib::ExitCode::FAILURE
            }
        }
    });
    app.run()
}

struct CpuWidgets {
    stack: gtk::Stack,
    subtitle: Label,
    rings: [RingWidget; 2],
    details: RowsWidget,
    cores: CoresWidget,
    frequency_title: Label,
    frequency: RowsWidget,
    processes: ProcessesWidget,
}

struct MemoryWidgets {
    stack: gtk::Stack,
    subtitle: Label,
    rings: [RingWidget; 2],
    used: Label,
    bar: BarWidget,
    details: RowsWidget,
    processes: ProcessesWidget,
}

/// A page waits on "Reading…" until its first event, then shows its figures for good.
fn waiting_stack(content: &impl IsA<gtk::Widget>) -> gtk::Stack {
    let reading = Label::new(Some("Reading…"));
    reading.add_css_class("dim-label");
    reading.set_margin_top(48);
    reading.set_margin_bottom(48);
    let stack = gtk::Stack::new();
    stack.add_named(&reading, Some("waiting"));
    stack.add_named(content, Some("content"));
    stack.set_visible_child_name("waiting");
    stack
}

fn page_box() -> GtkBox {
    let content = GtkBox::new(Orientation::Vertical, 14);
    for set in [GtkBox::set_margin_top, GtkBox::set_margin_bottom] {
        set(&content, 14);
    }
    content.set_margin_start(14);
    content.set_margin_end(14);
    content
}

fn subtitle_label() -> Label {
    let label = Label::builder().halign(Align::Center).wrap(true).build();
    label.add_css_class("dim-label");
    label
}

fn rings_box(rings: &[RingWidget; 2]) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 28);
    row.set_halign(Align::Center);
    for ring in rings {
        row.append(&ring.root);
    }
    row
}

fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(child)
        .build()
}

impl CpuWidgets {
    fn new(on_quit: Rc<dyn Fn(QuitTarget)>) -> CpuWidgets {
        let content = page_box();
        let subtitle = subtitle_label();
        let rings = [RingWidget::new(), RingWidget::new()];
        let details = RowsWidget::new();
        let cores = CoresWidget::new();
        let frequency_title = section("Frequency");
        let frequency = RowsWidget::new();
        let processes = ProcessesWidget::new("Usage", on_quit);
        content.append(&subtitle);
        content.append(&rings_box(&rings));
        content.append(&section("Details"));
        content.append(&details.root);
        content.append(&section("Cores"));
        content.append(&cores.root);
        content.append(&frequency_title);
        content.append(&frequency.root);
        content.append(&section("Top processes"));
        content.append(&processes.root);
        CpuWidgets {
            stack: waiting_stack(&content),
            subtitle,
            rings,
            details,
            cores,
            frequency_title,
            frequency,
            processes,
        }
    }

    fn show(&self, page: &CpuPage) {
        let view = cpu_view(page);
        self.subtitle.set_visible(view.subtitle.is_some());
        self.subtitle
            .set_label(view.subtitle.as_deref().unwrap_or_default());
        for (widget, ring) in self.rings.iter().zip(&view.rings) {
            widget.show(ring);
        }
        self.details.show(&view.details);
        self.cores.show(&view.cores);
        self.frequency_title.set_visible(view.frequency.is_some());
        self.frequency.show(view.frequency.as_slice());
        self.processes.show(view.processes.as_deref());
        self.stack.set_visible_child_name("content");
    }
}

impl MemoryWidgets {
    fn new(on_quit: Rc<dyn Fn(QuitTarget)>) -> MemoryWidgets {
        let content = page_box();
        let subtitle = subtitle_label();
        let rings = [RingWidget::new(), RingWidget::new()];
        let used = Label::builder().xalign(1.0).hexpand(true).build();
        used.add_css_class("title-4");
        used.add_css_class("numeric");
        let used_row = GtkBox::new(Orientation::Horizontal, 8);
        used_row.append(&Label::new(Some("Used")));
        used_row.append(&used);
        let bar = BarWidget::new();
        let details = RowsWidget::new();
        let processes = ProcessesWidget::new("Memory", on_quit);
        content.append(&subtitle);
        content.append(&rings_box(&rings));
        content.append(&used_row);
        content.append(&bar.area);
        content.append(&section("Details"));
        content.append(&details.root);
        content.append(&section("Top processes"));
        content.append(&processes.root);
        MemoryWidgets {
            stack: waiting_stack(&content),
            subtitle,
            rings,
            used,
            bar,
            details,
            processes,
        }
    }

    fn show(&self, page: &MemoryPage) {
        let view = memory_view(page);
        self.subtitle.set_label(&view.subtitle);
        for (widget, ring) in self.rings.iter().zip(&view.rings) {
            widget.show(ring);
        }
        self.used.set_label(&view.used);
        self.bar.show(&view.bar);
        self.details.show(&view.details);
        self.processes.show(view.processes.as_deref());
        self.stack.set_visible_child_name("content");
    }
}

struct Panel {
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    /// "pages" or "error".
    root: gtk::Stack,
    pages: adw::ViewStack,
    status: adw::StatusPage,
    /// Only while connected: there is nobody to ask otherwise.
    refresh: gtk::Button,
    cpu: CpuWidgets,
    memory: MemoryWidgets,
    link: RefCell<Option<Link>>,
    /// Raised by each connection attempt, so the events of an older one are ignored.
    generation: Cell<u64>,
    connecting: Cell<bool>,
}

impl Panel {
    fn new(app: &adw::Application) -> Rc<Panel> {
        Rc::new_cyclic(|weak: &Weak<Panel>| {
            let on_quit: Rc<dyn Fn(QuitTarget)> = {
                let weak = weak.clone();
                Rc::new(move |target| {
                    if let Some(panel) = weak.upgrade() {
                        panel.confirm_quit(target);
                    }
                })
            };
            let cpu = CpuWidgets::new(Rc::clone(&on_quit));
            let memory = MemoryWidgets::new(on_quit);
            let pages = adw::ViewStack::new();
            pages.add_titled_with_icon(
                &scrolled(&cpu.stack),
                Some(page_name(Page::Cpu)),
                "CPU",
                "computer-chip-symbolic",
            );
            pages.add_titled_with_icon(
                &scrolled(&memory.stack),
                Some(page_name(Page::Memory)),
                "Memory",
                "drive-harddisk-solidstate-symbolic",
            );
            let retry = gtk::Button::builder()
                .label("Retry")
                .halign(Align::Center)
                .build();
            retry.add_css_class("pill");
            retry.add_css_class("suggested-action");
            let status = adw::StatusPage::builder()
                .icon_name("dialog-warning-symbolic")
                .title("StatsBar is not available")
                .child(&retry)
                .build();
            let root = gtk::Stack::new();
            root.add_named(&pages, Some("pages"));
            root.add_named(&status, Some("error"));
            let refresh = gtk::Button::with_label("Refresh");
            refresh.set_sensitive(false);
            let footer = gtk::ActionBar::new();
            footer.pack_start(&refresh);
            let header = adw::HeaderBar::builder()
                .title_widget(
                    &adw::ViewSwitcher::builder()
                        .stack(&pages)
                        .policy(adw::ViewSwitcherPolicy::Wide)
                        .build(),
                )
                .build();
            let toolbar = adw::ToolbarView::new();
            toolbar.add_top_bar(&header);
            toolbar.set_content(Some(&root));
            toolbar.add_bottom_bar(&footer);
            let toasts = adw::ToastOverlay::new();
            toasts.set_child(Some(&toolbar));
            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("StatsBar")
                .default_width(WIDTH)
                .default_height(HEIGHT)
                .content(&toasts)
                .build();
            {
                let weak = weak.clone();
                retry.connect_clicked(move |_| {
                    if let Some(panel) = weak.upgrade() {
                        panel.connect();
                    }
                });
            }
            {
                let weak = weak.clone();
                refresh.connect_clicked(move |_| {
                    if let Some(panel) = weak.upgrade() {
                        panel.send(&Request::Refresh);
                    }
                });
            }
            {
                let weak = weak.clone();
                pages.connect_visible_child_name_notify(move |pages| {
                    let page = pages.visible_child_name().as_deref().and_then(page_of_name);
                    if let (Some(panel), Some(page)) = (weak.upgrade(), page) {
                        panel.send(&Request::Subscribe { page });
                    }
                });
            }
            Panel {
                window,
                toasts,
                root,
                pages,
                status,
                refresh,
                cpu,
                memory,
                link: RefCell::new(None),
                generation: Cell::new(0),
                connecting: Cell::new(false),
            }
        })
    }

    fn current_page(&self) -> Page {
        self.pages
            .visible_child_name()
            .as_deref()
            .and_then(page_of_name)
            .unwrap_or(Page::Cpu)
    }

    fn open(self: &Rc<Panel>, page: Page) {
        // Subscribes through the switcher's notify when the page changes.
        self.pages.set_visible_child_name(page_name(page));
        self.window.present();
        if self.link.borrow().is_none() && !self.connecting.get() {
            self.connect();
        }
    }

    fn connect(self: &Rc<Panel>) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.connecting.set(true);
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(|| {
                let path = socket_path(std::env::var_os("XDG_RUNTIME_DIR").as_deref())?;
                Link::connect(&path, LINK_TIMEOUT)
            })
            .await
            .unwrap_or_else(|_| {
                Err(LinkError::Handshake(
                    "the connecting thread panicked".to_owned(),
                ))
            });
            let Some(panel) = weak.upgrade() else { return };
            if panel.generation.get() != generation {
                return;
            }
            panel.connecting.set(false);
            match result {
                Ok(link) => panel.attach(link, generation),
                Err(error) => panel.fail(generation, &error.to_string()),
            }
        });
    }

    fn attach(self: &Rc<Panel>, link: Link, generation: u64) {
        let events = link.events.clone();
        self.link.replace(Some(link));
        self.root.set_visible_child_name("pages");
        self.refresh.set_sensitive(true);
        self.send(&Request::Subscribe {
            page: self.current_page(),
        });
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(incoming) = events.recv().await {
                let Some(panel) = weak.upgrade() else { return };
                if panel.generation.get() != generation {
                    return;
                }
                let event = match incoming {
                    Incoming::Event(event) => event,
                    Incoming::Closed(closed) => {
                        panel.fail(generation, &closed.to_string());
                        return;
                    }
                };
                match *event {
                    Event::Cpu(page) => panel.cpu.show(&page),
                    Event::Memory(page) => panel.memory.show(&page),
                    // The reader turns an error line into `Closed`.
                    Event::Error { .. } => {}
                }
            }
        });
    }

    fn send(&self, request: &Request) {
        let result = match self.link.borrow().as_ref() {
            Some(link) => link.send(request),
            None => return,
        };
        if let Err(error) = result {
            self.fail(
                self.generation.get(),
                &format!("the connection to StatsBar failed: {error}"),
            );
        }
    }

    fn fail(&self, generation: u64, message: &str) {
        if self.generation.get() != generation {
            return;
        }
        // Dropping the link closes the socket and ends its reader thread.
        self.link.replace(None);
        self.refresh.set_sensitive(false);
        self.status.set_description(Some(message));
        self.root.set_visible_child_name("error");
    }

    fn confirm_quit(self: &Rc<Panel>, target: QuitTarget) {
        let dialog = adw::AlertDialog::new(
            Some(&format!("Quit {}?", target.name)),
            Some(
                "It is asked to close, as when you close it yourself. Unsaved work in it may be lost.",
            ),
        );
        dialog.add_responses(&[("cancel", "Cancel"), ("quit", "Quit")]);
        dialog.set_response_appearance("quit", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Some(window) = weak.upgrade().map(|panel| panel.window.clone()) else {
                return;
            };
            let response = dialog.choose_future(Some(&window)).await;
            let Some(panel) = weak.upgrade() else { return };
            if response != "quit" {
                return;
            }
            let text = match quit(Path::new(PROC), &target) {
                Ok(()) => format!("Asked {} to quit", target.name),
                Err(error) => format!("Could not quit {}: {error}", target.name),
            };
            panel.toasts.add_toast(adw::Toast::new(&text));
        });
    }
}
