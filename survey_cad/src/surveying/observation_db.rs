use rusqlite::{Connection, params};
use serde::{Serialize, Deserialize};
use chrono::NaiveDate;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ObsType {
    TotalStation,
    Gnss,
    LevelRun,
    Traverse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraverseLeg {
    pub from: String,
    pub to: String,
    pub bearing: f64,
    pub distance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ObservationData {
    TotalStation {
        from: String,
        to: String,
        horiz_angle: f64,
        vert_angle: f64,
        slope_distance: f64,
    },
    Gnss {
        point: String,
        northing: f64,
        easting: f64,
        elevation: f64,
    },
    LevelRun {
        from: String,
        to: String,
        backsight: f64,
        foresight: f64,
    },
    Traverse {
        name: String,
        legs: Vec<TraverseLeg>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservationRecord {
    pub id: Option<i64>,
    pub obs_type: ObsType,
    pub date: NaiveDate,
    pub instrument: Option<String>,
    pub crew: Option<String>,
    pub control_point: Option<String>,
    pub data: ObservationData,
}

#[derive(Default)]
pub struct QueryFilter {
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub instrument: Option<String>,
    pub crew: Option<String>,
    pub control_point: Option<String>,
    pub obs_type: Option<ObsType>,
}

pub struct ObservationDB {
    conn: Connection,
}

impl ObservationDB {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS observations (
                id INTEGER PRIMARY KEY,
                obs_type TEXT NOT NULL,
                date TEXT NOT NULL,
                instrument TEXT,
                crew TEXT,
                control_point TEXT,
                data TEXT NOT NULL
            )",
        )?;
        Ok(Self { conn })
    }

    pub fn insert(&self, rec: &ObservationRecord) -> rusqlite::Result<i64> {
        let data_json = serde_json::to_string(&rec.data).unwrap();
        self.conn.execute(
            "INSERT INTO observations (obs_type, date, instrument, crew, control_point, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                format!("{:?}", rec.obs_type),
                rec.date.to_string(),
                rec.instrument,
                rec.crew,
                rec.control_point,
                data_json
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn query(&self, filter: &QueryFilter) -> rusqlite::Result<Vec<ObservationRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, obs_type, date, instrument, crew, control_point, data FROM observations",
        )?;
        let rows = stmt.query_map([], |row| {
            let data_str: String = row.get(6)?;
            let obs_type_str: String = row.get(1)?;
            let rec = ObservationRecord {
                id: row.get(0)?,
                obs_type: match obs_type_str.as_str() {
                    "TotalStation" => ObsType::TotalStation,
                    "Gnss" => ObsType::Gnss,
                    "LevelRun" => ObsType::LevelRun,
                    "Traverse" => ObsType::Traverse,
                    _ => ObsType::TotalStation,
                },
                date: NaiveDate::parse_from_str(row.get::<_, String>(2)?.as_str(), "%Y-%m-%d").unwrap(),
                instrument: row.get(3)?,
                crew: row.get(4)?,
                control_point: row.get(5)?,
                data: serde_json::from_str(&data_str).unwrap(),
            };
            Ok(rec)
        })?;
        let mut res = Vec::new();
        for r in rows {
            let rec = r?;
            if let Some(ref t) = filter.obs_type {
                if &rec.obs_type != t { continue; }
            }
            if let Some(ref s) = filter.instrument {
                if rec.instrument.as_deref() != Some(s.as_str()) { continue; }
            }
            if let Some(ref s) = filter.crew {
                if rec.crew.as_deref() != Some(s.as_str()) { continue; }
            }
            if let Some(ref s) = filter.control_point {
                if rec.control_point.as_deref() != Some(s.as_str()) { continue; }
            }
            if let Some(from) = filter.date_from {
                if rec.date < from { continue; }
            }
            if let Some(to) = filter.date_to {
                if rec.date > to { continue; }
            }
            res.push(rec);
        }
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn insert_and_query() {
        let file = NamedTempFile::new().unwrap();
        let db = ObservationDB::open(file.path().to_str().unwrap()).unwrap();
        let rec = ObservationRecord {
            id: None,
            obs_type: ObsType::Gnss,
            date: NaiveDate::from_ymd_opt(2024,1,1).unwrap(),
            instrument: Some("GNSS1".into()),
            crew: Some("crew1".into()),
            control_point: Some("CP1".into()),
            data: ObservationData::Gnss {
                point: "P1".into(),
                northing: 100.0,
                easting: 200.0,
                elevation: 50.0,
            },
        };
        db.insert(&rec).unwrap();
        let filter = QueryFilter { instrument: Some("GNSS1".into()), ..Default::default() };
        let res = db.query(&filter).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].instrument.as_deref(), Some("GNSS1"));
    }
}
