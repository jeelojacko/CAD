use truck_modeling::*;
use truck_stepio::out::*;

macro_rules! dir ( () => { concat!(env!("CARGO_MANIFEST_DIR"), "/../resources/shape/") });

const SOLID_JSONS: &[&str] = &[
    concat!(dir!(), "bottle.json"),
    concat!(dir!(), "punched-cube.json"),
    concat!(dir!(), "torus-punched-cube.json"),
    concat!(dir!(), "cube-in-cube.json"),
];

#[test]
fn parse_solid() {
    for json_file in SOLID_JSONS.iter() {
        let json = std::fs::read(json_file).unwrap();
        let solid: CompressedSolid = serde_json::from_reader(json.as_slice()).unwrap();
        let step_string =
            CompleteStepDisplay::new(StepModel::from(&solid), Default::default()).to_string();
        ruststep::parser::parse(&step_string).unwrap_or_else(|e| {
            panic!(
                "failed to parse step from {json_file}\n[Error Message]\n{e}[STEP file]\n{step_string}"
            )
        });
    }
}

#[test]
fn parse_shell() {
    for json_file in SOLID_JSONS.iter() {
        let json = std::fs::read(json_file).unwrap();
        let mut solid: CompressedSolid = serde_json::from_reader(json.as_slice()).unwrap();
        let shell = solid.boundaries.pop().unwrap();
        let step_string =
            CompleteStepDisplay::new(StepModel::from(&shell), Default::default()).to_string();
        ruststep::parser::parse(&step_string).unwrap_or_else(|e| {
            panic!(
                "failed to parse step from {json_file}\n[Error Message]\n{e}[STEP file]\n{step_string}"
            )
        });
    }
}

#[test]
fn parse_solids() {
    let solids: Vec<CompressedSolid> = SOLID_JSONS
        .iter()
        .map(|json_file| {
            let json = std::fs::read(json_file).unwrap();
            serde_json::from_reader(json.as_slice()).unwrap()
        })
        .collect();
    let step_string =
        CompleteStepDisplay::new(StepModels::from_iter(&solids), Default::default()).to_string();
    ruststep::parser::parse(&step_string).unwrap_or_else(|e| {
        panic!("failed to parse step\n[Error Message]\n{e}[STEP file]\n{step_string}")
    });
}
