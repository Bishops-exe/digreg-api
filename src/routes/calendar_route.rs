use std::collections::BTreeMap;
use std::ops::Range;

use chrono::{NaiveDate, NaiveTime, TimeDelta};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::routes::Route;
use crate::utils::parse_int_bool;

/// `POST api/calendar/student` — one school week (5 days) of timetable data.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarRoute {
    /// Monday of the week to fetch (`yyyy-MM-dd`). The server rejects
    /// non-Monday dates.
    pub(crate) start_date: NaiveDate,
}

impl Route for CalendarRoute {
    type Response = CalendarResponse;

    fn get_route() -> &'static str {
        "api/calendar/student"
    }
}

/// One school week: a map keyed by date (`yyyy-MM-dd`), each day the lessons
/// scheduled that day in period order.
///
/// The wire format is much messier — every day is buried under two
/// meaningless nested maps and then a period-keyed map of slots, and a lesson
/// covering several periods carries the extras in a `linkedHours` array.
/// [`Deserialize`] flattens all of it: non-lesson slots are dropped and every
/// `linkedHours` entry is hoisted into a standalone [`Lesson`] on its own
/// date, so each element here is a plain [`Lesson`] and nothing is nested.
#[derive(Debug, Default)]
pub struct CalendarResponse(pub BTreeMap<NaiveDate, Vec<Lesson>>);

impl std::ops::Deref for CalendarResponse {
    type Target = BTreeMap<NaiveDate, Vec<Lesson>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSlot {
    #[serde(deserialize_with = "parse_int_bool")]
    is_lesson: bool,
    lesson: Option<RawLesson>,
}

/// A lesson as it arrives on the wire: a [`Lesson`] plus the `linkedHours`
/// array of further periods belonging to the same block. The array never
/// reaches the public API — [`extract_days`] drains it into standalone
/// lessons.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLesson {
    #[serde(flatten)]
    lesson: Lesson,
    #[serde(default)]
    linked_hours: Vec<RawLesson>,
}

/// The wire shape: `date -> <ignored> -> <ignored> -> period number -> slot`.
type RawWeek = BTreeMap<NaiveDate, BTreeMap<String, BTreeMap<String, BTreeMap<u32, RawSlot>>>>;

/// A period-keyed timetable, `date -> period -> lesson`.
type DayGrid = BTreeMap<NaiveDate, BTreeMap<u32, Lesson>>;

impl<'de> Deserialize<'de> for CalendarResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let grid = extract_days(RawWeek::deserialize(deserializer)?);
        Ok(CalendarResponse(grid_to_lists(grid)))
    }
}

impl RawSlot {
    /// The lesson in this slot, or `None` for a period with nothing scheduled.
    fn into_lesson(self) -> Option<RawLesson> {
        self.lesson.filter(|_| self.is_lesson)
    }
}

/// Flatten the wire week into a period-keyed [`DayGrid`]: drop the two
/// meaningless nesting levels, discard non-lesson slots, and hoist every
/// `linkedHours` entry into its own period slot on its own date.
///
/// Linked hours are walked with an explicit work list, so nesting of any
/// depth is handled without recursion.
fn extract_days(raw: RawWeek) -> DayGrid {
    let mut days = DayGrid::new();
    let mut pending: Vec<RawLesson> = raw
        .into_values()
        .flat_map(BTreeMap::into_values)
        .flat_map(BTreeMap::into_values)
        .flat_map(BTreeMap::into_values)
        .filter_map(RawSlot::into_lesson)
        .collect();

    while let Some(mut raw) = pending.pop() {
        pending.append(&mut raw.linked_hours);
        let period = u32::from(raw.lesson.span.start.hour);
        days.entry(raw.lesson.date)
            .or_default()
            .insert(period, raw.lesson);
    }

    days
}

