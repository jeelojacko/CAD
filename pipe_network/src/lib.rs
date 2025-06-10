use std::collections::HashMap;
use std::io::{self, Write};

use roxmltree::Document;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Structure {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipe {
    pub id: String,
    pub from: String,
    pub to: String,
    pub diameter: f64,
    pub c: f64,
    /// Invert elevation at the pipe start
    pub start_invert: f64,
    /// Invert elevation at the pipe end
    pub end_invert: f64,
    /// Design flow for the pipe (m^3/s)
    #[serde(default)]
    pub design_flow: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Network {
    pub structures: Vec<Structure>,
    pub pipes: Vec<Pipe>,
}

/// Rule specifying minimum slope based on pipe diameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlopeRule {
    /// Minimum pipe diameter the rule applies to (m)
    pub min_diameter: f64,
    /// Desired slope for pipes of at least this diameter
    pub slope: f64,
}

fn parse_num<T: std::str::FromStr>(s: &str) -> io::Result<T>
where
    T::Err: std::fmt::Display,
{
    s.trim()
        .parse::<T>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

impl Network {
    pub fn structure_index(&self) -> HashMap<&str, usize> {
        let mut map = HashMap::new();
        for (i, s) in self.structures.iter().enumerate() {
            map.insert(s.id.as_str(), i);
        }
        map
    }
}

pub fn read_network_csv(structs: &str, pipes: &str) -> io::Result<Network> {
    let s_lines = std::fs::read_to_string(structs)?;
    let p_lines = std::fs::read_to_string(pipes)?;
    let mut network = Network::default();
    for line in s_lines.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 4 {
            continue;
        }
        network.structures.push(Structure {
            id: parts[0].trim().to_string(),
            x: parse_num(parts[1])?,
            y: parse_num(parts[2])?,
            z: parse_num(parts[3])?,
        });
    }
    for line in p_lines.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 5 {
            continue;
        }
        network.pipes.push(Pipe {
            id: parts[0].trim().to_string(),
            from: parts[1].trim().to_string(),
            to: parts[2].trim().to_string(),
            diameter: parse_num(parts[3])?,
            c: parse_num(parts[4])?,
            start_invert: match parts.get(5) {
                Some(v) => parse_num(v)?,
                None => 0.0,
            },
            end_invert: match parts.get(6) {
                Some(v) => parse_num(v)?,
                None => 0.0,
            },
            design_flow: match parts.get(7) {
                Some(v) => parse_num(v)?,
                None => 0.0,
            },
        });
    }
    Ok(network)
}

/// Read slope design rules from a CSV file with `diameter,slope` lines.
pub fn read_slope_rules_csv(path: &str) -> io::Result<Vec<SlopeRule>> {
    let mut rules = Vec::new();
    for line in std::fs::read_to_string(path)?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        let diam: f64 = parse_num(parts[0])?;
        let slope: f64 = parse_num(parts[1])?;
        rules.push(SlopeRule {
            min_diameter: diam,
            slope,
        });
    }
    // sort ascending by min_diameter for easier lookup
    rules.sort_by(|a, b| a.min_diameter.partial_cmp(&b.min_diameter).unwrap());
    Ok(rules)
}

pub fn write_network_csv(net: &Network, structs: &str, pipes: &str) -> io::Result<()> {
    let mut s_file = std::fs::File::create(structs)?;
    for s in &net.structures {
        writeln!(s_file, "{},{},{},{}", s.id, s.x, s.y, s.z)?;
    }
    let mut p_file = std::fs::File::create(pipes)?;
    for p in &net.pipes {
        writeln!(
            p_file,
            "{},{},{},{},{},{},{},{}",
            p.id,
            p.from,
            p.to,
            p.diameter,
            p.c,
            p.start_invert,
            p.end_invert,
            p.design_flow
        )?;
    }
    Ok(())
}

pub fn write_network_landxml(path: &str, net: &Network) -> io::Result<()> {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\"?>\n<LandXML>\n  <PipeNetworks>\n");
    xml.push_str("    <Structs>\n");
    for s in &net.structures {
        xml.push_str(&format!(
            "      <Struct id=\"{}\" x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
            s.id, s.x, s.y, s.z
        ));
    }
    xml.push_str("    </Structs>\n    <Pipes>\n");
    for p in &net.pipes {
        xml.push_str(&format!(
            "      <Pipe id=\"{}\" from=\"{}\" to=\"{}\" diameter=\"{}\" c=\"{}\" startInv=\"{}\" endInv=\"{}\" designFlow=\"{}\"/>\n",
            p.id,
            p.from,
            p.to,
            p.diameter,
            p.c,
            p.start_invert,
            p.end_invert,
            p.design_flow
        ));
    }
    xml.push_str("    </Pipes>\n  </PipeNetworks>\n</LandXML>\n");
    std::fs::write(path, xml)
}

