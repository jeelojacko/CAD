use thiserror::Error;

/// Topological Errors
#[derive(Debug, PartialEq, Eq, Error)]
pub enum Error {
    /// Two same vertices cannot construct an edge.
    /// # Examples
    /// ```
    /// use truck_topology::*;
    /// use truck_topology::errors::Error;
    /// let v = Vertex::new(());
    /// assert_eq!(Edge::try_new(&v, &v, ()), Err(Error::SameVertex));
    /// ```
    #[error("Two same vertices cannot construct an edge.")]
    SameVertex,
    /// The empty wire cannot construct a face.
    /// # Examples
    /// ```
    /// use truck_topology::*;
    /// use truck_topology::errors::Error;
    /// assert_eq!(Face::try_new(vec![Wire::<(), ()>::new()], ()), Err(Error::EmptyWire));
    /// ```
    #[error("This wire is empty.")]
    EmptyWire,
    /// The boundary of a face must be closed.
    /// # Examples
    /// ```
    /// use truck_topology::*;
    /// use truck_topology::errors::Error;
    /// let v = Vertex::news(&[(), ()]);
    /// let wire: Wire<(), ()> = vec![Edge::new(&v[0], &v[1], ())].into();
    /// assert_eq!(Face::try_new(vec![wire], ()), Err(Error::NotClosedWire));
    /// ```
    #[error("This wire is not closed.")]
    NotClosedWire,
    /// The boundary of a face must be simple.
    /// # Examples
    /// ```
    /// use truck_topology::*;
    /// use truck_topology::errors::Error;
    /// let v = Vertex::news(&[(); 4]);
    /// let wire: Wire<(), ()> = vec![
    ///     Edge::new(&v[0], &v[1], ()),
    ///     Edge::new(&v[1], &v[2], ()),
    ///     Edge::new(&v[2], &v[3], ()),
    ///     Edge::new(&v[3], &v[1], ()),
    ///     Edge::new(&v[1], &v[0], ()),
    /// ].into();
    /// assert_eq!(Face::try_new(vec![wire], ()), Err(Error::NotSimpleWire));
    /// ```
    #[error("This wire is not simple.")]
    NotSimpleWire,
    /// Some boundaries has a shared vertex.
    #[error("Some wires has a shared vertex.")]
    NotDisjointWires,
    /// The empty shell cannot construct a solid.
    /// # Examples
    /// ```
    /// use truck_topology::*;
    /// use truck_topology::errors::Error;
    /// assert_eq!(Solid::try_new(vec![Shell::<(), (), ()>::new()]), Err(Error::EmptyShell));
    /// ```
    #[error("This shell is empty.")]
    EmptyShell,
    /// The vector of boundaries of the solid must consist connected shells.
    /// # Examples
    /// ```
    /// use truck_topology::*;
    /// use truck_topology::errors::Error;
    /// let v = Vertex::news(&[(), (), (), ()]);
    /// let wire = vec![
    ///     Wire::from(vec![Edge::new(&v[0], &v[1], ()), Edge::new(&v[1], &v[0], ())]),
    ///     Wire::from(vec![Edge::new(&v[2], &v[3], ()), Edge::new(&v[3], &v[2], ())]),
    /// ];
    /// let shell: Shell<(), (), ()> = wire.into_iter().map(|w| Face::new(vec![w], ())).collect();
    /// assert_eq!(Solid::try_new(vec![shell]), Err(Error::NotConnected));
    /// ```
    #[error("This shell is not connected.")]
    NotConnected,
    /// The boundary of the solid must be closed.
    /// # Examples
    /// ```
    /// use truck_topology::*;
    /// use truck_topology::errors::Error;
    /// let v = Vertex::news(&[(), ()]);
    /// let wire = Wire::from(vec![Edge::new(&v[0], &v[1], ()), Edge::new(&v[1], &v[0], ())]);
    /// let shell: Shell<(), (), ()> = vec![Face::new(vec![wire], ())].into();
    /// assert_eq!(Solid::try_new(vec![shell]), Err(Error::NotClosedShell));
    /// ```
    #[error("This shell is not oriented and closed.")]
    NotClosedShell,
    /// The boundary of the solid must be a manifold.
    /// # Examples
    /// ```
    /// // the wedge sum of two spheres
    /// use truck_topology::*;
    /// use truck_topology::errors::Error;
    /// let v = Vertex::news(&[(), (), ()]);
    /// let edge = [
    ///     Edge::new(&v[0], &v[1], ()),
    ///     Edge::new(&v[1], &v[0], ()),
    ///     Edge::new(&v[1], &v[2], ()),
    ///     Edge::new(&v[2], &v[1], ()),
    /// ];
    /// let wire = vec![
    ///     wire![&edge[0], &edge[1]],
    ///     wire![&edge[1].inverse(), &edge[0].inverse()],
    ///     wire![&edge[2], &edge[3]],
    ///     wire![&edge[3].inverse(), &edge[2].inverse()],
    /// ];
    /// let shell: Shell<(), (), ()> = wire.into_iter().map(|w| Face::new(vec![w], ())).collect();
    /// assert_eq!(Solid::try_new(vec![shell]), Err(Error::NotManifold));
    /// ```
    #[error("This shell is not a manifold.")]
    NotManifold,
}

#[test]
fn print_messages() {
    use std::io::Write;
    writeln!(
        &mut std::io::stderr(),
        "****** test of the expressions of error messages ******\n"
    )
    .unwrap();
    writeln!(&mut std::io::stderr(), "{}\n", Error::SameVertex).unwrap();
    writeln!(&mut std::io::stderr(), "{}\n", Error::EmptyWire).unwrap();
    writeln!(&mut std::io::stderr(), "{}\n", Error::NotClosedWire).unwrap();
    writeln!(&mut std::io::stderr(), "{}\n", Error::NotSimpleWire).unwrap();
    writeln!(&mut std::io::stderr(), "{}\n", Error::NotDisjointWires).unwrap();
    writeln!(&mut std::io::stderr(), "{}\n", Error::EmptyShell).unwrap();
    writeln!(&mut std::io::stderr(), "{}\n", Error::NotConnected).unwrap();
    writeln!(&mut std::io::stderr(), "{}\n", Error::NotClosedShell).unwrap();
    writeln!(&mut std::io::stderr(), "{}\n", Error::NotManifold).unwrap();
    writeln!(
        &mut std::io::stderr(),
        "*******************************************************"
    )
    .unwrap();
}
