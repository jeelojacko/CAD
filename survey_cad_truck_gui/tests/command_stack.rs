use survey_cad_truck_gui::commands::{CommandStack, Command, Context, spawn_point};
use survey_cad_truck_gui::truck_backend::TruckBackend;
use survey_cad::point_database::PointDatabase;
use survey_cad::geometry::Point;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn undo_redo_point() {
    let points = Rc::new(RefCell::new(PointDatabase::new()));
    let styles = Rc::new(RefCell::new(Vec::new()));
    let lines = Rc::new(RefCell::new(Vec::new()));
    let line_styles = Rc::new(RefCell::new(Vec::new()));
    let dims = Rc::new(RefCell::new(Vec::new()));
    let backend = Rc::new(RefCell::new(TruckBackend::new(1,1)));

    let mut stack = CommandStack::new();
    let mut ctx = Context {
        points: &points,
        point_styles: &styles,
        lines: &lines,
        line_styles: &line_styles,
        dimensions: &dims,
        backend: &backend,
    };

    let p = Point::new(1.0, 2.0);
    spawn_point(&points, &styles, &backend, p);
    stack.push(Command::RemovePoint { index: 0, point: p });

    assert_eq!(points.borrow().len(), 1);
    stack.undo(&ctx);
    assert_eq!(points.borrow().len(), 0);
    stack.redo(&ctx);
    assert_eq!(points.borrow().len(), 1);
}

#[test]
fn pushing_clears_redo() {
    let points = Rc::new(RefCell::new(PointDatabase::new()));
    let styles = Rc::new(RefCell::new(Vec::new()));
    let lines = Rc::new(RefCell::new(Vec::new()));
    let line_styles = Rc::new(RefCell::new(Vec::new()));
    let dims = Rc::new(RefCell::new(Vec::new()));
    let backend = Rc::new(RefCell::new(TruckBackend::new(1,1)));
    let mut stack = CommandStack::new();
    let mut ctx = Context {
        points: &points,
        point_styles: &styles,
        lines: &lines,
        line_styles: &line_styles,
        dimensions: &dims,
        backend: &backend,
    };

    let p1 = Point::new(0.0,0.0);
    spawn_point(&points, &styles, &backend, p1);
    stack.push(Command::RemovePoint { index: 0, point: p1 });

    let p2 = Point::new(1.0,1.0);
    spawn_point(&points, &styles, &backend, p2);
    stack.push(Command::RemovePoint { index: 1, point: p2 });

    assert_eq!(points.borrow().len(), 2);
    stack.undo(&ctx); // removes p2
    assert_eq!(points.borrow().len(), 1);

    let p3 = Point::new(2.0,2.0);
    spawn_point(&points, &styles, &backend, p3);
    stack.push(Command::RemovePoint { index: 1, point: p3 });
    assert_eq!(points.borrow().len(), 2);

    stack.redo(&ctx); // should do nothing
    assert_eq!(points.borrow().len(), 2);
}
