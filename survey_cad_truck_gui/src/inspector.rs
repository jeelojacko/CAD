use std::cell::RefCell;
use std::rc::Rc;

use crate::error::GuiError;

use slint::{Image, SharedString, VecModel};

use survey_cad::geometry::Point;

use crate::truck_backend::TruckBackend;
use crate::workspace::refresh_workspace;
use crate::{ContextMenu, EntityInspector, MainWindow};

pub fn show_context_menu(
    app: &MainWindow,
    state: &Rc<RefCell<Option<slint::Weak<ContextMenu>>>>,
    x: f32,
    y: f32,
) {
    if let Some(m) = state.borrow_mut().take().and_then(|w| w.upgrade()) {
        let _ = m.hide();
    }
    let menu = ContextMenu::new().unwrap();
    menu.set_pos_x(x);
    menu.set_pos_y(y);
    {
        let weak = app.as_weak();
        menu.on_mov(move || {
            if let Some(a) = weak.upgrade() {
                a.invoke_move_entity();
            }
        });
    }
    {
        let weak = app.as_weak();
        menu.on_rot(move || {
            if let Some(a) = weak.upgrade() {
                a.invoke_rotate_entity();
            }
        });
    }
    {
        let weak = app.as_weak();
        menu.on_properties(move || {
            if let Some(a) = weak.upgrade() {
                a.invoke_inspector();
            }
        });
    }
    {
        let weak = app.as_weak();
        menu.on_delete(move || {
            if let Some(a) = weak.upgrade() {
                a.invoke_delete_selected();
            }
        });
    }
    menu.show().unwrap();
    *state.borrow_mut() = Some(menu.as_weak());
}

pub fn has_selection(
    pts: &Rc<RefCell<Vec<usize>>>,
    lines: &Rc<RefCell<Vec<(Point, Point)>>>,
    polys: &Rc<RefCell<Vec<usize>>>,
    plines: &Rc<RefCell<Vec<usize>>>,
    arcs: &Rc<RefCell<Vec<usize>>>,
    dims: &Rc<RefCell<Vec<usize>>>,
) -> bool {
    !pts.borrow().is_empty()
        || !lines.borrow().is_empty()
        || !polys.borrow().is_empty()
        || !plines.borrow().is_empty()
        || !arcs.borrow().is_empty()
        || !dims.borrow().is_empty()
}

