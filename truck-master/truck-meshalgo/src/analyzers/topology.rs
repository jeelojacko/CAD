use super::*;
use itertools::Itertools;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use truck_topology::shell::ShellCondition;

/// Extracts boundaries, and check shell condition: closed or orientation.
pub trait Topology {
    /// Returns a vector of all boundaries as line strip.
    fn extract_boundaries(&self) -> Vec<Vec<usize>>;
    /// Determines the shell conditions: non-regular, regular, oriented, or closed.  
    /// The complexity increases in proportion to the number of edges.
    ///
    /// Examples for each condition can be found on the page of
    /// [`ShellCondition`](https://docs.rs/truck-topology/0.2.0/truck_topology/shell/enum.ShellCondition.html).
    fn shell_condition(&self) -> ShellCondition;
}

#[derive(Clone, Debug)]
struct Boundaries {
    checked: HashSet<[usize; 2]>,
    boundary: HashMap<[usize; 2], bool>,
    condition: ShellCondition,
}

impl Boundaries {
    #[inline(always)]
    fn new() -> Self {
        Boundaries {
            checked: Default::default(),
            boundary: Default::default(),
            condition: ShellCondition::Oriented,
        }
    }
    #[inline(always)]
    fn insert(&mut self, edge: [Vertex; 2]) {
        let ori = edge[0].pos < edge[1].pos;
        let edge = match ori {
            true => [edge[0].pos, edge[1].pos],
            false => [edge[1].pos, edge[0].pos],
        };
        self.condition = self.condition
            & match (self.checked.insert(edge), self.boundary.insert(edge, ori)) {
                (true, None) => ShellCondition::Oriented,
                (false, None) => ShellCondition::Irregular,
                (true, Some(_)) => panic!("unexpected case!"),
                (false, Some(ori0)) => {
                    self.boundary.remove(&edge);
                    match ori == ori0 {
                        true => ShellCondition::Regular,
                        false => ShellCondition::Oriented,
                    }
                }
            };
    }

    #[inline(always)]
    fn condition(&self) -> ShellCondition {
        if self.condition == ShellCondition::Oriented && self.boundary.is_empty() {
            ShellCondition::Closed
        } else {
            self.condition
        }
    }
}

impl FromIterator<[Vertex; 2]> for Boundaries {
    fn from_iter<I: IntoIterator<Item = [Vertex; 2]>>(iter: I) -> Boundaries {
        let mut boundaries = Boundaries::new();
        iter.into_iter().for_each(|edge| boundaries.insert(edge));
        boundaries
    }
}

fn face_edge_iter<T: Copy>(face: &[T]) -> impl Iterator<Item = [T; 2]> + '_ {
    face.iter().circular_tuple_windows().map(|(v, w)| [*v, *w])
}

impl Topology for Faces {
    fn extract_boundaries(&self) -> Vec<Vec<usize>> {
        let boundaries: Boundaries = self.face_iter().flat_map(face_edge_iter).collect();
        let mut vemap: HashMap<usize, usize> = boundaries
            .boundary
            .into_iter()
            .map(|(edge, ori)| match ori {
                true => (edge[0], edge[1]),
                false => (edge[1], edge[0]),
            })
            .collect();

        let mut res = Vec::new();
        while !vemap.is_empty() {
            let mut wire = Vec::new();
            let front = vemap.iter().next().unwrap();
            let front = (*front.0, *front.1);
            vemap.remove(&front.0);
            wire.push(front.0);
            let mut cursor = front.1;
            while cursor != front.0 {
                wire.push(cursor);
                cursor = vemap.remove(&cursor).unwrap_or(front.0);
            }
            res.push(wire);
        }
        res
    }
    fn shell_condition(&self) -> ShellCondition {
        self.face_iter()
            .flat_map(face_edge_iter)
            .collect::<Boundaries>()
            .condition()
    }
}

impl Topology for PolygonMesh {
    fn extract_boundaries(&self) -> Vec<Vec<usize>> { self.faces().extract_boundaries() }
    fn shell_condition(&self) -> ShellCondition { self.faces().shell_condition() }
}
