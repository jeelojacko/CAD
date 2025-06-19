use slint::{SharedString, VecModel};
use std::rc::Rc;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let dlg = LineStyleManager::new()?;
    let styles = vec![SharedString::from("Example A"), SharedString::from("Example B")];
    dlg.set_styles_model(Rc::new(VecModel::from(styles)).into());
    dlg.run()
}
