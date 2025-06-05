//! Rendering utilities. Placeholder for drawing CAD entities.

use crate::geometry::Point;

use log::error;
use pixels::{Pixels, SurfaceTexture};
use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

/// Simple rendering of a point. In real application this would draw to screen.
pub fn render_point(p: Point) {
    let _ = env_logger::builder().is_test(true).try_init();

    let event_loop = EventLoop::new().unwrap();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Survey Point")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };
    let window = Arc::new(window);

    let mut pixels = {
        let surface_texture = SurfaceTexture::new(WIDTH, HEIGHT, window.clone());
        Pixels::new(WIDTH, HEIGHT, surface_texture).expect("Failed to create pixel buffer")
    };

    let _ = event_loop.run(move |event, elwt| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => {
            elwt.exit();
        }
        Event::WindowEvent {
            event: WindowEvent::Resized(size),
            ..
        } => {
            let _ = pixels.resize_surface(size.width, size.height);
        }
        Event::WindowEvent {
            event: WindowEvent::RedrawRequested,
            ..
        } => {
            draw_point(pixels.frame_mut(), WIDTH as usize, HEIGHT as usize, p);
            if let Err(err) = pixels.render() {
                error!("pixels.render() failed: {err}");
                elwt.exit();
            }
        }
        Event::AboutToWait => {
            window.request_redraw();
        }
        _ => {}
    });
}

/// Renders a collection of points. The window will close when requested.
pub fn render_points(points: &[Point]) {
    let _ = env_logger::builder().is_test(true).try_init();

    let event_loop = EventLoop::new().unwrap();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Survey Points")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };
    let window = Arc::new(window);

    let mut pixels = {
        let surface_texture = SurfaceTexture::new(WIDTH, HEIGHT, window.clone());
        Pixels::new(WIDTH, HEIGHT, surface_texture).expect("Failed to create pixel buffer")
    };

    let points = points.to_vec();
    let _ = event_loop.run(move |event, elwt| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => elwt.exit(),
        Event::WindowEvent {
            event: WindowEvent::Resized(size),
            ..
        } => {
            let _ = pixels.resize_surface(size.width, size.height);
        }
        Event::WindowEvent {
            event: WindowEvent::RedrawRequested,
            ..
        } => {
            draw_points(pixels.frame_mut(), WIDTH as usize, HEIGHT as usize, &points);
            if let Err(err) = pixels.render() {
                error!("pixels.render() failed: {err}");
                elwt.exit();
            }
        }
        Event::AboutToWait => {
            window.request_redraw();
        }
        _ => {}
    });
}

fn draw_point(frame: &mut [u8], width: usize, height: usize, point: Point) {
    for pix in frame.chunks_exact_mut(4) {
        pix.copy_from_slice(&[0x20, 0x20, 0x20, 0xff]);
    }

    let x = point.x.round() as i32;
    let y = point.y.round() as i32;
    if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
        let idx = ((y as usize) * width + x as usize) * 4;
        frame[idx] = 0xff;
        frame[idx + 1] = 0x00;
        frame[idx + 2] = 0x00;
        frame[idx + 3] = 0xff;
    }
}

fn draw_points(frame: &mut [u8], width: usize, height: usize, points: &[Point]) {
    for pix in frame.chunks_exact_mut(4) {
        pix.copy_from_slice(&[0x20, 0x20, 0x20, 0xff]);
    }

    for point in points {
        let x = point.x.round() as i32;
        let y = point.y.round() as i32;
        if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
            let idx = ((y as usize) * width + x as usize) * 4;
            frame[idx] = 0xff;
            frame[idx + 1] = 0x00;
            frame[idx + 2] = 0x00;
            frame[idx + 3] = 0xff;
        }
    }
}
