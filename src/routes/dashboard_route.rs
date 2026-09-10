use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use crate::routes::{FetchError, Route};
use crate::utils::only_200;
use crate::Session;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardRoute {
    pub(crate) view_future: bool,
}

impl Route for DashboardRoute {
    type Response = DashboardResponse;

    fn get_route() -> &'static str {
        "api/student/dashboard/dashboard"
    }
}

/// `POST api/student/dashboard/save_reminder` — create a user-authored
/// reminder. The response is a single dashboard item (the freshly created
/// reminder).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReminderRoute {
    pub(crate) date: NaiveDate,
    pub(crate) text: String,
}

impl Route for SaveReminderRoute {
    type Response = DashboardItem;

    fn get_route() -> &'static str {
        "api/student/dashboard/save_reminder"
    }
}

/// `POST api/student/dashboard/delete_reminder` — delete a user-authored
/// reminder by id.
#[derive(Serialize)]
pub struct DeleteReminderRoute {
    pub(crate) id: i64,
}

impl Route for DeleteReminderRoute {
    type Response = SuccessResponse;

    fn get_route() -> &'static str {
        "api/student/dashboard/delete_reminder"
    }
}

/// `POST api/student/dashboard/toggle_reminder` — set the done-state of a
/// checkable dashboard item.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleReminderRoute {
    pub(crate) id: i64,
    /// The item's `type` string (the `r#type` field of [`DashboardItem`]).
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) value: bool,
}

impl Route for ToggleReminderRoute {
    type Response = SuccessResponse;

    fn get_route() -> &'static str {
        "api/student/dashboard/toggle_reminder"
    }
}

/// `{ "success": bool }` — returned by the reminder mutation endpoints. The
/// client treats anything other than `success: true` as a failed operation.
#[derive(Debug, Deserialize)]
pub struct SuccessResponse {
    pub success: bool,
}

/// The dashboard response is a bare JSON array of day objects, one per date in
/// the requested range. Derefs to `[DashboardDate]`.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct DashboardResponse(pub Vec<DashboardDate>);

impl std::ops::Deref for DashboardResponse {
    type Target = [DashboardDate];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Deserialize)]
pub struct DashboardDate {
    pub date: String,
    pub items: Vec<DashboardItem>,
}

/// A dashboard entry. School-authored items (homework, grades, …) carry the
/// full field set below; user-created reminders are a much smaller object
/// (`id`, `type`, `title`, `subtitle`, `warning`, `checkable`, `checked`,
/// `deleteable` only — the same shape `save_reminder` returns), so every
/// school-only field is optional.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardItem {
    pub id: i64,
    #[serde(default)]
    pub category: Option<i64>,
    pub r#type: String,
    pub title: String,
    pub subtitle: String,
    /// Usually the subject name; absent or `null` on some items
    /// (`docs/API.md` — `string?`).
    #[serde(default)]
    pub label: Option<String>,
    pub warning: bool,
    pub checkable: bool,
    pub checked: bool,
    #[serde(default)]
    pub online: i32,
    #[serde(default)]
    pub submission: Option<serde_json::Value>,
    /// Only user-created reminders are deletable; the field is absent on
    /// school-authored items (`docs/API.md` spells it `deleteable`).
    #[serde(default, rename = "deleteable")]
    deletable: bool,
    /// Deadline strings are absent or `null` on items that carry no deadline
    /// (e.g. reminders).
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default)]
    pub deadline_formatted: Option<String>,
    #[serde(default)]
    pub deadline_start: Option<serde_json::Value>,
    #[serde(default)]
    pub deadline_start_formatted: Option<String>,
    #[serde(default)]
    pub submission_allowed: i32,
    #[serde(default)]
    pub submission_resigned: bool,
    #[serde(default)]
    pub submission_is_now_in_overtime: bool,
    /// Small category code, not a flag. `0` is the client's "warning"
    /// signal (see `docs/API.md`); other values also occur.
    #[serde(default)]
    pub homework: i64,
    #[serde(default)]
    pub done: Option<serde_json::Value>,
    #[serde(default)]
    pub grade_group_submissions: Option<serde_json::Value>,
}


impl DashboardItem {
    /// Set this item's done-state on the server (`toggle_reminder`).
    ///
    /// Only meaningful for checkable items ([`checkable`](Self::checkable));
    /// the local `checked` field is left untouched.
    pub async fn set_is_done(&self, session: &Session, is_done: bool) -> Result<(), FetchError> {
        let route = ToggleReminderRoute {
            id: self.id,
            kind: self.r#type.clone(),
            value: is_done,
        };

        let (_, response) = only_200(route.fetch_client(session).await)?;
        if response.success {
            Ok(())
        } else {
            Err(FetchError::Unsuccessful)
        }
    }

