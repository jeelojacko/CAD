use crate::geometry::Point;

/// Group of point IDs with a name.
#[derive(Debug, Clone, Default)]
pub struct PointGroup {
    pub name: String,
    pub point_ids: Vec<usize>,
}

/// Simple in-memory database for survey points.
#[derive(Debug, Clone, Default)]
pub struct PointDatabase {
    points: Vec<Point>,
    groups: Vec<PointGroup>,
}

impl std::ops::Deref for PointDatabase {
    type Target = Vec<Point>;
    fn deref(&self) -> &Self::Target {
        &self.points
    }
}

impl std::ops::DerefMut for PointDatabase {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.points
    }
}

impl PointDatabase {
    /// Create a new empty database.
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            groups: Vec::new(),
        }
    }

    /// Returns a slice of all points.
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    /// Returns a mutable slice of all points.
    pub fn points_mut(&mut self) -> &mut [Point] {
        &mut self.points
    }

    /// Adds a point and returns its ID.
    pub fn add_point(&mut self, point: Point) -> usize {
        self.points.push(point);
        self.points.len() - 1
    }

    /// Updates an existing point.
    pub fn update_point(&mut self, id: usize, point: Point) -> bool {
        if let Some(p) = self.points.get_mut(id) {
            *p = point;
            true
        } else {
            false
        }
    }

    /// Removes the point with the given ID.
    pub fn remove_point(&mut self, id: usize) -> Option<Point> {
        if id >= self.points.len() {
            return None;
        }
        for g in &mut self.groups {
            g.point_ids.retain(|&pid| pid != id);
            for pid in &mut g.point_ids {
                if *pid > id {
                    *pid -= 1;
                }
            }
        }
        Some(self.points.remove(id))
    }

    /// Adds a new group and returns its ID.
    pub fn add_group<S: Into<String>>(&mut self, name: S) -> usize {
        self.groups.push(PointGroup {
            name: name.into(),
            point_ids: Vec::new(),
        });
        self.groups.len() - 1
    }

    /// Removes a group.
    pub fn remove_group(&mut self, id: usize) -> Option<PointGroup> {
        if id >= self.groups.len() {
            None
        } else {
            Some(self.groups.remove(id))
        }
    }

    /// Assigns a point to a group.
    pub fn assign_point(&mut self, point_id: usize, group_id: usize) -> bool {
        if point_id >= self.points.len() || group_id >= self.groups.len() {
            return false;
        }
        let g = &mut self.groups[group_id];
        if !g.point_ids.contains(&point_id) {
            g.point_ids.push(point_id);
        }
        true
    }

    /// Removes a point from a group.
    pub fn remove_point_from_group(&mut self, point_id: usize, group_id: usize) -> bool {
        if let Some(g) = self.groups.get_mut(group_id) {
            let len = g.point_ids.len();
            g.point_ids.retain(|&pid| pid != point_id);
            len != g.point_ids.len()
        } else {
            false
        }
    }

    /// Returns an iterator over all points with their IDs.
    pub fn iter_points(&self) -> impl Iterator<Item = (usize, &Point)> {
        self.points.iter().enumerate()
    }

    /// Returns an iterator over all groups with their IDs.
    pub fn iter_groups(&self) -> impl Iterator<Item = (usize, &PointGroup)> {
        self.groups.iter().enumerate()
    }

    /// Returns an iterator over points in a specific group.
    pub fn iter_group_points(
        &self,
        group_id: usize,
    ) -> Option<impl Iterator<Item = (usize, &Point)>> {
        if group_id >= self.groups.len() {
            None
        } else {
            Some(
                self.groups[group_id]
                    .point_ids
                    .iter()
                    .filter_map(move |&pid| self.points.get(pid).map(|p| (pid, p))),
            )
        }
    }

    /// Clears all points and groups.
    pub fn clear(&mut self) {
        self.points.clear();
        self.groups.clear();
    }
}
