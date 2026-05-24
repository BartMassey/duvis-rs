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

// Minimum rectangle dimensions are derived from the font size: a rect must
// be tall enough to fit one label band and wide enough for ~8 average
// glyphs (plus inset on each side).
const MIN_CHARS: f64 = 8.0;
const AVG_GLYPH_FRAC: f64 = 0.55; // Helvetica average advance ≈ 0.55 × font size
const TEXT_INSET: f64 = 5.0;
const HEADER_PADDING: f64 = 5.0;
// Cross-axis gutter: a strip of the parent's body left visible on one
// side of the children (bottom for horizontal splits, right for
// vertical splits) so containment is obvious without doubling padding
// on every side.
const CHILD_INSET: f64 = 3.0;
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
    fn min_w(&self) -> f64 {
        MIN_CHARS * AVG_GLYPH_FRAC * self.font_size + 2.0 * TEXT_INSET
    }
    fn min_h(&self) -> f64 {
        self.header()
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
    if r.w < state.min_w() || r.h < state.min_h() {
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
    let body_x = r.x;
    let body_y = r.y + header;
    let body_w = r.w;
    let body_h = r.h - header;
    if body_w < state.min_w() || body_h < state.min_h() {
        return;
    }

    let total: u64 = children.iter().map(|&c| state.duvis.entry(c).size()).sum();
    if total == 0 {
        return;
    }

    let horizontal = body_w >= body_h;
    let span = if horizontal { body_w } else { body_h };
    let min_slice = if horizontal { state.min_w() } else { state.min_h() };

    // Reserve a CHILD_INSET strip of parent body on the cross axis: bottom
    // for horizontal layouts, right for vertical. Children's split-axis
    // slices remain proportional; only the cross dim shrinks by the gutter.
    let cross_dim = if horizontal {
        body_h - CHILD_INSET
    } else {
        body_w - CHILD_INSET
    };
    let cross_min = if horizontal {
        state.min_h()
    } else {
        state.min_w()
    };
    if cross_dim < cross_min {
        return;
    }

    // Classify children: any whose proportional slice along the split axis
    // is below min_slice goes into the "excess" bucket and is drawn as one
    // composite "+N more" rectangle covering its combined slice. Preserves
    // the slice sizes of the kept children.
    let threshold = min_slice * total as f64 / span;
    let mut kept: Vec<usize> = Vec::with_capacity(children.len());
    let mut excess_count: usize = 0;
    let mut excess_total: u64 = 0;
    for &c in children {
        let sz = state.duvis.entry(c).size();
        if (sz as f64) >= threshold {
            kept.push(c);
        } else {
            excess_count += 1;
            excess_total += sz;
        }
    }

    let excess_slice = (excess_total as f64 / total as f64) * span;
    let has_excess = excess_count > 0 && excess_slice >= min_slice;
    // If the excess bucket can't make a min_slice rectangle of its own,
    // fold its share back into the kept children (renormalize) so the
    // body has no unclaimed gap.
    let denom: u64 = if has_excess { total } else { total - excess_total };
    if denom == 0 {
        return;
    }

    let mut cur_x = body_x;
    let mut cur_y_off = 0.0;
    for &c in &kept {
        let size = state.duvis.entry(c).size() as f64;
        let s = (size / denom as f64) * span;
        let sub = if horizontal {
            Rect {
                x: cur_x,
                y: body_y,
                w: s,
                h: cross_dim,
            }
        } else {
            Rect {
                x: body_x,
                y: body_y + cur_y_off,
                w: cross_dim,
                h: s,
            }
        };
        draw_tree(cr, state, c, sub, &my_path);
        if horizontal {
            cur_x += s;
        } else {
            cur_y_off += s;
        }
    }

    if has_excess {
        let sub = if horizontal {
            Rect {
                x: cur_x,
                y: body_y,
                w: excess_slice,
                h: cross_dim,
            }
        } else {
            Rect {
                x: body_x,
                y: body_y + cur_y_off,
                w: cross_dim,
                h: excess_slice,
            }
        };
        draw_excess(cr, state, sub, excess_count, excess_total);
    }
}

fn draw_excess(cr: &cairo::Context, state: &State, r: Rect, count: usize, size: u64) {
    cr.rectangle(r.x, r.y, r.w, r.h);
    let _ = cr.stroke();
    let _ = cr.save();
    cr.rectangle(r.x, r.y, r.w, r.h);
    cr.clip();
    cr.move_to(r.x + TEXT_INSET, r.y + state.font_size);
    let _ = cr.show_text(&format!("+{} more ({})", count, size));
    let _ = cr.restore();
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
        if width < state.min_w() || cur_h < state.min_h() {
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
