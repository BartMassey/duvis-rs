// Copyright (c) 2014 Bart Massey
// [This program is licensed under the "MIT License"]
// See LICENSE.txt in the source distribution for license terms.
//
// GTK4 + Cairo visualization. Recursive rectangle split with
// aspect-ratio-driven orientation, after Andrew Graham's
// graphics.c. Each node gets a header band labeling it; its
// children fill the remaining area, proportional to size.
//
// Click-zoom: clicking a rectangle promotes that node to the
// new focus root. Ancestors above the focus are still drawn
// (as nested header bands) and remain clickable, so a click
// on any ancestor zooms back out.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::cairo;
use gtk4::glib;
use gtk4::prelude::*;

use crate::tree::Duvis;

const MIN_DIM: f64 = 10.0;
const TEXT_INSET: f64 = 5.0;
const HEADER_PADDING: f64 = 5.0;
const DEFAULT_WIDTH: i32 = 600;
const DEFAULT_HEIGHT: i32 = 480;
const APP_ID: &str = "org.duvis.viewer";

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
    fn area(&self) -> f64 {
        self.w * self.h
    }
}

#[derive(Clone)]
struct HitRect {
    /// Full path from the original root to this node.
    path: Vec<usize>,
    rect: Rect,
}

struct State {
    duvis: Duvis,
    font_size: f64,
    /// Focus path, root → currently focused node. Always non-empty.
    path: RefCell<Vec<usize>>,
    /// Repopulated each draw; consumed by the click handler.
    hits: RefCell<Vec<HitRect>>,
}

impl State {
    fn header(&self) -> f64 {
        self.font_size + HEADER_PADDING
    }
}

