// Copyright (c) 2014 Bart Massey
// [This program is licensed under the "MIT License"]
// See LICENSE.txt in the source distribution for license terms.
//
// GTK4 + Cairo visualization. Recursive rectangle split with
// aspect-ratio-driven orientation, after Andrew Graham's
// graphics.c. Each node gets a header band labeling it; its
// children fill the remaining area, proportional to size.

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

struct State {
    duvis: Duvis,
    root: usize,
    font_size: f64,
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

fn draw_node(
    cr: &cairo::Context,
    state: &State,
    idx: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    let e = state.duvis.entry(idx);

    cr.rectangle(x, y, width, height);
    let _ = cr.stroke();

    // Clip text to the rectangle interior so labels never
    // spill into neighboring nodes.
    let _ = cr.save();
    cr.rectangle(x, y, width, height);
    cr.clip();

    cr.move_to(x + TEXT_INSET, y + state.font_size);
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

fn draw_tree(
    cr: &cairo::Context,
    state: &State,
    idx: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    if width < MIN_DIM || height < MIN_DIM {
        return;
    }

    draw_node(cr, state, idx, x, y, width, height);

    let children = state.duvis.entry(idx).children();
    if children.is_empty() {
        return;
    }

    let header = state.header();
    let child_y = y + header;
    let child_height = height - header;
    if child_height < MIN_DIM {
        return;
    }

    let total_size: u64 = children.iter().map(|&c| state.duvis.entry(c).size()).sum();
    if total_size == 0 {
        return;
    }

    let horizontal = width > height;
    let span = if horizontal { width } else { child_height };
    let mut child_x = x;
    let mut remaining = span;

    for &c in children {
        if remaining <= 0.0 {
            break;
        }
        let raw = (state.duvis.entry(c).size() as f64 / total_size as f64) * span;
        let size = raw.max(1.0).min(remaining);

        if horizontal {
            draw_tree(cr, state, c, child_x, child_y, size, child_height);
            child_x += size;
        } else {
            draw_tree(
                cr,
                state,
                c,
                child_x,
                child_y + (child_height - remaining),
                width,
                size,
            );
        }
        remaining -= size;
    }
}

fn build_drawing_area(state: Rc<State>) -> gtk4::DrawingArea {
    let darea = gtk4::DrawingArea::builder()
        .content_width(DEFAULT_WIDTH)
        .content_height(DEFAULT_HEIGHT)
        .build();
    darea.set_draw_func(move |_, cr, w, h| {
        setup_cairo(cr, state.font_size);
        draw_tree(cr, &state, state.root, 0.0, 0.0, w as f64, h as f64);
    });
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
        root: root_idx,
        font_size,
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
