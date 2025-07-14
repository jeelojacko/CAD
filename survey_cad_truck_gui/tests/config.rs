use survey_cad_truck_gui::ui_state::{load_config, save_config, Config, Theme};
use tempfile::tempdir;
use std::env;

fn set_temp_config_dir(dir: &std::path::Path) {
    env::set_var("XDG_CONFIG_HOME", dir);
}

#[test]
fn load_default_when_missing() {
    let dir = tempdir().unwrap();
    set_temp_config_dir(dir.path());
    let cfg = load_config();
    assert_eq!(cfg.window_width, 800);
    assert_eq!(cfg.window_height, 600);
    assert!(cfg.last_open_dir.is_none());
    assert!(!cfg.auto_tin);
}

#[test]
fn config_round_trip() {
    let dir = tempdir().unwrap();
    set_temp_config_dir(dir.path());
    let cfg = Config {
        window_width: 1024,
        window_height: 768,
        last_open_dir: Some("dir".into()),
        snap: Default::default(),
        auto_tin: true,
        quick_scripts: vec!["q".into()],
        profile: Default::default(),
        theme: Theme::Light,
        font_path: Some("font".into()),
    };
    save_config(&cfg);
    let loaded = load_config();
    assert_eq!(loaded.window_width, 1024);
    assert_eq!(loaded.window_height, 768);
    assert_eq!(loaded.last_open_dir.as_deref(), Some("dir"));
    assert!(loaded.auto_tin);
    assert_eq!(loaded.quick_scripts, vec!["q".to_string()]);
    assert_eq!(loaded.profile as u8, cfg.profile as u8);
    assert_eq!(loaded.theme as u8, cfg.theme as u8);
    assert_eq!(loaded.font_path.as_deref(), Some("font"));
}
