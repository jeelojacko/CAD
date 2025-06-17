use crate::*;
use errors::Error;

type Vertex = StandardVertex;

/// can be regarded as a vertex slice
pub trait AsVertexSlice: AsRef<[Self::V]> {
    /// items converted to vertex
    type V: Copy + Into<Vertex>;
}

impl From<&Vertex> for Vertex {
    fn from(v: &Vertex) -> Vertex { *v }
}

impl<T: AsVertexSlice> AsVertexSlice for &T {
    type V = T::V;
}

macro_rules! impl_as_vertex_slice {
    ($vertex: ty) => {
        impl<'a> AsVertexSlice for &'a [$vertex] {
            type V = $vertex;
        }
        impl<const N: usize> AsVertexSlice for [$vertex; N] {
            type V = $vertex;
        }
        impl AsVertexSlice for Vec<$vertex> {
            type V = $vertex;
        }
        impl<'a> AsVertexSlice for &'a [&'a $vertex] {
            type V = &'a $vertex;
        }
        impl<'a> AsVertexSlice for Vec<&'a $vertex> {
            type V = &'a $vertex;
        }
    };
}

impl_as_vertex_slice!(Vertex);

macro_rules! impl_as_vertex {
    (impl From<$vertex: ty> for Vertex { $from: item }) => {
        impl From<$vertex> for Vertex {
            #[inline(always)]
            $from
        }
        impl From<&$vertex> for Vertex {
            #[inline(always)]
            fn from(v: &$vertex) -> Vertex { Vertex::from(*v) }
        }
        impl_as_vertex_slice!($vertex);
    };
}

impl_as_vertex! {
    impl From<(usize, Option<usize>, Option<usize>)> for Vertex {
        fn from(tuple: (usize, Option<usize>, Option<usize>)) -> Vertex {
            Vertex {
                pos: tuple.0,
                uv: tuple.1,
                nor: tuple.2,
            }
        }
    }
}

impl_as_vertex! {
    impl From<[usize; 3]> for Vertex {
        fn from(arr: [usize; 3]) -> Vertex {
            Vertex {
                pos: arr[0],
                uv: Some(arr[1]),
                nor: Some(arr[2]),
            }
        }
    }
}

impl_as_vertex! {
    impl From<usize> for Vertex {
        fn from(idx: usize) -> Vertex {
            Vertex {
                pos: idx,
                uv: None,
                nor: None,
            }
        }
    }
}

impl<T: AsVertexSlice> FromIterator<T> for Faces {
    #[inline(always)]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Faces {
        let mut faces = Faces::default();
        faces.extend(iter);
        faces
    }
}

impl<S: AsRef<[usize]>> FromIterator<S> for Faces<usize> {
    #[inline(always)]
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Faces<usize> {
        let mut faces = Faces::default();
        faces.extend(iter);
        faces
    }
}

#[test]
fn faces_from_iter() {
    let slice: &[&[[usize; 3]]] = &[
        &[[0, 0, 0], [1, 1, 1], [2, 2, 2]],
        &[[0, 0, 0], [2, 2, 2], [3, 3, 3]],
        &[[0, 0, 0], [4, 4, 4], [5, 5, 5], [1, 1, 1]],
    ];
    let _faces = Faces::from_iter(slice);
}

impl<V: Copy> Faces<V> {
    /// Extends faces by an iterator.
    #[inline(always)]
    pub fn extend<U: Copy + Into<V>, T: AsRef<[U]>, I: IntoIterator<Item = T>>(&mut self, iter: I) {
        iter.into_iter().for_each(|face| self.push(face))
    }

    /// Creates faces of a polygon mesh by the vectors of triangle and quadrangle.
    /// # Examples
    /// ```
    /// // Creates faces consisis only triangles.
    /// use truck_polymesh::*;
    /// let tri_faces: Vec<[StandardVertex; 3]> = vec![
    ///     [[0, 0, 0].into(), [1, 1, 1].into(), [2, 2, 2].into()],
    ///     [[0, 0, 0].into(), [2, 2, 2].into(), [3, 3, 3].into()],
    /// ];
    /// let faces = Faces::from_tri_and_quad_faces(tri_faces, Vec::new());
    /// ```
    #[inline(always)]
    pub const fn from_tri_and_quad_faces(tri_faces: Vec<[V; 3]>, quad_faces: Vec<[V; 4]>) -> Self {
        Faces {
            tri_faces,
            quad_faces,
            other_faces: Vec::new(),
        }
    }

