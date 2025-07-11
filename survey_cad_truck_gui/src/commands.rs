use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use survey_cad::geometry::{Point, LinearDimension};
use survey_cad::point_database::PointDatabase;
use truck_modeling::base::Point3;

use crate::truck_backend::TruckBackend;

#[derive(Clone)]
pub enum Command {
    RemovePoint { index: usize, point: Point },
    AddPoint { index: usize, point: Point },
    RemoveLine { index: usize, line: (Point, Point) },
    AddLine { index: usize, line: (Point, Point) },
    RemoveDimension { index: usize, dim: LinearDimension },
    AddDimension { index: usize, dim: LinearDimension },
    TinDeleteVertex { surface: usize, index: usize, point: Point3 },
    TinAddVertex { surface: usize, index: usize, point: Point3 },
}

pub struct CommandStack {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl Default for CommandStack {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct MacroRecorder {
    pub file: Option<std::fs::File>,
}

#[derive(Default)]
pub struct MacroPlaying(pub bool);

pub fn record_macro(rec: &mut MacroRecorder, line: &str) {
    if let Some(file) = &mut rec.file {
        let _ = writeln!(file, "{line}");
    }
}

pub struct Context<'a> {
    pub points: &'a Rc<RefCell<PointDatabase>>,
    pub point_styles: &'a Rc<RefCell<Vec<usize>>>,
    pub lines: &'a Rc<RefCell<Vec<(Point, Point)>>>,
    pub line_styles: &'a Rc<RefCell<Vec<usize>>>,
    pub dimensions: &'a Rc<RefCell<Vec<LinearDimension>>>,
    pub backend: &'a Rc<RefCell<TruckBackend>>,
}

impl CommandStack {
    pub fn new() -> Self {
        Self { undo: Vec::new(), redo: Vec::new() }
    }

    pub fn push(&mut self, cmd: Command) {
        self.undo.push(cmd);
        self.redo.clear();
    }

    pub fn undo(&mut self, ctx: &Context) {
        if let Some(cmd) = self.undo.pop() {
            let inverse = apply_command(&cmd, ctx);
            self.redo.push(inverse);
        }
    }

    pub fn redo(&mut self, ctx: &Context) {
        if let Some(cmd) = self.redo.pop() {
            let inverse = apply_command(&cmd, ctx);
            self.undo.push(inverse);
        }
    }
}

pub fn apply_command(cmd: &Command, ctx: &Context) -> Command {
    match cmd {
        Command::RemovePoint { index, point } => {
            ctx.points.borrow_mut().remove(*index);
            ctx.point_styles.borrow_mut().remove(*index);
            ctx.backend.borrow_mut().remove_point(*index);
            Command::AddPoint { index: *index, point: *point }
        }
        Command::AddPoint { index, point } => {
            ctx.points.borrow_mut().insert(*index, *point);
            ctx.point_styles.borrow_mut().insert(*index, 0);
            ctx.backend.borrow_mut().add_point(point.x, point.y, 0.0);
            Command::RemovePoint { index: *index, point: *point }
        }
        Command::RemoveLine { index, line } => {
            ctx.lines.borrow_mut().remove(*index);
            ctx.line_styles.borrow_mut().remove(*index);
            ctx.backend.borrow_mut().remove_line(*index);
            Command::AddLine { index: *index, line: *line }
        }
        Command::AddLine { index, line } => {
            ctx.lines.borrow_mut().insert(*index, *line);
            ctx.line_styles.borrow_mut().insert(*index, 0);
            ctx.backend.borrow_mut().add_line(
                [line.0.x, line.0.y, 0.0],
                [line.1.x, line.1.y, 0.0],
                [1.0, 1.0, 1.0, 1.0],
                1.0,
            );
            Command::RemoveLine { index: *index, line: *line }
        }
        Command::RemoveDimension { index, dim } => {
            ctx.backend.borrow_mut().remove_dimension(*index);
            ctx.dimensions.borrow_mut().remove(*index);
            Command::AddDimension { index: *index, dim: dim.clone() }
        }
        Command::AddDimension { index, dim } => {
            ctx.dimensions.borrow_mut().insert(*index, dim.clone());
            ctx.backend.borrow_mut().add_dimension(
                [dim.start.x, dim.start.y, 0.0],
                [dim.end.x, dim.end.y, 0.0],
                [1.0, 1.0, 1.0, 1.0],
                1.0,
            );
            Command::RemoveDimension { index: *index, dim: dim.clone() }
        }
        Command::TinDeleteVertex { surface, index, point } => {
            ctx.backend.borrow_mut().delete_vertex(*surface, *index);
            Command::TinAddVertex { surface: *surface, index: *index, point: *point }
        }
        Command::TinAddVertex { surface, index, point } => {
            ctx.backend.borrow_mut().add_vertex(*surface, *point);
            Command::TinDeleteVertex { surface: *surface, index: *index, point: *point }
        }
    }
}

pub fn spawn_point(
    points: &Rc<RefCell<PointDatabase>>,
    styles: &Rc<RefCell<Vec<usize>>>,
    backend: &Rc<RefCell<TruckBackend>>,
    p: Point,
) {
    points.borrow_mut().push(p);
    styles.borrow_mut().push(0);
    backend.borrow_mut().add_point(p.x, p.y, 0.0);
}

pub fn spawn_line(
    points: &Rc<RefCell<PointDatabase>>,
    lines: &Rc<RefCell<Vec<(Point, Point)>>>,
    point_styles: &Rc<RefCell<Vec<usize>>>,
    line_styles: &Rc<RefCell<Vec<usize>>>,
    backend: &Rc<RefCell<TruckBackend>>,
    a: Point,
    b: Point,
) {
    spawn_point(points, point_styles, backend, a);
    spawn_point(points, point_styles, backend, b);
    lines.borrow_mut().push((a, b));
    line_styles.borrow_mut().push(0);
    backend
        .borrow_mut()
        .add_line([a.x, a.y, 0.0], [b.x, b.y, 0.0], [1.0, 1.0, 1.0, 1.0], 1.0);
}
