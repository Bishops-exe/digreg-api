use chrono::NaiveDateTime;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::routes::{FetchError, Route};
use crate::utils::only_200;
use crate::Session;

/// `POST api/notification/unread` — the user's unread notifications. Empty
/// request body.
#[derive(Serialize)]
pub struct UnreadNotificationsRoute {}

impl Route for UnreadNotificationsRoute {
    type Response = UnreadNotificationsResponse;

    fn get_route() -> &'static str {
        "api/notification/unread"
    }
}

/// `POST api/notification/markAsRead` — mark one notification read (`id` set)
/// or all of them (`id` omitted, i.e. an empty `{}` body). The response is not
/// used.
#[derive(Serialize)]
pub struct MarkNotificationsReadRoute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<i64>,
}

impl Route for MarkNotificationsReadRoute {
    type Response = IgnoredAny;

    fn get_route() -> &'static str {
        "api/notification/markAsRead"
    }
}

/// A bare JSON array of notifications. Derefs to `[Notification]`.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct UnreadNotificationsResponse(pub Vec<Notification>);

impl std::ops::Deref for UnreadNotificationsResponse {
    type Target = [Notification];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: i64,
    pub title: String,
    /// Note the capital `T` on the wire (`subTitle`) — unlike the dashboard
    /// item's lowercase `subtitle`.
    pub sub_title: String,
    /// Free-form; only `"message"` is treated specially (see [`message_id`]).
    ///
    /// [`message_id`]: Notification::message_id
    #[serde(rename = "type")]
    pub kind: String,
    /// Id of the object this notification points at; its meaning depends on
    /// `kind`.
    #[serde(default)]
    pub object_id: Option<i64>,
    pub time_sent: NaiveDateTime,
}

impl Notification {
    /// When this notification is about a message, that message's id.
    pub fn message_id(&self) -> Option<i64> {
        if self.kind == "message" {
            self.object_id
        } else {
            None
        }
    }

    /// Mark just this notification read (`markAsRead` with `{ id }`).
    pub async fn mark_read(&self, session: &Session) -> Result<(), FetchError> {
        let route = MarkNotificationsReadRoute { id: Some(self.id) };
        only_200(route.fetch_client(session).await).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_the_notification_routes() {
        assert_eq!(
            serde_json::to_value(UnreadNotificationsRoute {}).unwrap(),
            serde_json::json!({}),
        );
        assert_eq!(
            serde_json::to_value(MarkNotificationsReadRoute { id: Some(501) }).unwrap(),
            serde_json::json!({ "id": 501 }),
        );
        // "mark all" is the same endpoint with an empty body.
        assert_eq!(
            serde_json::to_value(MarkNotificationsReadRoute { id: None }).unwrap(),
            serde_json::json!({}),
        );
    }

    #[test]
    fn deserializes_notifications_and_resolves_message_id() {
        let json = r#"[
            {
                "id": 501, "title": "Neue Hausaufgabe", "subTitle": "Italienisch",
                "type": "message", "objectId": 1234, "timeSent": "2022-10-19T08:00:00"
            },
            {
                "id": 502, "title": "Etwas anderes", "subTitle": "",
                "type": "grade", "objectId": 9, "timeSent": "2022-10-19T09:00:00"
            }
        ]"#;

        let notifications: UnreadNotificationsResponse = serde_json::from_str(json).unwrap();

        assert_eq!(notifications.len(), 2);
        assert_eq!(notifications[0].sub_title, "Italienisch");
        assert_eq!(notifications[0].message_id(), Some(1234));
        assert_eq!(notifications[1].message_id(), None);
    }
}