pub fn read_network_landxml(path: &str) -> io::Result<Network> {
    let xml = std::fs::read_to_string(path)?;
    let doc = Document::parse(&xml).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut network = Network::default();
    if let Some(structs) = doc.descendants().find(|n| n.has_tag_name("Structs")) {
        for s in structs.children().filter(|c| c.has_tag_name("Struct")) {
            network.structures.push(Structure {
                id: s.attribute("id").unwrap_or("").to_string(),
                x: match s.attribute("x") {
                    Some(v) => parse_num(v)?,
                    None => 0.0,
                },
                y: match s.attribute("y") {
                    Some(v) => parse_num(v)?,
                    None => 0.0,
                },
                z: match s.attribute("z") {
                    Some(v) => parse_num(v)?,
                    None => 0.0,
                },
            });
        }
    }
    if let Some(pipes) = doc.descendants().find(|n| n.has_tag_name("Pipes")) {
        for p in pipes.children().filter(|c| c.has_tag_name("Pipe")) {
            network.pipes.push(Pipe {
                id: p.attribute("id").unwrap_or("").to_string(),
                from: p.attribute("from").unwrap_or("").to_string(),
                to: p.attribute("to").unwrap_or("").to_string(),
                diameter: match p.attribute("diameter") {
                    Some(v) => parse_num(v)?,
                    None => 0.0,
                },
                c: match p.attribute("c") {
                    Some(v) => parse_num(v)?,
                    None => 100.0,
                },
                start_invert: match p.attribute("startInv") {
                    Some(v) => parse_num(v)?,
                    None => 0.0,
                },
                end_invert: match p.attribute("endInv") {
                    Some(v) => parse_num(v)?,
                    None => 0.0,
                },
                design_flow: match p.attribute("designFlow") {
                    Some(v) => parse_num(v)?,
                    None => 0.0,
                },
            });
        }
    }
    Ok(network)
}

/// Apply slope design rules to compute pipe inverts.
pub fn apply_slope_rules(net: &mut Network, rules: &[SlopeRule]) {
    let idx: HashMap<String, usize> = net
        .structures
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.clone(), i))
        .collect();
    for pipe in &mut net.pipes {
        if let Some(&start_idx) = idx.get(pipe.from.as_str()) {
            let a = &net.structures[start_idx];
            // compute pipe length if we know the end structure
            let length = idx
                .get(pipe.to.as_str())
                .map(|&b_idx| {
                    let b = &net.structures[b_idx];
                    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
                })
                .unwrap_or(0.0);
            pipe.start_invert = a.z;
            let slope = rules
                .iter()
                .filter(|r| pipe.diameter >= r.min_diameter)
                .max_by(|a, b| a.min_diameter.partial_cmp(&b.min_diameter).unwrap())
                .map(|r| r.slope)
                .unwrap_or(0.0);
            pipe.end_invert = pipe.start_invert - slope * length;
        }
    }
}

/// Calculates head loss using the Hazen-Williams equation (SI units).
pub fn hazen_williams_headloss(flow: f64, length: f64, diameter: f64, c: f64) -> f64 {
    if diameter <= 0.0 || c <= 0.0 {
        return 0.0;
    }
    10.67 * length * flow.powf(1.852) / (c.powf(1.852) * diameter.powf(4.8704))
}

/// Computes hydraulic grade line drop along a pipe.
pub fn hydraulic_grade(start_elev: f64, headloss: f64) -> f64 {
    start_elev - headloss
}

/// Computes pipe slope using start and end invert elevations and pipe length.
pub fn pipe_slope(start_invert: f64, end_invert: f64, length: f64) -> f64 {
    if length == 0.0 {
        return 0.0;
    }
    (start_invert - end_invert) / length
}

/// Calculates hydraulic grade at the end of a pipe given start invert and flow parameters.
pub fn hydraulic_grade_from_inverts(
    start_invert: f64,
    flow: f64,
    length: f64,
    diameter: f64,
    c: f64,
) -> f64 {
    let hl = hazen_williams_headloss(flow, length, diameter, c);
    hydraulic_grade(start_invert, hl)
}

/// Results of analyzing a single pipe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeAnalysis {
    pub id: String,
    pub length: f64,
    pub design_flow: f64,
    pub slope: f64,
    pub headloss: f64,
    pub start_grade: f64,
    pub end_grade: f64,
}

/// Results of detailed pipe analysis with additional hydraulics info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedPipeAnalysis {
    pub id: String,
    pub length: f64,
    pub design_flow: f64,
    pub slope: f64,
    pub headloss: f64,
    pub velocity: f64,
    pub friction_slope: f64,
    pub start_grade: f64,
    pub end_grade: f64,
}