    /// Delete this user-created reminder (`delete_reminder`), consuming it.
    ///
    /// On failure the item is handed back next to the error so the caller can
    /// restore it (e.g. undo an optimistic UI removal).
    ///
    /// # Panics
    /// If the item is not deletable — see [`is_deletable`](Self::is_deletable).
    pub async fn remove(self, session: &Session) -> Result<(), (Self, FetchError)> {
        assert!(
            self.is_deletable(),
            "dashboard item {} is not deletable",
            self.id
        );

        let route = DeleteReminderRoute { id: self.id };
        match only_200(route.fetch_client(session).await) {
            Ok((_, response)) if response.success => Ok(()),
            Ok(_) => Err((self, FetchError::Unsuccessful)),
            Err(err) => Err((self, err)),
        }
    }

    pub fn is_deletable(&self) -> bool {
        self.deletable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_the_dashboard_route_with_a_camel_case_key() {
        assert_eq!(
            serde_json::to_value(DashboardRoute { view_future: true }).unwrap(),
            serde_json::json!({ "viewFuture": true }),
        );
    }

    #[test]
    fn serializes_the_reminder_mutation_routes() {
        let save = SaveReminderRoute {
            date: NaiveDate::from_ymd_opt(2022, 10, 25).unwrap(),
            text: "Buy new pencils".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(save).unwrap(),
            serde_json::json!({ "date": "2022-10-25", "text": "Buy new pencils" }),
        );

        assert_eq!(
            serde_json::to_value(DeleteReminderRoute { id: 42 }).unwrap(),
            serde_json::json!({ "id": 42 }),
        );

        let toggle = ToggleReminderRoute {
            id: 42,
            kind: "homework".to_owned(),
            value: true,
        };
        assert_eq!(
            serde_json::to_value(toggle).unwrap(),
            serde_json::json!({ "id": 42, "type": "homework", "value": true }),
        );
    }

    #[test]
    fn deserializes_the_reminder_success_response() {
        let ok: SuccessResponse = serde_json::from_str(r#"{ "success": true }"#).unwrap();
        assert!(ok.success);
        let nok: SuccessResponse = serde_json::from_str(r#"{ "success": false }"#).unwrap();
        assert!(!nok.success);
    }

    #[test]
    fn deserializes_a_minimal_reminder_item() {
        // The shape `save_reminder` returns and that reminders take in the
        // dashboard array: only the eight always-present fields.
        let json = r#"{
            "id": 184, "type": "homework", "title": "Erinnerung",
            "subtitle": "test", "warning": false, "checkable": true,
            "checked": false, "deleteable": true
        }"#;

        let item: DashboardItem = serde_json::from_str(json).unwrap();

        assert_eq!(item.id, 184);
        assert_eq!(item.category, None);
        assert_eq!(item.label, None);
        assert_eq!(item.deadline, None);
        assert_eq!(item.homework, 0);
        assert!(item.is_deletable());
    }

    #[test]
    fn deserializes_a_school_item_with_a_null_deadline_start() {
        let json = r#"{
            "id": 127, "category": 127, "type": "gradeGroup", "title": "Hausaufgabe",
            "subtitle": "Übung", "label": "Technologien", "warning": false,
            "checkable": true, "checked": false, "online": 0, "submission": null,
            "deadline": "2026-09-15 08:40:00",
            "deadlineFormatted": "Dienstag, 15.09.2026, 08:40",
            "deadlineStart": null, "deadlineStartFormatted": null,
            "submissionAllowed": 0, "submissionResigned": false,
            "submissionIsNowInOvertime": false, "homework": 1,
            "done": null, "gradeGroupSubmissions": null
        }"#;

        let item: DashboardItem = serde_json::from_str(json).unwrap();

        assert_eq!(item.label.as_deref(), Some("Technologien"));
        assert_eq!(item.deadline_start_formatted, None);
        assert_eq!(item.deadline.as_deref(), Some("2026-09-15 08:40:00"));
        assert!(!item.is_deletable());
    }

    #[test]
    fn deserializes_the_bare_day_array() {
        // Trimmed from the `docs/API.md` captured example: a bare array, one
        // empty day and one day with a single item.
        let json = r#"[
            { "items": [], "date": "2022-10-19" },
            {
                "date": "2022-10-20",
                "items": [
                    {
                        "id": 1338, "category": 1338, "type": "gradeGroup",
                        "title": "Hausaufgabe Schriftlich", "subtitle": "Completare gli esercizi",
                        "label": "Italienisch", "warning": false, "checkable": true,
                        "checked": false, "online": 0, "submission": null,
                        "deadline": "2022-10-20 07:45:00",
                        "deadlineFormatted": "Donnerstag, 20.10.2022, 07:45",
                        "deadlineStart": null,
                        "deadlineStartFormatted": "Donnerstag, 01.01.1970, 01:00",
                        "submissionAllowed": 0, "submissionResigned": false,
                        "submissionIsNowInOvertime": false, "homework": 1,
                        "done": null, "gradeGroupSubmissions": null
                    }
                ]
            }
        ]"#;

        let response: DashboardResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.len(), 2);
        assert!(response[0].items.is_empty());
        assert_eq!(response[1].items[0].id, 1338);
        assert!(!response[1].items[0].is_deletable(), "school items lack `deleteable`");
    }
}