    /// Push a face to the faces.
    ///
    /// If `face.len() < 3`, the face is ignored with warning.
    /// # Examples
    /// ```
    /// use truck_polymesh::*;
    /// let mut faces = Faces::<StandardVertex>::default(); // empty faces
    /// faces.push(&[[0, 0, 0], [1, 1, 1], [2, 2, 2]]);
    /// faces.push(&[[3, 3, 3], [0, 0, 0], [2, 2, 2]]);
    /// faces.push(&[[0, 0, 0], [4, 4, 4], [5, 5, 5], [1, 1, 1]]);
    /// faces.push(&[[100, 1000, 10]]); // Wargning: ignored one vertex "face"
    /// ```
    #[inline(always)]
    pub fn push<U: Copy + Into<V>, T: AsRef<[U]>>(&mut self, face: T) {
        let face = face.as_ref();
        match face.len() {
            0 => {}
            1 => {}
            2 => {}
            3 => self
                .tri_faces
                .push([face[0].into(), face[1].into(), face[2].into()]),
            4 => self.quad_faces.push([
                face[0].into(),
                face[1].into(),
                face[2].into(),
                face[3].into(),
            ]),
            _ => self
                .other_faces
                .push(Vec::from_iter(face.iter().map(|v| (*v).into()))),
        }
    }

    /// Returns the vector of triangles.
    #[inline(always)]
    pub const fn tri_faces(&self) -> &Vec<[V; 3]> { &self.tri_faces }

    /// Returns the mutable slice of triangles.
    #[inline(always)]
    pub fn tri_faces_mut(&mut self) -> &mut [[V; 3]] { &mut self.tri_faces }

    /// Returns the vector of quadrangles.
    #[inline(always)]
    pub const fn quad_faces(&self) -> &Vec<[V; 4]> { &self.quad_faces }

    /// Returns the mutable slice of quadrangles.
    #[inline(always)]
    pub fn quad_faces_mut(&mut self) -> &mut [[V; 4]] { &mut self.quad_faces }

    /// Returns the vector of n-gons (n > 4).
    #[inline(always)]
    pub const fn other_faces(&self) -> &Vec<Vec<V>> { &self.other_faces }

    /// Returns the mutable iterator of n-gons (n > 4).
    #[inline(always)]
    pub fn other_faces_mut(&mut self) -> impl Iterator<Item = &mut [V]> {
        self.other_faces.iter_mut().map(|face| face.as_mut())
    }

    /// Returns the iterator of the slice.
    ///
    /// By the internal optimization, this iterator does not runs in the simple order
    /// in which they are registered, but runs order: triangle, square, and the others.
    /// # Examples
    /// ```
    /// use truck_polymesh::*;
    /// let slice: &[&[usize]] = &[
    ///     &[0, 1, 2],
    ///     &[0, 4, 5, 1],
    ///     &[1, 2, 6, 7, 8, 9],
    ///     &[0, 2, 3],
    /// ];
    /// let faces = Faces::<usize>::from_iter(slice);
    /// let mut iter = faces.face_iter();
    /// assert_eq!(iter.next(), Some([0, 1, 2].as_ref()));
    /// assert_eq!(iter.next(), Some([0, 2, 3].as_ref()));
    /// assert_eq!(iter.next(), Some([0, 4, 5, 1].as_ref()));
    /// assert_eq!(iter.next(), Some([1, 2, 6, 7, 8, 9].as_ref()));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline(always)]
    pub fn face_iter(&self) -> impl Iterator<Item = &[V]> {
        self.tri_faces
            .iter()
            .map(|v| v.as_ref())
            .chain(self.quad_faces.iter().map(|v| v.as_ref()))
            .chain(self.other_faces.iter().map(|v| v.as_ref()))
    }

    /// Returns the iterator of the slice.
    ///
    /// By the internal optimization, this iterator does not runs in the simple order
    /// in which they are registered, but runs order: triangle, square, and the others.
    /// cf: [`Faces:face_iter`](./struct.Faces.html#method.face_iter)
    #[inline(always)]
    pub fn face_iter_mut(&mut self) -> impl Iterator<Item = &mut [V]> {
        self.tri_faces
            .iter_mut()
            .map(|v| v.as_mut())
            .chain(self.quad_faces.iter_mut().map(|v| v.as_mut()))
            .chain(self.other_faces.iter_mut().map(|v| v.as_mut()))
    }