/// Analyze each pipe in a network using its `design_flow`.
pub fn analyze_network(net: &Network) -> Vec<PipeAnalysis> {
    let idx = net.structure_index();
    let mut results = Vec::new();
    for pipe in &net.pipes {
        if let (Some(&a_idx), Some(&b_idx)) = (idx.get(pipe.from.as_str()), idx.get(pipe.to.as_str())) {
            let a = &net.structures[a_idx];
            let b = &net.structures[b_idx];
            let length = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            let slope = pipe_slope(pipe.start_invert, pipe.end_invert, length);
            let flow = pipe.design_flow;
            let headloss = hazen_williams_headloss(flow, length, pipe.diameter, pipe.c);
            let start_grade = a.z;
            let end_grade = hydraulic_grade(start_grade, headloss);
            results.push(PipeAnalysis {
                id: pipe.id.clone(),
                length,
                design_flow: flow,
                slope,
                headloss,
                start_grade,
                end_grade,
            });
        }
    }
    results
}

/// Perform detailed analysis including velocity and friction slope
pub fn analyze_network_detailed(net: &Network) -> Vec<DetailedPipeAnalysis> {
    let idx = net.structure_index();
    let mut results = Vec::new();
    for pipe in &net.pipes {
        if let (Some(&a_idx), Some(&b_idx)) = (idx.get(pipe.from.as_str()), idx.get(pipe.to.as_str())) {
            let a = &net.structures[a_idx];
            let b = &net.structures[b_idx];
            let length = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            let slope = pipe_slope(pipe.start_invert, pipe.end_invert, length);
            let flow = pipe.design_flow;
            let headloss = hazen_williams_headloss(flow, length, pipe.diameter, pipe.c);
            let area = std::f64::consts::PI * (pipe.diameter * pipe.diameter) / 4.0;
            let velocity = if area > 0.0 { flow / area } else { 0.0 };
            let friction_slope = if length > 0.0 { headloss / length } else { 0.0 };
            let start_grade = a.z;
            let end_grade = hydraulic_grade(start_grade, headloss);
            results.push(DetailedPipeAnalysis {
                id: pipe.id.clone(),
                length,
                design_flow: flow,
                slope,
                headloss,
                velocity,
                friction_slope,
                start_grade,
                end_grade,
            });
        }
    }
    results
}

/// Write pipe analysis results to CSV.
pub fn write_analysis_csv(path: &str, results: &[PipeAnalysis]) -> io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    for r in results {
        writeln!(
            file,
            "{},{},{},{},{},{},{}",
            r.id, r.length, r.design_flow, r.slope, r.headloss, r.start_grade, r.end_grade
        )?;
    }
    Ok(())
}

/// Write detailed pipe analysis results to CSV.
pub fn write_detailed_analysis_csv(path: &str, results: &[DetailedPipeAnalysis]) -> io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    for r in results {
        writeln!(
            file,
            "{},{},{},{},{},{},{},{},{}",
            r.id,
            r.length,
            r.design_flow,
            r.slope,
            r.headloss,
            r.velocity,
            r.friction_slope,
            r.start_grade,
            r.end_grade
        )?;
    }
    Ok(())
}

/// Write pipe analysis results to LandXML.
pub fn write_analysis_landxml(path: &str, results: &[PipeAnalysis]) -> io::Result<()> {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\"?>\n<LandXML>\n  <PipeResults>\n");
    for r in results {
        xml.push_str(&format!(
            "    <Pipe id=\"{}\" length=\"{}\" designFlow=\"{}\" slope=\"{}\" headloss=\"{}\" startGrade=\"{}\" endGrade=\"{}\"/>\n",
            r.id, r.length, r.design_flow, r.slope, r.headloss, r.start_grade, r.end_grade
        ));
    }
    xml.push_str("  </PipeResults>\n</LandXML>\n");
    std::fs::write(path, xml)
}

