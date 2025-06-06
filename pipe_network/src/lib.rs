use std::collections::HashMap;
use std::io::{self, Write};

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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Network {
    pub structures: Vec<Structure>,
    pub pipes: Vec<Pipe>,
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
        if line.trim().is_empty() { continue; }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 4 { continue; }
        network.structures.push(Structure {
            id: parts[0].trim().to_string(),
            x: parts[1].trim().parse().unwrap_or(0.0),
            y: parts[2].trim().parse().unwrap_or(0.0),
            z: parts[3].trim().parse().unwrap_or(0.0),
        });
    }
    for line in p_lines.lines() {
        if line.trim().is_empty() { continue; }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 5 { continue; }
        network.pipes.push(Pipe {
            id: parts[0].trim().to_string(),
            from: parts[1].trim().to_string(),
            to: parts[2].trim().to_string(),
            diameter: parts[3].trim().parse().unwrap_or(0.0),
            c: parts[4].trim().parse().unwrap_or(100.0),
        });
    }
    Ok(network)
}

pub fn write_network_csv(net: &Network, structs: &str, pipes: &str) -> io::Result<()> {
    let mut s_file = std::fs::File::create(structs)?;
    for s in &net.structures {
        writeln!(s_file, "{},{},{},{}", s.id, s.x, s.y, s.z)?;
    }
    let mut p_file = std::fs::File::create(pipes)?;
    for p in &net.pipes {
        writeln!(p_file, "{},{},{},{},{}", p.id, p.from, p.to, p.diameter, p.c)?;
    }
    Ok(())
}

pub fn write_network_landxml(path: &str, net: &Network) -> io::Result<()> {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\"?>\n<LandXML>\n  <PipeNetworks>\n");
    xml.push_str("    <Structs>\n");
    for s in &net.structures {
        xml.push_str(&format!("      <Struct id=\"{}\" x=\"{}\" y=\"{}\" z=\"{}\"/>\n", s.id, s.x, s.y, s.z));
    }
    xml.push_str("    </Structs>\n    <Pipes>\n");
    for p in &net.pipes {
        xml.push_str(&format!("      <Pipe id=\"{}\" from=\"{}\" to=\"{}\" diameter=\"{}\" c=\"{}\"/>\n", p.id, p.from, p.to, p.diameter, p.c));
    }
    xml.push_str("    </Pipes>\n  </PipeNetworks>\n</LandXML>\n");
    std::fs::write(path, xml)
}

/// Calculates head loss using the Hazen-Williams equation (SI units).
pub fn hazen_williams_headloss(flow: f64, length: f64, diameter: f64, c: f64) -> f64 {
    if diameter <= 0.0 || c <= 0.0 { return 0.0; }
    10.67 * length * flow.powf(1.852) / (c.powf(1.852) * diameter.powf(4.8704))
}

/// Computes hydraulic grade line drop along a pipe.
pub fn hydraulic_grade(start_elev: f64, headloss: f64) -> f64 {
    start_elev - headloss
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headloss_calc() {
        let h = hazen_williams_headloss(0.1, 100.0, 0.3, 120.0);
        assert!(h > 0.0);
    }
}
