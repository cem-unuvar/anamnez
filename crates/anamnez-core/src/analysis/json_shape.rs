//! Patient JSON shape fed into the analysis prompt.
//!
//! Includes demographics + active problem list + allergies + medications +
//! encounters + observation history.

use crate::db::Database;
use crate::error::Result;
use crate::ids::PatientId;
use rusqlite::params;
use serde_json::{json, Value};

pub fn build(db: &Database, patient_id: PatientId) -> Result<Value> {
    db.with_reader(|conn| {
        // Demographics (only non-PHI-sensitive fields needed for analysis context).
        let (given_names, family_name, dob, sex, gender_identity, deceased_at): (
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
        ) = conn.query_row(
            "SELECT given_names, family_name, date_of_birth, sex_assigned_at_birth, gender_identity, deceased_at \
             FROM patient WHERE id = ?1 AND suppressed_at IS NULL",
            params![patient_id.as_uuid().to_string()],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )?;

        let problems = collect(
            conn,
            "SELECT id, code, code_system, display_text, recorded_at, effective_period_start \
             FROM observation \
             WHERE patient_id = ?1 AND is_problem_list_item = 1 AND status = 'final' \
               AND effective_period_end IS NULL \
             ORDER BY recorded_at DESC",
            patient_id,
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "code": r.get::<_, Option<String>>(1)?,
                    "code_system": r.get::<_, Option<String>>(2)?,
                    "display_text": r.get::<_, String>(3)?,
                    "recorded_at": r.get::<_, String>(4)?,
                    "effective_period_start": r.get::<_, String>(5)?,
                }))
            },
        )?;

        let allergies = collect(
            conn,
            "SELECT id, code, code_system, display_text, severity, status FROM allergy WHERE patient_id = ?1",
            patient_id,
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "code": r.get::<_, Option<String>>(1)?,
                    "code_system": r.get::<_, Option<String>>(2)?,
                    "display_text": r.get::<_, String>(3)?,
                    "severity": r.get::<_, String>(4)?,
                    "status": r.get::<_, String>(5)?,
                }))
            },
        )?;

        let medications = collect(
            conn,
            "SELECT id, code, code_system, display_text, dose_quantity, dose_unit, route, status, started_at, ended_at \
             FROM medication WHERE patient_id = ?1",
            patient_id,
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "code": r.get::<_, String>(1)?,
                    "code_system": r.get::<_, String>(2)?,
                    "display_text": r.get::<_, String>(3)?,
                    "dose_quantity": r.get::<_, Option<f64>>(4)?,
                    "dose_unit": r.get::<_, Option<String>>(5)?,
                    "route": r.get::<_, String>(6)?,
                    "status": r.get::<_, String>(7)?,
                    "started_at": r.get::<_, String>(8)?,
                    "ended_at": r.get::<_, Option<String>>(9)?,
                }))
            },
        )?;

        let encounters = collect(
            conn,
            "SELECT id, kind, reason_text, reason_code, reason_code_system, started_at, ended_at, status \
             FROM encounter WHERE patient_id = ?1 ORDER BY started_at DESC",
            patient_id,
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "kind": r.get::<_, String>(1)?,
                    "reason_text": r.get::<_, String>(2)?,
                    "reason_code": r.get::<_, Option<String>>(3)?,
                    "reason_code_system": r.get::<_, Option<String>>(4)?,
                    "started_at": r.get::<_, String>(5)?,
                    "ended_at": r.get::<_, Option<String>>(6)?,
                    "status": r.get::<_, String>(7)?,
                }))
            },
        )?;

        let observations = collect(
            conn,
            "SELECT id, code, code_system, display_text, status, effective_period_start, effective_period_end \
             FROM observation WHERE patient_id = ?1 ORDER BY effective_period_start ASC",
            patient_id,
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "code": r.get::<_, Option<String>>(1)?,
                    "code_system": r.get::<_, Option<String>>(2)?,
                    "display_text": r.get::<_, String>(3)?,
                    "status": r.get::<_, String>(4)?,
                    "effective_period_start": r.get::<_, String>(5)?,
                    "effective_period_end": r.get::<_, Option<String>>(6)?,
                }))
            },
        )?;

        Ok(json!({
            "patient": {
                "given_names": given_names,
                "family_name": family_name,
                "date_of_birth": dob,
                "sex_assigned_at_birth": sex,
                "gender_identity": gender_identity,
                "deceased_at": deceased_at,
            },
            "problems": problems,
            "allergies": allergies,
            "medications": medications,
            "encounters": encounters,
            "observations": observations,
        }))
    })
}

fn collect(
    conn: &rusqlite::Connection,
    sql: &str,
    patient_id: PatientId,
    f: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<Value>,
) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![patient_id.as_uuid().to_string()], f)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
