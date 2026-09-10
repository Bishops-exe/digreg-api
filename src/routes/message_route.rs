use chrono::NaiveDateTime;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::routes::{FetchError, Route};
use crate::utils::only_200;
use crate::Session;

/// `POST api/message/getMyMessages` — every message addressed to the user.
/// Empty request body.
#[derive(Serialize)]
pub struct MyMessagesRoute {}

impl Route for MyMessagesRoute {
    type Response = MyMessagesResponse;

    fn get_route() -> &'static str {
        "api/message/getMyMessages"
    }
}

/// `POST api/message/markAsRead` — mark one message read. The response is not
/// used; the official client marks the message read locally without trusting
/// any server-echoed value.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkMessageReadRoute {
    pub(crate) message_id: i64,
}

impl Route for MarkMessageReadRoute {
    type Response = IgnoredAny;

    fn get_route() -> &'static str {
        "api/message/markAsRead"
    }
}

/// A bare JSON array of messages. Derefs to `[Message]`.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct MyMessagesResponse(pub Vec<Message>);

impl std::ops::Deref for MyMessagesResponse {
    type Target = [Message];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    pub subject: String,
    pub text: String,
    pub time_sent: NaiveDateTime,
    /// `None` while unread. When set it is a server timestamp, but the client
    /// only ever writes this field locally.
    pub time_read: Option<NaiveDateTime>,
    pub recipient_string: String,
    pub from_name: String,
    #[serde(default)]
    pub submissions: Vec<MessageSubmission>,
}

impl Message {
    pub fn is_unread(&self) -> bool {
        self.time_read.is_none()
    }

    /// Mark this message read on the server (`markAsRead`).
    ///
    /// The local `time_read` field is left untouched; the official client
    /// tracks read-state locally rather than trusting a server echo.
    pub async fn mark_read(&self, session: &Session) -> Result<(), FetchError> {
        let route = MarkMessageReadRoute { message_id: self.id };
        only_200(route.fetch_client(session).await).map(|_| ())
    }

    /// The attachments the official client would actually surface: `type` is
    /// `"file"`, `isDownloadable` is set, and every id/name field is present.
    pub fn downloadable_files(&self) -> impl Iterator<Item = &MessageSubmission> {
        self.submissions
            .iter()
            .filter(|submission| submission.is_downloadable_file())
    }
}

/// An attachment on a [`Message`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSubmission {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub is_downloadable: bool,
    pub id: Option<i64>,
    pub message_id: Option<i64>,
    pub original_name: Option<String>,
    pub file: Option<String>,
}

impl MessageSubmission {
    /// Whether this entry is a downloadable file with all of `id`,
    /// `messageId`, `originalName` and `file` present (the client drops it
    /// otherwise).
    pub fn is_downloadable_file(&self) -> bool {
        self.kind == "file"
            && self.is_downloadable
            && self.id.is_some()
            && self.message_id.is_some()
            && self.original_name.is_some()
            && self.file.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_the_message_routes() {
        assert_eq!(
            serde_json::to_value(MyMessagesRoute {}).unwrap(),
            serde_json::json!({}),
        );
        assert_eq!(
            serde_json::to_value(MarkMessageReadRoute { message_id: 88 }).unwrap(),
            serde_json::json!({ "messageId": 88 }),
        );
    }

    #[test]
    fn a_submission_is_only_a_downloadable_file_when_every_field_is_present() {
        let mut submission = MessageSubmission {
            kind: "file".to_owned(),
            is_downloadable: true,
            id: Some(1),
            message_id: Some(2),
            original_name: Some("a.pdf".to_owned()),
            file: Some("stored.pdf".to_owned()),
        };
        assert!(submission.is_downloadable_file());

        submission.is_downloadable = false;
        assert!(!submission.is_downloadable_file());

        submission.is_downloadable = true;
        submission.kind = "text".to_owned();
        assert!(!submission.is_downloadable_file());

        submission.kind = "file".to_owned();
        submission.file = None;
        assert!(!submission.is_downloadable_file());
    }

    #[test]
    fn deserializes_a_message_with_attachment() {
        let json = r#"[
            {
                "id": 88,
                "subject": "Elternsprechtag",
                "text": "Der Elternsprechtag findet am ... statt.",
                "timeSent": "2022-10-05T09:00:00",
                "timeRead": null,
                "recipientString": "Alle Schüler der 8K",
                "fromName": "Sekretariat",
                "submissions": [
                    {
                        "type": "file", "isDownloadable": true, "id": 12,
                        "messageId": 88, "originalName": "einladung.pdf",
                        "file": "abcd1234.pdf"
                    },
                    { "type": "text", "isDownloadable": false, "id": null,
                      "messageId": null, "originalName": null, "file": null }
                ]
            }
        ]"#;

        let messages: MyMessagesResponse = serde_json::from_str(json).unwrap();

        assert_eq!(messages.len(), 1);
        assert!(messages[0].is_unread());
        assert_eq!(messages[0].downloadable_files().count(), 1);
        assert_eq!(
            messages[0].time_sent,
            "2022-10-05T09:00:00".parse::<NaiveDateTime>().unwrap()
        );
    }
}