    /// Returns true if the faces is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// Returns the number of faces.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.tri_faces.len() + self.quad_faces.len() + self.other_faces.len()
    }

    /// Merges `other` into `self`.
    #[inline(always)]
    pub fn naive_concat(&mut self, other: Self) {
        self.tri_faces.extend(other.tri_faces);
        self.quad_faces.extend(other.quad_faces);
        self.other_faces.extend(other.other_faces);
    }

    #[inline(always)]
    pub(super) fn is_compatible(&self, attrs: &impl Attributes<V>) -> Result<(), Error<V>>
    where V: std::fmt::Debug {
        self.face_iter()
            .flatten()
            .try_for_each(|v| match attrs.get(*v) {
                Some(_) => Ok(()),
                None => Err(Error::OutOfRange(*v)),
            })
    }

    /// Returns iterator with triangulation faces
    ///
    /// # Examples
    /// ```
    /// use truck_polymesh::*;
    /// let slice: &[&[usize]] = &[
    ///     &[0, 1, 2],
    ///     &[0, 4, 5, 1],
    ///     &[1, 2, 6, 7, 8, 9],
    ///     &[1, 2, 4, 3],
    ///     &[0, 2, 3],
    /// ];
    /// let faces = Faces::<usize>::from_iter(slice);
    /// let mut iter = faces.triangle_iter();
    /// assert_eq!(iter.len(), 10);
    /// assert_eq!(iter.next(), Some([0, 1, 2]));
    /// assert_eq!(iter.next(), Some([0, 2, 3]));
    /// assert_eq!(iter.next(), Some([0, 4, 5]));
    /// assert_eq!(iter.next(), Some([0, 5, 1]));
    /// assert_eq!(iter.next(), Some([1, 2, 4]));
    /// assert_eq!(iter.next(), Some([1, 4, 3]));
    /// assert_eq!(iter.next(), Some([1, 2, 6]));
    /// assert_eq!(iter.next(), Some([1, 6, 7]));
    /// assert_eq!(iter.next(), Some([1, 7, 8]));
    /// assert_eq!(iter.next(), Some([1, 8, 9]));
    /// assert_eq!(iter.len(), 0);
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline(always)]
    pub fn triangle_iter(&self) -> TriangleIterator<'_, V> {
        let len = self.face_iter().fold(0, |sum, face| sum + face.len() - 2);
        TriangleIterator {
            tri_faces: self.tri_faces.iter(),
            quad_faces: self.quad_faces.iter(),
            other_faces: self.other_faces.iter(),
            current_face: None,
            current_vertex: 0,
            len,
        }
    }
}

impl<V> Default for Faces<V> {
    fn default() -> Self {
        Self {
            tri_faces: Vec::new(),
            quad_faces: Vec::new(),
            other_faces: Vec::new(),
        }
    }
}

impl<V> std::ops::Index<usize> for Faces<V> {
    type Output = [V];
    fn index(&self, idx: usize) -> &Self::Output {
        if idx < self.tri_faces.len() {
            &self.tri_faces[idx]
        } else if idx < self.tri_faces.len() + self.quad_faces.len() {
            &self.quad_faces[idx - self.tri_faces.len()]
        } else {
            &self.other_faces[idx - self.tri_faces.len() - self.quad_faces.len()]
        }
    }
}

impl<V: Copy> Invertible for Faces<V> {
    #[inline(always)]
    fn invert(&mut self) { self.face_iter_mut().for_each(|f| f.reverse()); }
    #[inline(always)]
    fn inverse(&self) -> Self {
        let tri_faces: Vec<_> = self
            .tri_faces
            .iter()
            .map(|face| [face[2], face[1], face[0]])
            .collect();
        let quad_faces: Vec<_> = self
            .quad_faces
            .iter()
            .map(|face| [face[3], face[2], face[1], face[0]])
            .collect();
        let other_faces: Vec<_> = self
            .other_faces
            .iter()
            .map(|face| face.iter().rev().map(Clone::clone).collect())
            .collect();
        Faces {
            tri_faces,
            quad_faces,
            other_faces,
        }
    }
}

impl std::ops::IndexMut<usize> for Faces {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        if idx < self.tri_faces.len() {
            &mut self.tri_faces[idx]
        } else if idx < self.tri_faces.len() + self.quad_faces.len() {
            &mut self.quad_faces[idx - self.tri_faces.len()]
        } else {
            &mut self.other_faces[idx - self.tri_faces.len() - self.quad_faces.len()]
        }
    }
}

/// iterator run on faces as the set of triangle.
#[derive(Clone, Debug)]
pub struct TriangleIterator<'a, V> {
    tri_faces: std::slice::Iter<'a, [V; 3]>,
    quad_faces: std::slice::Iter<'a, [V; 4]>,
    other_faces: std::slice::Iter<'a, Vec<V>>,
    current_face: Option<&'a [V]>,
    current_vertex: usize,
    len: usize,
}

impl<V: Copy> Iterator for TriangleIterator<'_, V> {
    type Item = [V; 3];
    fn next(&mut self) -> Option<[V; 3]> {
        let TriangleIterator {
            tri_faces,
            quad_faces,
            other_faces,
            current_face,
            current_vertex,
            len,
        } = self;
        if *len == 0 {
            return None;
        }
        *len -= 1;
        if let Some(face) = tri_faces.next() {
            Some(*face)
        } else {
            if current_face.is_none() {
                *current_face = quad_faces
                    .next()
                    .map(AsRef::as_ref)
                    .or_else(|| other_faces.next().map(AsRef::as_ref));
            }
            let face = current_face.unwrap();
            let res = [
                face[0],
                face[*current_vertex + 1],
                face[*current_vertex + 2],
            ];
            *current_vertex += 1;
            if current_face.unwrap().len() == *current_vertex + 2 {
                *current_face = None;
                *current_vertex = 0;
            }
            Some(res)
        }
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) { (self.len, Some(self.len)) }
}

impl<V: Copy> ExactSizeIterator for TriangleIterator<'_, V> {}
