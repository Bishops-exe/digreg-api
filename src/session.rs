use crate::School;
use crate::routes::absences_route::{AbsencesResponse, AbsencesRoute};
use crate::routes::calendar_route::{CalendarResponse, CalendarRoute};
use crate::routes::dashboard_route::{
    DashboardItem, DashboardResponse, DashboardRoute, SaveReminderRoute,
};
use crate::routes::extend_session::{ExtendSessionResponse, ExtendSessionRoute};
use crate::routes::grades_route::{AllSubjectsResponse, AllSubjectsRoute};
use crate::routes::message_route::{MyMessagesResponse, MyMessagesRoute};
use crate::routes::notification_route::{
    MarkNotificationsReadRoute, UnreadNotificationsResponse, UnreadNotificationsRoute,
};
use crate::routes::{FetchError, Route};
use crate::utils::only_200;
use chrono::{Datelike, Days, NaiveDate, Utc};
use reqwest::Client;

pub struct Session {
    pub(crate) client: Client,
    pub(crate) school: School,
}

impl Session {
    pub(crate) fn from(client: Client, school: School) -> Self {
        Self { client, school }
    }

    /// Keep the session alive by telling the server the user is still active.
    pub async fn extend(&self) -> Result<ExtendSessionResponse, FetchError> {
        let route = ExtendSessionRoute {
            last_action: Utc::now(),
        };
        let (_, _, response) = route.fetch_client(self).await?;
        Ok(response)
    }

    pub async fn dashboard(&self, view_future: bool) -> Result<DashboardResponse, FetchError> {
        let route = DashboardRoute { view_future };
        only_200(route.fetch_client(self).await).map(|x| x.1)
    }
    
    #[inline]
    pub async fn dashboard_future(&self) -> Result<DashboardResponse, FetchError> {
        self.dashboard(true).await
    }
    
    #[inline]
    pub async fn dashboard_past(&self) -> Result<DashboardResponse, FetchError> {
        self.dashboard(false).await
    }

    /// Fetch the timetable for the week containing `day` (the request is
    /// snapped back to that week's Monday, as the server requires).
    pub async fn calendar(&self, day: NaiveDate) -> Result<CalendarResponse, FetchError> {
        let monday = day - Days::new(day.weekday().num_days_from_monday() as u64);
        let route = CalendarRoute { start_date: monday };
        let (_, _, response) = route.fetch_client(self).await?;
        Ok(response)
    }

    /// Fetch the timetable for the current week.
    #[inline]
    pub async fn calendar_this_week(&self) -> Result<CalendarResponse, FetchError> {
        self.calendar(Utc::now().date_naive()).await
    }

    /// Create a user-authored reminder on `date` (`save_reminder`). Returns
    /// the newly created dashboard item.
    pub async fn push_reminder(
        &self,
        date: NaiveDate,
        content: &str,
    ) -> Result<DashboardItem, FetchError> {
        let route = SaveReminderRoute {
            date,
            text: content.to_owned(),
        };
        only_200(route.fetch_client(self).await).map(|x| x.1)
    }

    /// Fetch every subject with its grade summary for the semester the session
    /// is currently switched to.
    ///
    /// `student_id` is the student's user id (from the session config). There
    /// is no semester parameter — the result covers whichever semester the
    /// session was last switched to server-side.
    pub async fn all_subjects(
        &self,
        student_id: i64,
    ) -> Result<AllSubjectsResponse, FetchError> {
        let route = AllSubjectsRoute { student_id };
        only_200(route.fetch_client(self).await).map(|x| x.1)
    }

    /// Fetch every message addressed to the user (`getMyMessages`). Mark one
    /// read with [`Message::mark_read`](crate::routes::message_route::Message::mark_read).
    pub async fn messages(&self) -> Result<MyMessagesResponse, FetchError> {
        let route = MyMessagesRoute {};
        only_200(route.fetch_client(self).await).map(|x| x.1)
    }

    /// Fetch the student's absences, upcoming absences and absence statistics
    /// (`absences`).
    pub async fn absences(&self) -> Result<AbsencesResponse, FetchError> {
        let route = AbsencesRoute {};
        only_200(route.fetch_client(self).await).map(|x| x.1)
    }

    /// Fetch the user's unread notifications (`notification/unread`). Mark one
    /// read with [`Notification::mark_read`](crate::routes::notification_route::Notification::mark_read).
    pub async fn unread_notifications(
        &self,
    ) -> Result<UnreadNotificationsResponse, FetchError> {
        let route = UnreadNotificationsRoute {};
        only_200(route.fetch_client(self).await).map(|x| x.1)
    }

    /// Mark every notification read (`notification/markAsRead` with an empty
    /// body).
    pub async fn mark_all_notifications_read(&self) -> Result<(), FetchError> {
        let route = MarkNotificationsReadRoute { id: None };
        only_200(route.fetch_client(self).await).map(|_| ())
    }
}
