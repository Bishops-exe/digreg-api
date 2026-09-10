use chrono::NaiveDate;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::routes::Route;

/// `POST api/student/dashboard/absences` — the student's absences (past and
/// upcoming) plus summary statistics. Empty request body.
#[derive(Serialize)]
pub struct AbsencesRoute {}

impl Route for AbsencesRoute {
    type Response = AbsencesResponse;

    fn get_route() -> &'static str {
        "api/student/dashboard/absences"
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbsencesResponse {
    pub absences: Vec<Absence>,
    pub future_absences: Vec<FutureAbsence>,
    pub can_edit: bool,
    pub statistics: AbsenceStatistics,
    /// Static legal-text templates offered when self-declaring an absence.
    pub self_declarations_list: Vec<String>,
    pub self_declarations_active_list: Vec<Value>,
    pub is_absences_self_declaration_active: bool,
    pub is_absences_self_declaration_mandatory: bool,
}

/// A past absence, made up of one or more consecutive missed lesson hours.
///
/// The wire field names here are `snake_case`, unlike the `camelCase` used at
/// the top level and in [`FutureAbsence`].
#[derive(Debug, Deserialize)]
pub struct Absence {
    #[serde(deserialize_with = "deserialize_justification")]
    pub justified: Justification,
    pub reason: Option<String>,
    pub note: Option<String>,
    pub reason_signature: Option<String>,
    pub reason_timestamp: Option<String>,
    pub group: Vec<AbsentHour>,
}

impl Absence {
    /// Total minutes missed across every hour in this absence.
    pub fn total_minutes(&self) -> u64 {
        self.group.iter().map(|hour| hour.minutes).sum()
    }
}

/// One missed lesson period within an [`Absence`].
#[derive(Debug, Deserialize)]
pub struct AbsentHour {
    pub date: NaiveDate,
    /// Lesson-period number.
    pub hour: u64,
    /// Duration in minutes; `50` is one full lesson.
    pub minutes: u64,
    /// Minutes arrived late.
    pub minutes_begin: u64,
    /// Minutes left early.
    pub minutes_end: u64,
}

/// An upcoming absence (announced in advance), given as a date/period range
/// rather than a list of hours.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FutureAbsence {
    #[serde(deserialize_with = "deserialize_justification")]
    pub justified: Justification,
    pub reason: Option<String>,
    pub note: Option<String>,
    #[serde(rename = "reason_signature")]
    pub reason_signature: Option<String>,
    #[serde(rename = "reason_timestamp")]
    pub reason_timestamp: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    /// First lesson-period number covered.
    pub start_time: u64,
    /// Last lesson-period number covered.
    pub end_time: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbsenceStatistics {
    pub counter: u64,
    pub counter_for_school: u64,
    /// Sent as a string on the wire (a bare number in some captures), and
    /// `None` when empty.
    #[serde(default, deserialize_with = "deserialize_percentage")]
    pub percentage: Option<f64>,
    pub justified: u64,
    pub not_justified: u64,
    pub delayed: u64,
}

/// Whether an absence has been excused. Decoded from the integer `justified`
/// code (see `docs/API.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justification {
    /// `2` — justified.
    Justified,
    /// `3` — not justified.
    NotJustified,
    /// `4` — excused for a school activity (e.g. a field trip).
    ForSchool,
    /// Any other code — not yet processed.
    Pending,
}

impl Justification {
    pub fn from_code(code: u64) -> Self {
        match code {
            2 => Self::Justified,
            3 => Self::NotJustified,
            4 => Self::ForSchool,
            _ => Self::Pending,
        }
    }
}

fn deserialize_justification<'de, D>(deserializer: D) -> Result<Justification, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Justification::from_code(u64::deserialize(deserializer)?))
}

fn deserialize_percentage<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Number(f64),
        Text(String),
    }

    Ok(match Option::<Raw>::deserialize(deserializer)? {
        None => None,
        Some(Raw::Number(n)) => Some(n),
        Some(Raw::Text(s)) => {
            let s = s.trim();
            match s.is_empty() {
                true => None,
                false => Some(s.parse().map_err(de::Error::custom)?),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = r#"{
        "absences": [
            {
                "justified": 2, "reason": null, "note": null,
                "reason_signature": null, "reason_timestamp": null,
                "group": [
                    { "date": "2022-09-12", "hour": 3, "minutes": 50, "minutes_begin": 0, "minutes_end": 0 },
                    { "date": "2022-09-12", "hour": 4, "minutes": 50, "minutes_begin": 0, "minutes_end": 0 }
                ]
            }
        ],
        "futureAbsences": [
            {
                "justified": 4, "reason": "Arzttermin", "note": null,
                "reason_signature": null, "reason_timestamp": null,
                "startDate": "2022-11-01", "endDate": "2022-11-01",
                "startTime": 1, "endTime": 3
            }
        ],
        "canEdit": true,
        "statistics": {
            "counter": 0, "counterForSchool": 0, "percentage": 0,
            "justified": 0, "notJustified": 0, "delayed": 0
        },
        "selfDeclarationsList": ["Ich erkläre ..."],
        "selfDeclarationsActiveList": [],
        "isAbsencesSelfDeclarationActive": false,
        "isAbsencesSelfDeclarationMandatory": true
    }"#;

    #[test]
    fn serializes_as_an_empty_json_object() {
        assert_eq!(
            serde_json::to_value(AbsencesRoute {}).unwrap(),
            serde_json::json!({}),
        );
    }

    #[test]
    fn justification_codes_decode_to_the_documented_variants() {
        assert_eq!(Justification::from_code(2), Justification::Justified);
        assert_eq!(Justification::from_code(3), Justification::NotJustified);
        assert_eq!(Justification::from_code(4), Justification::ForSchool);
        assert_eq!(Justification::from_code(0), Justification::Pending);
        assert_eq!(Justification::from_code(99), Justification::Pending);
    }

    #[test]
    fn deserializes_the_absences_response() {
        let response: AbsencesResponse = serde_json::from_str(RESPONSE).unwrap();

        assert_eq!(response.absences.len(), 1);
        assert_eq!(response.absences[0].justified, Justification::Justified);
        assert_eq!(response.absences[0].total_minutes(), 100);
        assert_eq!(response.future_absences[0].justified, Justification::ForSchool);
        assert_eq!(response.future_absences[0].reason.as_deref(), Some("Arzttermin"));
        assert_eq!(response.statistics.percentage, Some(0.0));
    }

    #[test]
    fn percentage_handles_string_and_empty() {
        let stats: AbsenceStatistics = serde_json::from_str(
            r#"{ "counter": 1, "counterForSchool": 0, "percentage": "12.5",
                 "justified": 1, "notJustified": 0, "delayed": 0 }"#,
        )
        .unwrap();
        assert_eq!(stats.percentage, Some(12.5));

        let stats: AbsenceStatistics = serde_json::from_str(
            r#"{ "counter": 0, "counterForSchool": 0, "percentage": "",
                 "justified": 0, "notJustified": 0, "delayed": 0 }"#,
        )
        .unwrap();
        assert_eq!(stats.percentage, None);
    }
}