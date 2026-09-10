use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use chrono::NaiveDate;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::routes::Route;
use crate::utils::parse_int_bool;

/// `POST api/student/all_subjects` — every subject with a grade summary, scoped
/// to whichever semester the session is currently switched to server-side
/// (there is no `semester` parameter — see the semester-switch endpoint in
/// `docs/API.md`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllSubjectsRoute {
    /// The student's user id, as read from the session config.
    pub(crate) student_id: i64,
}

impl Route for AllSubjectsRoute {
    type Response = AllSubjectsResponse;

    fn get_route() -> &'static str {
        "api/student/all_subjects"
    }
}

#[derive(Debug, Deserialize)]
pub struct AllSubjectsResponse {
    pub subjects: Vec<SubjectGrades>,
}

/// One subject and the student's grades in it for the current semester. This is
/// a summary; `api/student/subject_detail` carries the full per-grade data.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectGrades {
    pub subject: Subject,
    pub absences: i64,
    pub grades: Vec<GradeEntry>,
    pub average_semester: f64,
    pub average_year: f64,
    pub subject_id: i64,
    pub student: Student,
    pub count_competences: i64,
    pub count_descriptions: i64,
    pub count_observations: i64,
}

#[derive(Debug, Deserialize)]
pub struct Subject {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Student {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeEntry {
    pub grade: Grade,
    /// Percent weight of this grade in the subject average.
    pub weight: i64,
    pub date: NaiveDate,
    /// Display name of the grade type. Named `type` here, but `typeName` in
    /// `subject_detail` — a documented cross-endpoint inconsistency.
    #[serde(rename = "type")]
    pub type_name: String,
    pub type_id: i64,
    pub student_id: i64,
    pub subject_id: i64,
    pub semester: i64,
    /// `0` / non-`0` on the wire here (an int, not the bool `subject_detail`
    /// uses for the same field).
    #[serde(deserialize_with = "parse_int_bool")]
    pub cancelled: bool,
    pub created_time_stamp: String,
    pub cancelled_time_stamp: Option<String>,
    pub description: String,
}

/// A grade sent as a fixed two-decimal string like `"9.00"` or `"9.75"`.
///
/// Stored as a "centi-grade" integer (`major * 100 + minor`), the same
/// representation the official client parses it into. The fractional part is
/// one of `.00`, `.25`, `.50`, `.75` in practice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Grade(i32);

impl Grade {
    /// The grade as hundredths, e.g. `925` for `"9.25"`.
    pub fn centi(self) -> i32 {
        self.0
    }

    /// The integer part, e.g. `9` for `"9.25"`.
    pub fn whole(self) -> i32 {
        self.0 / 100
    }

    /// The fractional part in hundredths, e.g. `25` for `"9.25"`.
    pub fn fraction(self) -> i32 {
        self.0 % 100
    }

    pub fn as_f64(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not a `major.dd` grade string")]
pub struct GradeParseError(String);

impl FromStr for Grade {
    type Err = GradeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Mirror the client: split on the dot and take the two halves as
        // integers directly (`docs/API.md` — the digits after the dot are used
        // verbatim, so they must always be exactly two).
        let err = || GradeParseError(s.to_owned());
        let (whole, fraction) = s.split_once('.').ok_or_else(err)?;
        let whole: i32 = whole.parse().map_err(|_| err())?;
        let fraction: i32 = fraction.parse().map_err(|_| err())?;
        Ok(Grade(whole * 100 + fraction))
    }
}

impl Display for Grade {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}.{:02}", self.whole(), self.fraction())
    }
}

impl<'de> Deserialize<'de> for Grade {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_the_student_id_with_a_camel_case_key() {
        assert_eq!(
            serde_json::to_value(AllSubjectsRoute { student_id: 6619 }).unwrap(),
            serde_json::json!({ "studentId": 6619 }),
        );
    }

    #[test]
    fn parses_dotted_grade_strings() {
        assert_eq!("9.00".parse(), Ok(Grade(900)));
        assert_eq!("9.75".parse(), Ok(Grade(975)));
        assert_eq!("10.00".parse(), Ok(Grade(1000)));
        assert_eq!(Grade(925).to_string(), "9.25");
        assert_eq!(Grade(950).as_f64(), 9.5);
    }

    #[test]
    fn rejects_malformed_grade_strings() {
        assert!("9".parse::<Grade>().is_err());
        assert!("".parse::<Grade>().is_err());
        assert!("abc".parse::<Grade>().is_err());
    }

    #[test]
    fn deserializes_a_subject_entry() {
        let json = r#"{
            "subject": { "id": 17, "name": "Bewegung und Sport" },
            "absences": 0,
            "grades": [
                {
                    "grade": "9.00", "weight": 100, "date": "2022-10-11",
                    "type": "Sonstige Bewertung", "typeId": 7, "studentId": 6619,
                    "cancelled": 0, "subjectId": 17, "semester": 1,
                    "createdTimeStamp": "2022-10-17 12:34:12",
                    "cancelledTimeStamp": null, "description": ""
                }
            ],
            "averageSemester": 0, "averageYear": 0, "subjectId": 17,
            "student": { "id": 6619, "firstName": "Michael", "lastName": "Debertol" },
            "countCompetences": 0, "countDescriptions": 0, "countObservations": 1
        }"#;

        let entry: SubjectGrades = serde_json::from_str(json).unwrap();

        assert_eq!(entry.subject.name, "Bewegung und Sport");
        assert_eq!(entry.grades.len(), 1);
        assert_eq!(entry.grades[0].grade, Grade(900));
        assert!(!entry.grades[0].cancelled);
        assert_eq!(entry.student.last_name, "Debertol");
    }
}