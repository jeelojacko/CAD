use std::cell::RefCell;
use std::rc::Rc;

pub use pipe_network::{Network, Pipe, Structure};

use crate::truck_backend::TruckBackend;

pub struct PipeEditor {
    pub network: Network,
    backend: Rc<RefCell<TruckBackend>>,
    structure_points: Vec<usize>,
    pipe_lines: Vec<usize>,
}

impl PipeEditor {
    pub fn new(backend: Rc<RefCell<TruckBackend>>) -> Self {
        Self {
            network: Network::default(),
            backend,
            structure_points: Vec::new(),
            pipe_lines: Vec::new(),
        }
    }

    pub fn set_network(&mut self, net: Network) {
        self.clear_render();
        self.network = net;
        self.refresh_render();
    }

    pub fn refresh_render(&mut self) {
        self.clear_render();
        for s in &self.network.structures {
            let idx = self.backend.borrow_mut().add_point(s.x, s.y, s.z);
            self.structure_points.push(idx);
        }
        for p in &self.network.pipes {
            let start = self
                .network
                .structures
                .iter()
                .find(|s| s.id == p.from);
            let end = self
                .network
                .structures
                .iter()
                .find(|s| s.id == p.to);
            if let (Some(a), Some(b)) = (start, end) {
                let idx = self.backend.borrow_mut().add_line(
                    [a.x, a.y, a.z],
                    [b.x, b.y, b.z],
                    [0.0, 1.0, 0.0, 1.0],
                    1.0,
                );
                self.pipe_lines.push(idx);
            }
        }
    }

    pub fn clear_render(&mut self) {
        for idx in self.pipe_lines.drain(..).rev() {
            self.backend.borrow_mut().remove_line(idx);
        }
        for idx in self.structure_points.drain(..).rev() {
            self.backend.borrow_mut().remove_point(idx);
        }
    }
}
