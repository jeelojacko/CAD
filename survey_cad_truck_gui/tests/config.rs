//! Configuration tests for `survey_cad_truck_gui`.
//!
//! These rely on a fully working GUI environment with Python bindings which
//! may not be available in all CI setups.  To avoid build failures, the test
//! module is only compiled when the `gui-tests` feature is explicitly enabled.
#![cfg(feature = "gui-tests")]

use survey_cad_truck_gui::ui_state::{Config, SnapPrefs, WorkspaceProfile, Theme, load_config, save_config};
use tempfile::tempdir;
use std::env;

#[test]
fn save_and_load_config() {
    let dir = tempdir().unwrap();
    // Redirect configuration directory to the temporary path
    env::set_var("XDG_CONFIG_HOME", dir.path());
    env::set_var("APPDATA", dir.path());
    env::set_var("HOME", dir.path());

    let cfg = Config {
        window_width: 1024,
        window_height: 768,
        last_open_dir: Some("/tmp".into()),
        snap: SnapPrefs::default(),
        auto_tin: true,
        quick_scripts: vec!["one".into(), "two".into()],
        profile: WorkspaceProfile::default(),
        theme: Theme::default(),
        font_path: Some("font/path".into()),
        macro_dir: Some("/tmp/macros".into()),
        lod_distance: Some(100.0),
        crs_epsg: 4326,
    };

    save_config(&cfg);
    let loaded = load_config();
    assert_eq!(cfg, loaded);
}
