slint::slint! {
import { Button, VerticalBox, HorizontalBox, LineEdit } from "std-widgets.slint";
export component DistanceApp inherits Window {
    width: 300px;
    height: 200px;
    in-out property <string> x1;
    in-out property <string> y1;
    in-out property <string> x2;
    in-out property <string> y2;
    in-out property <string> result;
    callback compute();

    VerticalBox {
        HorizontalBox {
            Text { text: "Point A:"; }
            LineEdit { text <=> root.x1; placeholder-text: "x"; }
            LineEdit { text <=> root.y1; placeholder-text: "y"; }
        }
        HorizontalBox {
            Text { text: "Point B:"; }
            LineEdit { text <=> root.x2; placeholder-text: "x"; }
            LineEdit { text <=> root.y2; placeholder-text: "y"; }
        }
        Button { text: "Compute Distance"; clicked => { root.compute(); } }
        Text { text: root.result; }
    }
}
}

fn main() -> Result<(), slint::PlatformError> {
    let app = DistanceApp::new()?;
    let weak = app.as_weak();
    app.on_compute(move || {
        let app = weak.unwrap();
        let x1: f64 = app.get_x1().parse().unwrap_or(0.0);
        let y1: f64 = app.get_y1().parse().unwrap_or(0.0);
        let x2: f64 = app.get_x2().parse().unwrap_or(0.0);
        let y2: f64 = app.get_y2().parse().unwrap_or(0.0);
        let p1 = survey_cad::geometry::Point::new(x1, y1);
        let p2 = survey_cad::geometry::Point::new(x2, y2);
        let dist = survey_cad::geometry::distance(p1, p2);
        app.set_result(format!("Distance: {:.2}", dist).into());
    });
    app.run()
}