#[allow(clippy::too_many_arguments)]
pub fn show_inspector_for_point(
    idx: usize,
    app: &MainWindow,
    layer_names: &Rc<RefCell<Vec<String>>>,
    style_names: &[SharedString],
    layers: &Rc<RefCell<Vec<usize>>>,
    styles: &Rc<RefCell<Vec<usize>>>,
    metadata: &Rc<RefCell<Vec<String>>>,
    elevation: &Rc<RefCell<Vec<String>>>,
    measurement: &Rc<RefCell<Vec<String>>>,
    data_sets: &Rc<RefCell<Vec<usize>>>,
    data_set_names: &Rc<RefCell<Vec<String>>>,
    render_image: Rc<dyn Fn() -> Result<Image, GuiError>>,
    backend: &Rc<RefCell<TruckBackend>>,
) {
    while layers.borrow().len() <= idx {
        layers.borrow_mut().push(0);
    }
    while styles.borrow().len() <= idx {
        styles.borrow_mut().push(0);
    }
    while metadata.borrow().len() <= idx {
        metadata.borrow_mut().push(String::new());
    }
    while elevation.borrow().len() <= idx {
        elevation.borrow_mut().push(String::new());
    }
    while measurement.borrow().len() <= idx {
        measurement.borrow_mut().push(String::new());
    }
    while data_sets.borrow().len() <= idx {
        data_sets.borrow_mut().push(0);
    }

    let layer_model = Rc::new(VecModel::from(
        layer_names
            .borrow()
            .iter()
            .map(|s| SharedString::from(s.as_str()))
            .collect::<Vec<_>>(),
    ));
    let style_model = Rc::new(VecModel::from(style_names.to_vec()));
    let data_set_model = Rc::new(VecModel::from(
        data_set_names
            .borrow()
            .iter()
            .map(|s| SharedString::from(s.as_str()))
            .collect::<Vec<_>>(),
    ));

    app.set_inspector_layers_model(layer_model.into());
    app.set_inspector_styles_model(style_model.into());
    app.set_inspector_hatch_model(Rc::new(VecModel::from(Vec::<SharedString>::new())).into());
    app.set_inspector_data_set_model(data_set_model.into());
    app.set_inspector_entity_type(SharedString::from("Point"));
    app.set_inspector_layer_index(layers.borrow()[idx] as i32);
    app.set_inspector_style_index(styles.borrow()[idx] as i32);
    app.set_inspector_metadata(SharedString::from(metadata.borrow()[idx].as_str()));
    app.set_inspector_elevation(SharedString::from(elevation.borrow()[idx].as_str()));
    app.set_inspector_measurement(SharedString::from(measurement.borrow()[idx].as_str()));
    app.set_inspector_data_set_index(data_sets.borrow()[idx] as i32);

    {
        let layers = layers.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        app.on_inspector_layer_index_changed(move |val| {
            if let Some(l) = layers.borrow_mut().get_mut(idx) {
                *l = val as usize;
            }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let styles_ref = styles.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        app.on_inspector_style_index_changed(move |val| {
            if let Some(s) = styles_ref.borrow_mut().get_mut(idx) {
                *s = val as usize;
            }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let meta_ref = metadata.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        app.on_inspector_metadata_changed(move |text| {
            if let Some(m) = meta_ref.borrow_mut().get_mut(idx) {
                *m = text.to_string();
            }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let elev_ref = elevation.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        app.on_inspector_elevation_changed(move |text| {
            if let Some(e) = elev_ref.borrow_mut().get_mut(idx) {
                *e = text.to_string();
            }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let meas_ref = measurement.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        app.on_inspector_measurement_changed(move |text| {
            if let Some(m) = meas_ref.borrow_mut().get_mut(idx) {
                *m = text.to_string();
            }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let ds_ref = data_sets.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        app.on_inspector_data_set_index_changed(move |val| {
            if let Some(d) = ds_ref.borrow_mut().get_mut(idx) {
                *d = val as usize;
            }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    app.set_inspector_visible(true);
}

#[allow(clippy::too_many_arguments)]
pub fn show_inspector_for_polygon(
    idx: usize,
    app: &MainWindow,
    layer_names: &Rc<RefCell<Vec<String>>>,
    hatch_names: &[SharedString],
    layers: &Rc<RefCell<Vec<usize>>>,
    hatches: &Rc<RefCell<Vec<usize>>>,
    measurement: &Rc<RefCell<Vec<String>>>,
    data_sets: &Rc<RefCell<Vec<usize>>>,
    data_set_names: &Rc<RefCell<Vec<String>>>,
    render_image: Rc<dyn Fn() -> Result<Image, GuiError>>,
    backend: &Rc<RefCell<TruckBackend>>,
) {
    while layers.borrow().len() <= idx {
        layers.borrow_mut().push(0);
    }
    while hatches.borrow().len() <= idx {
        hatches.borrow_mut().push(0);
    }
    while measurement.borrow().len() <= idx {
        measurement.borrow_mut().push(String::new());
    }
    while data_sets.borrow().len() <= idx {
        data_sets.borrow_mut().push(0);
    }

    let layer_model = Rc::new(VecModel::from(
        layer_names
            .borrow()
            .iter()
            .map(|s| SharedString::from(s.as_str()))
            .collect::<Vec<_>>(),
    ));
    let hatch_model = Rc::new(VecModel::from(hatch_names.to_vec()));
    let data_set_model = Rc::new(VecModel::from(
        data_set_names
            .borrow()
            .iter()
            .map(|s| SharedString::from(s.as_str()))
            .collect::<Vec<_>>(),
    ));

    app.set_inspector_layers_model(layer_model.into());
    app.set_inspector_styles_model(Rc::new(VecModel::from(Vec::<SharedString>::new())).into());
    app.set_inspector_hatch_model(hatch_model.into());
    app.set_inspector_data_set_model(data_set_model.into());
    app.set_inspector_entity_type(SharedString::from("Polygon"));
    app.set_inspector_layer_index(layers.borrow()[idx] as i32);
    app.set_inspector_hatch_index(hatches.borrow()[idx] as i32);
    app.set_inspector_metadata(SharedString::from(""));
    app.set_inspector_measurement(SharedString::from(measurement.borrow()[idx].as_str()));
    app.set_inspector_data_set_index(data_sets.borrow()[idx] as i32);

    {
        let layers = layers.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        dlg.on_layer_changed(move |val| {
            if let Some(l) = layers.borrow_mut().get_mut(idx) {
                *l = val as usize;
            }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let h_ref = hatches.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        app.on_inspector_hatch_index_changed(move |val| {
            if let Some(h) = h_ref.borrow_mut().get_mut(idx) {
                *h = val as usize;
            }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let meas_ref = measurement.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        app.on_inspector_measurement_changed(move |text| {
            if let Some(m) = meas_ref.borrow_mut().get_mut(idx) {
                *m = text.to_string();
            }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let ds_ref = data_sets.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        app.on_inspector_data_set_index_changed(move |val| {
            if let Some(d) = ds_ref.borrow_mut().get_mut(idx) {
                *d = val as usize;
            }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    app.set_inspector_visible(true);
}