fn setup_cairo(cr: &cairo::Context, font_size: f64) {
    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.select_font_face(
        "Helvetica",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    cr.set_font_size(font_size);
    cr.set_line_width(1.0);
    cr.set_line_join(cairo::LineJoin::Miter);
}

fn draw_node(cr: &cairo::Context, state: &State, idx: usize, r: Rect) {
    let e = state.duvis.entry(idx);

    cr.rectangle(r.x, r.y, r.w, r.h);
    let _ = cr.stroke();

    // Clip text to the rectangle interior so labels never
    // spill into neighboring nodes.
    let _ = cr.save();
    cr.rectangle(r.x, r.y, r.w, r.h);
    cr.clip();

    cr.move_to(r.x + TEXT_INSET, r.y + state.font_size);
    if e.depth() == 0 {
        let comps = e.components();
        let _ = cr.show_text(&comps[0].to_string_lossy());
        for c in comps.iter().take(state.duvis.base_depth()).skip(1) {
            let _ = cr.show_text("/");
            let _ = cr.show_text(&c.to_string_lossy());
        }
    } else {
        let leaf = e.components().last().unwrap();
        let _ = cr.show_text(&leaf.to_string_lossy());
    }
    let _ = cr.show_text(&format!(" ({})", e.size()));

    let _ = cr.restore();
}

fn draw_tree(cr: &cairo::Context, state: &State, idx: usize, r: Rect, ancestors: &[usize]) {
    if r.w < MIN_DIM || r.h < MIN_DIM {
        return;
    }

    let mut my_path: Vec<usize> = ancestors.to_vec();
    my_path.push(idx);
    state.hits.borrow_mut().push(HitRect {
        path: my_path.clone(),
        rect: r,
    });

    draw_node(cr, state, idx, r);

    let children = state.duvis.entry(idx).children();
    if children.is_empty() {
        return;
    }

    let header = state.header();
    let child_y = r.y + header;
    let child_height = r.h - header;
    if child_height < MIN_DIM {
        return;
    }

    let total_size: u64 = children.iter().map(|&c| state.duvis.entry(c).size()).sum();
    if total_size == 0 {
        return;
    }

    let horizontal = r.w > r.h;
    let span = if horizontal { r.w } else { child_height };
    let mut child_x = r.x;
    let mut remaining = span;

    for &c in children {
        if remaining <= 0.0 {
            break;
        }
        let raw = (state.duvis.entry(c).size() as f64 / total_size as f64) * span;
        let size = raw.max(1.0).min(remaining);

        let sub = if horizontal {
            Rect {
                x: child_x,
                y: child_y,
                w: size,
                h: child_height,
            }
        } else {
            Rect {
                x: child_x,
                y: child_y + (child_height - remaining),
                w: r.w,
                h: size,
            }
        };
        draw_tree(cr, state, c, sub, &my_path);
        if horizontal {
            child_x += size;
        }
        remaining -= size;
    }
}

/// Top-level draw entry: paints ancestor header bands for the
/// focus path's prefix, then renders the focused subtree.
fn draw(cr: &cairo::Context, state: &State, width: f64, height: f64) {
    setup_cairo(cr, state.font_size);
    state.hits.borrow_mut().clear();

    let path = state.path.borrow().clone();
    let header = state.header();

    let mut cur_y = 0.0;
    let mut cur_h = height;

    // Ancestors above the focus: render as nested headers,
    // each occupying its full remaining area so a click
    // anywhere inside the area but above the next band picks
    // that ancestor (smallest-area wins, see handle_click).
    for depth in 0..path.len().saturating_sub(1) {
        if width < MIN_DIM || cur_h < MIN_DIM {
            return;
        }
        let idx = path[depth];
        let r = Rect {
            x: 0.0,
            y: cur_y,
            w: width,
            h: cur_h,
        };
        state.hits.borrow_mut().push(HitRect {
            path: path[..=depth].to_vec(),
            rect: r,
        });
        draw_node(cr, state, idx, r);
        cur_y += header;
        cur_h -= header;
    }

    let focus_idx = *path.last().unwrap();
    let ancestors = &path[..path.len() - 1];
    let focus_rect = Rect {
        x: 0.0,
        y: cur_y,
        w: width,
        h: cur_h,
    };
    draw_tree(cr, state, focus_idx, focus_rect, ancestors);
}

fn handle_click(state: &State, darea: &gtk4::DrawingArea, x: f64, y: f64) {
    let new_path = {
        let hits = state.hits.borrow();
        let mut best: Option<&HitRect> = None;
        for h in hits.iter() {
            if h.rect.contains(x, y) && best.is_none_or(|b| h.rect.area() < b.rect.area()) {
                best = Some(h);
            }
        }
        best.map(|h| h.path.clone())
    };
    let Some(new_path) = new_path else {
        return;
    };
    let mut current = state.path.borrow_mut();
    if *current != new_path {
        *current = new_path;
        drop(current);
        darea.queue_draw();
    }
}

fn build_drawing_area(state: Rc<State>) -> gtk4::DrawingArea {
    let darea = gtk4::DrawingArea::builder()
        .content_width(DEFAULT_WIDTH)
        .content_height(DEFAULT_HEIGHT)
        .build();

    let state_for_draw = state.clone();
    darea.set_draw_func(move |_, cr, w, h| {
        draw(cr, &state_for_draw, w as f64, h as f64);
    });

    let gesture = gtk4::GestureClick::new();
    let state_for_click = state.clone();
    let darea_handle = darea.clone();
    gesture.connect_released(move |_, _, x, y| {
        handle_click(&state_for_click, &darea_handle, x, y);
    });
    darea.add_controller(gesture);

    darea
}

fn build_window(app: &gtk4::Application, darea: &gtk4::DrawingArea) -> gtk4::ApplicationWindow {
    gtk4::ApplicationWindow::builder()
        .application(app)
        .title("Duvis")
        .default_width(DEFAULT_WIDTH)
        .default_height(DEFAULT_HEIGHT)
        .child(darea)
        .build()
}

/// Render the given tree in a GTK4 window. Blocks until the
/// window is closed. Returns the GTK exit code as an `i32`.
pub fn run(duvis: Duvis, root_idx: usize, font_size: f64) -> i32 {
    let state = Rc::new(State {
        duvis,
        font_size,
        path: RefCell::new(vec![root_idx]),
        hits: RefCell::new(Vec::new()),
    });
    let app = gtk4::Application::builder().application_id(APP_ID).build();

    app.connect_activate(move |app| {
        let darea = build_drawing_area(state.clone());
        let window = build_window(app, &darea);
        window.present();
    });

    // Hand gtk an empty argv so it doesn't try to parse our CLI flags.
    let code = app.run_with_args::<&str>(&[]);
    glib::ExitCode::value(&code)
}
