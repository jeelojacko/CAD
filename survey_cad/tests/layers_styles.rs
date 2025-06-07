use survey_cad::{
    layers::{Layer, LayerManager},
    styles::{DimensionStyle, DimensionStyleOverride, TextStyle},
};

#[test]
fn layer_manager_filter() {
    let mut mgr = LayerManager::new();
    mgr.add_layer(Layer::new("A"));
    let mut off = Layer::new("B");
    off.is_on = false;
    mgr.add_layer(off);

    let on_layers = mgr.filter(|l| l.is_on);
    assert_eq!(on_layers.len(), 1);
    assert_eq!(on_layers[0].name, "A");

    mgr.set_layer_state("B", true);
    let on_layers = mgr.filter(|l| l.is_on);
    assert_eq!(on_layers.len(), 2);
}

#[test]
fn dimension_style_override() {
    let base = DimensionStyle::new("base", TextStyle::new("txt", "Arial", 2.5), 1.0);
    let over = DimensionStyleOverride {
        text_style: None,
        scale: Some(2.0),
    };
    let result = base.overridden(&over);
    assert_eq!(result.scale, 2.0);
    assert_eq!(result.text_style.name, "txt");
}
