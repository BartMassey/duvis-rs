// Copyright (c) 2014 Bart Massey
// [This program is licensed under the "MIT License"]
// See LICENSE.txt in the source distribution for license terms.
//
// GTK4 + Cairo visualization. Fairly literal port of the
// original duvis-andrew/graphics.c: recursive rectangle
// split, orientation flips with aspect ratio, header band
// labels each node, children get proportional area.

use std::rc::Rc;

use gtk4::cairo;
use gtk4::glib;
use gtk4::prelude::*;

use crate::tree::Duvis;

const HEADER: f64 = 25.0;
const MIN_DIM: f64 = 10.0;
const FONT_SIZE: f64 = 20.0;
const DEFAULT_WIDTH: i32 = 600;
const DEFAULT_HEIGHT: i32 = 480;
const APP_ID: &str = "org.duvis.viewer";

fn setup_cairo(cr: &cairo::Context) {
    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.select_font_face(
        "Helvetica",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    cr.set_font_size(FONT_SIZE);
    cr.set_line_width(1.0);
    cr.set_line_join(cairo::LineJoin::Miter);
}

fn draw_node(
    cr: &cairo::Context,
    duvis: &Duvis,
    idx: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    let e = duvis.entry(idx);

    cr.rectangle(x, y, width, height);
    let _ = cr.stroke();

    cr.move_to(x + 5.0, y + 20.0);
    if e.depth() == 0 {
        let comps = e.components();
        let _ = cr.show_text(&comps[0].to_string_lossy());
        for c in comps.iter().take(duvis.base_depth()).skip(1) {
            let _ = cr.show_text("/");
            let _ = cr.show_text(&c.to_string_lossy());
        }
    } else {
        let leaf = e.components().last().unwrap();
        let _ = cr.show_text(&leaf.to_string_lossy());
    }
    let _ = cr.show_text(&format!(" ({})", e.size()));
}

fn draw_tree(
    cr: &cairo::Context,
    duvis: &Duvis,
    idx: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    if width < MIN_DIM || height < MIN_DIM {
        return;
    }

    draw_node(cr, duvis, idx, x, y, width, height);

    let children = duvis.entry(idx).children();
    if children.is_empty() {
        return;
    }

    let child_y = y + HEADER;
    let child_height = height - HEADER;
    if child_height < MIN_DIM {
        return;
    }

    let total_size: u64 = children.iter().map(|&c| duvis.entry(c).size()).sum();
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
        let raw = (duvis.entry(c).size() as f64 / total_size as f64) * span;
        let size = raw.max(1.0).min(remaining);

        if horizontal {
            draw_tree(cr, duvis, c, child_x, child_y, size, child_height);
            child_x += size;
        } else {
            draw_tree(
                cr,
                duvis,
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

fn build_drawing_area(state: Rc<(Duvis, usize)>) -> gtk4::DrawingArea {
    let darea = gtk4::DrawingArea::builder()
        .content_width(DEFAULT_WIDTH)
        .content_height(DEFAULT_HEIGHT)
        .build();
    darea.set_draw_func(move |_, cr, w, h| {
        setup_cairo(cr);
        draw_tree(cr, &state.0, state.1, 0.0, 0.0, w as f64, h as f64);
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
pub fn run(duvis: Duvis, root_idx: usize) -> i32 {
    let state = Rc::new((duvis, root_idx));
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