/// Collapse each day's period map into a list of lessons in period order.
fn grid_to_lists(days: DayGrid) -> BTreeMap<NaiveDate, Vec<Lesson>> {
    days.into_iter()
        .map(|(date, day)| (date, day.into_values().collect()))
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    pub id: Option<i64>,
    pub ttcid: i64,
    pub date: NaiveDate,
    /// The block's extent: `.start` is the first period (its number and start
    /// time), `.end` is the last period covered (its number and end time).
    /// Built from `hour` / `toHour` / `timeStartObject` / `timeToEndObject`.
    #[serde(flatten)]
    pub span: LessonRange,
    pub time_show_enabled: bool,
    pub class_id: i64,
    pub class_name: String,
    pub class_comment: String,
    pub description: String,
    pub note: String,
    pub lesson_show: bool,
    pub teachers: Vec<Teacher>,
    pub teachers_source: Option<i64>,
    pub teachers_to_notify: Vec<Value>,
    pub teacher_myself: Option<Value>,
    pub can_see_teacher_source_markers: bool,
    pub is_auto_notify: bool,
    pub is_lesson_type_notify_on: bool,
    #[serde(rename = "exp_lt_default")]
    pub exp_lt_default: bool,
    pub is_secretary: bool,
    pub subject: Subject,
    pub homework_exams: Vec<HomeworkExam>,
    pub homework_exams_other: Vec<HomeworkExam>,
    pub lesson_contents: Vec<LessonContent>,
    pub rooms: Vec<Room>,
    pub read_only: bool,
    #[serde(deserialize_with = "parse_int_bool")]
    pub is_substitute: bool,
    #[serde(deserialize_with = "parse_int_bool")]
    pub link_to_previous_hour: bool,
    #[serde(deserialize_with = "deserialize_minutes", rename = "lessonDurationMinutes")]
    pub lesson_duration: TimeDelta,
    pub critical_observations: Vec<Value>,
    pub missing_students: Vec<Value>,
    pub missing_students_including_inactive: Vec<Value>,
    pub students: Vec<Value>,
    pub grades: Vec<Value>,
    pub observations: Vec<Value>,
    pub absence_open_absences_students: Vec<Value>,
}

fn deserialize_minutes<'de, D>(deserializer: D) -> Result<TimeDelta, D::Error>
where
    D: Deserializer<'de>,
{
    // Convert the Option to a Result to safely handle overflows
    TimeDelta::try_minutes(i64::deserialize(deserializer)?)
        .ok_or_else(|| de::Error::custom("Minute value caused a TimeDelta overflow"))
}

/// One end of a [`LessonRange`]: a lesson-period number and the wall-clock
/// time at that boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LessonBound {
    pub time: NaiveTime,
    /// Lesson-period number (`1..=9`).
    pub hour: u8,
}

/// A lesson block's extent, `first period start .. last period end`.
/// Derefs to [`Range<LessonBound>`].
#[derive(Debug, Clone)]
pub struct LessonRange(Range<LessonBound>);

impl LessonRange {
    /// Wall-clock time from the block's start to its end, breaks between
    /// periods included. For a per-period figure see [`Lesson::lesson_duration`].
    pub fn duration(&self) -> TimeDelta {
        self.end.time - self.start.time
    }
}

impl std::ops::Deref for LessonRange {
    type Target = Range<LessonBound>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for LessonRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            hour: u8,
            to_hour: u8,
            #[serde(deserialize_with = "time_from_object")]
            time_start_object: NaiveTime,
            #[serde(deserialize_with = "time_from_object")]
            time_to_end_object: NaiveTime,
        }

        let Raw {
            hour,
            to_hour,
            time_start_object,
            time_to_end_object,
        } = Raw::deserialize(deserializer)?;

        Ok(LessonRange(
            LessonBound { time: time_start_object, hour }
                ..LessonBound { time: time_to_end_object, hour: to_hour },
        ))
    }
}