/// Write detailed pipe analysis results to LandXML.
pub fn write_detailed_analysis_landxml(path: &str, results: &[DetailedPipeAnalysis]) -> io::Result<()> {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\"?>\n<LandXML>\n  <PipeResults>\n");
    for r in results {
        xml.push_str(&format!(
            "    <Pipe id=\"{}\" length=\"{}\" designFlow=\"{}\" slope=\"{}\" headloss=\"{}\" velocity=\"{}\" frictionSlope=\"{}\" startGrade=\"{}\" endGrade=\"{}\"/>\n",
            r.id,
            r.length,
            r.design_flow,
            r.slope,
            r.headloss,
            r.velocity,
            r.friction_slope,
            r.start_grade,
            r.end_grade
        ));
    }
    xml.push_str("  </PipeResults>\n</LandXML>\n");
    std::fs::write(path, xml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn headloss_calc() {
        let h = hazen_williams_headloss(0.1, 100.0, 0.3, 120.0);
        assert!(h > 0.0);
    }

    #[test]
    fn landxml_round_trip() {
        let net = Network {
            structures: vec![
                Structure {
                    id: "S1".into(),
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Structure {
                    id: "S2".into(),
                    x: 1.0,
                    y: 1.0,
                    z: 0.5,
                },
            ],
            pipes: vec![Pipe {
                id: "P1".into(),
                from: "S1".into(),
                to: "S2".into(),
                diameter: 0.3,
                c: 120.0,
                start_invert: 1.0,
                end_invert: 0.5,
                design_flow: 0.1,
            }],
        };
        let file = NamedTempFile::new().unwrap();
        write_network_landxml(file.path().to_str().unwrap(), &net).unwrap();
        let read = read_network_landxml(file.path().to_str().unwrap()).unwrap();
        assert_eq!(read.structures.len(), net.structures.len());
        assert_eq!(read.pipes.len(), net.pipes.len());
        for (a, b) in read.structures.iter().zip(net.structures.iter()) {
            assert_eq!(a.id, b.id);
            assert!((a.x - b.x).abs() < 1e-6);
            assert!((a.y - b.y).abs() < 1e-6);
            assert!((a.z - b.z).abs() < 1e-6);
        }
        for (a, b) in read.pipes.iter().zip(net.pipes.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.from, b.from);
            assert_eq!(a.to, b.to);
            assert!((a.diameter - b.diameter).abs() < 1e-6);
            assert!((a.c - b.c).abs() < 1e-6);
            assert!((a.start_invert - b.start_invert).abs() < 1e-6);
            assert!((a.end_invert - b.end_invert).abs() < 1e-6);
            assert!((a.design_flow - b.design_flow).abs() < 1e-6);
        }
    }

    #[test]
    fn slope_and_grade_utils() {
        let slope = pipe_slope(1.0, 0.5, 10.0);
        assert!((slope - 0.05).abs() < 1e-6);
        let grade = hydraulic_grade_from_inverts(1.0, 0.1, 10.0, 0.3, 120.0);
        assert!(grade < 1.0 && grade > 0.0);
    }

    #[test]
    fn analyze_network_basic() {
        let net = Network {
            structures: vec![
                Structure { id: "A".into(), x: 0.0, y: 0.0, z: 1.0 },
                Structure { id: "B".into(), x: 10.0, y: 0.0, z: 1.0 },
            ],
            pipes: vec![Pipe {
                id: "P".into(),
                from: "A".into(),
                to: "B".into(),
                diameter: 0.3,
                c: 120.0,
                start_invert: 1.0,
                end_invert: 0.9,
                design_flow: 0.2,
            }],
        };
        let res = analyze_network(&net);
        assert_eq!(res.len(), 1);
        assert!(res[0].headloss > 0.0);
    }

    #[test]
    fn apply_rules_and_detailed_analysis() {
        let mut net = Network {
            structures: vec![
                Structure { id: "A".into(), x: 0.0, y: 0.0, z: 2.0 },
                Structure { id: "B".into(), x: 20.0, y: 0.0, z: 2.0 },
            ],
            pipes: vec![Pipe {
                id: "P".into(),
                from: "A".into(),
                to: "B".into(),
                diameter: 0.5,
                c: 120.0,
                start_invert: 0.0,
                end_invert: 0.0,
                design_flow: 0.4,
            }],
        };
        let rules = vec![SlopeRule { min_diameter: 0.0, slope: 0.01 }];
        apply_slope_rules(&mut net, &rules);
        assert!(net.pipes[0].end_invert < net.pipes[0].start_invert);
        let det = analyze_network_detailed(&net);
        assert_eq!(det.len(), 1);
        assert!(det[0].velocity > 0.0);
    }

    #[test]
    fn csv_parse_error() {
        let mut s = NamedTempFile::new().unwrap();
        writeln!(s, "S1,abc,0,0").unwrap();
        let mut p = NamedTempFile::new().unwrap();
        writeln!(p, "P1,S1,S1,0.3,100").unwrap();
        let res = read_network_csv(s.path().to_str().unwrap(), p.path().to_str().unwrap());
        assert!(res.is_err());
    }

    #[test]
    fn slope_rules_parse_error() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "foo,0.01").unwrap();
        let res = read_slope_rules_csv(f.path().to_str().unwrap());
        assert!(res.is_err());
    }

    #[test]
    fn landxml_parse_error() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            "<?xml version=\"1.0\"?><LandXML><PipeNetworks><Structs><Struct id=\"S1\" x=\"foo\" y=\"0\" z=\"0\"/></Structs></PipeNetworks></LandXML>"
        )
        .unwrap();
        let res = read_network_landxml(f.path().to_str().unwrap());
        assert!(res.is_err());
    }
}