/// The API sends each time as `{ "h", "m", "ts", "text", "html" }`; we keep
/// only the wall-clock time, taken from `ts` (seconds since midnight).
fn time_from_object<'de, D>(deserializer: D) -> Result<NaiveTime, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Raw {
        ts: i64,
    }

    let ts = Raw::deserialize(deserializer)?.ts;
    let secs = u32::try_from(ts).map_err(|_| de::Error::custom("time `ts` out of range"))?;

    NaiveTime::from_num_seconds_from_midnight_opt(secs, 0)
        .ok_or_else(|| de::Error::custom("time `ts` out of range"))
}

#[derive(Debug, Deserialize)]
pub struct Teacher {
    pub id: i64,
    #[serde(rename = "firstName")]
    pub first_name: String,
    #[serde(rename = "lastName")]
    pub last_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
    pub id: i64,
    pub name: String,
    pub lernfeld: i64,
    pub default_lesson_content: String,
    pub default_lesson_content_type: i64,
}

#[derive(Debug, Deserialize)]
pub struct Room {
    pub id: Option<i64>,
    pub name: String,
}

/// Homework / exam entry. Not present in the captured sample; shape taken
/// from `docs/API.md`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeworkExam {
    pub id: i64,
    pub name: String,
    /// Small category code, not a flag. `0` is the client's "warning"
    /// signal (see `docs/API.md`); other values (e.g. `5`) also occur.
    pub homework: i64,
    pub online: i64,
    pub deadline: Option<String>,
    #[serde(default)]
    pub has_grades: bool,
    #[serde(default)]
    pub has_grade_group_submissions: bool,
    pub type_id: i64,
    pub type_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonContent {
    pub id: i64,
    pub name: String,
    /// Small category code, not a flag; see [`HomeworkExam::homework`].
    pub homework: i64,
    pub online: i64,
    pub deadline_start: Option<String>,
    pub deadline: Option<String>,
    pub deadline_overtime: Option<String>,
    pub has_lesson_content_submissions: bool,
    pub type_id: i64,
    pub type_name: String,
    pub lesson_content_submissions: Vec<LessonContentSubmission>,
    pub lesson_content_students: Vec<Value>,
    pub lesson_content_students_percentage: i64,
}

/// Submission attached to a [`LessonContent`]. Shape from `docs/API.md`;
/// note `id` / `lessonContentId` are strings here, unlike elsewhere in the API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonContentSubmission {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub original_name: Option<String>,
    pub id: String,
    pub lesson_content_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOUBLE_LESSON: &str = r#"{
        "id": 136, "ttcid": 336326, "date": "2026-09-08",
        "hour": 2, "toHour": 3,
        "timeStart": 27600, "timeEnd": 30600, "timeToEnd": 33900,
        "timeStartObject":  { "h": "08", "m": "40", "ts": 27600, "text": "08:40", "html": "08:40" },
        "timeEndObject":    { "h": "09", "m": "30", "ts": 30600, "text": "09:30", "html": "09:30" },
        "timeToEndObject":  { "h": "10", "m": "25", "ts": 33900, "text": "10:25", "html": "10:25" },
        "timeShowEnabled": true,
        "classId": 278, "className": "1AT", "classComment": "", "description": "", "note": "",
        "lessonShow": true, "teachers": [], "teachersSource": 0, "teachersToNotify": [],
        "teacherMyself": null, "canSeeTeacherSourceMarkers": false, "isAutoNotify": false,
        "isLessonTypeNotifyOn": false, "exp_lt_default": false, "isSecretary": false,
        "subject": { "id": 31, "name": "TZ", "lernfeld": 0, "defaultLessonContent": "", "defaultLessonContentType": 0 },
        "homeworkExams": [], "homeworkExamsOther": [], "lessonContents": [], "rooms": [],
        "readOnly": true, "isSubstitute": 0, "linkToPreviousHour": 0, "linkedHours": [],
        "lessonDurationMinutes": 50,
        "criticalObservations": [], "missingStudents": [], "missingStudentsIncludingInactive": [],
        "students": [], "grades": [], "observations": [], "absenceOpenAbsencesStudents": []
    }"#;

    #[test]
    fn serializes_the_start_date_as_an_iso_string() {
        let route = CalendarRoute {
            start_date: NaiveDate::from_ymd_opt(2022, 10, 17).unwrap(),
        };
        assert_eq!(
            serde_json::to_value(route).unwrap(),
            serde_json::json!({ "startDate": "2022-10-17" }),
        );
    }

    #[test]
    fn flattens_span_from_hour_and_time_objects() {
        let lesson: Lesson = serde_json::from_str(DOUBLE_LESSON).unwrap();

        assert_eq!(lesson.span.start.hour, 2);
        assert_eq!(lesson.span.end.hour, 3);
        assert_eq!(lesson.span.start.time, NaiveTime::from_hms_opt(7, 40, 0).unwrap());
        assert_eq!(lesson.span.end.time, NaiveTime::from_hms_opt(9, 25, 0).unwrap());
        assert_eq!(lesson.span.duration(), TimeDelta::minutes(105));
        assert_eq!(lesson.lesson_duration, TimeDelta::minutes(50));
    }

    /// A lesson slot on `date` spanning the single period `hour`, with the
    /// given `linked_hours` JSON spliced into its `linkedHours` array.
    fn lesson_json(date: &str, hour: u8, linked_hours: &str) -> String {
        let start = 25200 + u32::from(hour) * 3000;
        format!(
            r#"{{
                "id": null, "ttcid": 1, "date": "{date}",
                "hour": {hour}, "toHour": {hour},
                "timeStartObject": {{ "ts": {start} }},
                "timeEndObject":   {{ "ts": {end} }},
                "timeToEndObject": {{ "ts": {end} }},
                "timeShowEnabled": true,
                "classId": 1, "className": "1A", "classComment": "", "description": "", "note": "",
                "lessonShow": true, "teachers": [], "teachersSource": 0, "teachersToNotify": [],
                "teacherMyself": null, "canSeeTeacherSourceMarkers": false, "isAutoNotify": false,
                "isLessonTypeNotifyOn": false, "exp_lt_default": false, "isSecretary": false,
                "subject": {{ "id": 1, "name": "M", "lernfeld": 0, "defaultLessonContent": "", "defaultLessonContentType": 0 }},
                "homeworkExams": [], "homeworkExamsOther": [], "lessonContents": [], "rooms": [],
                "readOnly": true, "isSubstitute": 0, "linkToPreviousHour": 0,
                "linkedHours": [{linked_hours}],
                "lessonDurationMinutes": 50,
                "criticalObservations": [], "missingStudents": [], "missingStudentsIncludingInactive": [],
                "students": [], "grades": [], "observations": [], "absenceOpenAbsencesStudents": []
            }}"#,
            end = start + 2700,
        )
    }

    #[test]
    fn flattens_week_and_hoists_linked_hours() {
        // One day, one wire slot: a period-1 lesson whose block also covers
        // period 4 (carried in `linkedHours`), plus an empty non-lesson slot.
        let linked = lesson_json("2026-09-07", 4, "");
        let main = lesson_json("2026-09-07", 1, &linked);
        let week = format!(
            r#"{{ "2026-09-07": {{ "a": {{ "b": {{
                "1": {{ "isLesson": 1, "lesson": {main} }},
                "2": {{ "isLesson": 0, "lesson": null }}
            }} }} }} }}"#
        );

        let response: CalendarResponse = serde_json::from_str(&week).unwrap();

        let day = &response[&NaiveDate::from_ymd_opt(2026, 9, 7).unwrap()];
        let hours: Vec<u8> = day.iter().map(|lesson| lesson.span.start.hour).collect();
        assert_eq!(hours, [1, 4], "linked hour is hoisted into its own slot, in period order");
    }
}
