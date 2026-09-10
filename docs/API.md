# Digitales Register — Backend API Reference

This document describes the HTTP API that this Flutter app (everything under
`lib/`) speaks to. It is reverse-engineered from the app's own networking code —
there is no official API documentation from the school-register backend
(`digitalesregister.it`, the "Vinzentinum" web register, likely a PHP app). The
goal of this document is to contain everything needed to write an independent
API client/wrapper without reading the Dart source.

Everything below was extracted from:

- `lib/wrapper.dart` — the HTTP client (`Wrapper` class), login/session handling
- `lib/middleware/*.dart` — every call site that hits the network
  (`wrapper.send(...)`, direct `dio` downloads)
- `lib/reducer/*.dart` — the code that parses each JSON response into app
  models (this is the most reliable source for exact field names/types,
  since it's the code that actually consumes the server's response)
- `lib/data.dart`, `lib/app_state.dart` — the resulting data model shapes
- `lib/demo.dart` — canned example responses used by the app's built-in
  "demo account" (offline showcase mode). These are **real captured server
  responses** that were hard-coded for demo purposes, so they are used below
  as authentic examples wherever available.

> ⚠️ This is not an officially documented/stable API. Field names, presence,
> and behavior were derived by reading how the client happens to use the data
> today, and the backend can change without notice. Treat unknown/extra
> fields in responses as normal — the client itself only reads a subset of
> what's returned.

## Table of contents

- [Base URL & transport](#base-url--transport)
- [Client setup requirements](#client-setup-requirements)
- [Authentication](#authentication)
  - [Login](#login)
  - [Two-factor authentication](#two-factor-authentication)
  - [Session config (HTML scrape)](#session-config-html-scrape)
  - [Keeping the session alive](#keeping-the-session-alive)
  - [Logout](#logout)
  - [Change password (logged in)](#change-password-logged-in)
  - [Forgot password flow (logged out)](#forgot-password-flow-logged-out)
- [Generic request contract](#generic-request-contract)
- [Unexpected-logout detection & retry](#unexpected-logout-detection--retry)
- [Endpoints](#endpoints)
  - [Dashboard](#dashboard)
  - [Homework / reminders](#homework--reminders)
  - [Absences](#absences)
  - [Calendar](#calendar)
  - [Semester switch](#semester-switch)
  - [Grades](#grades)
  - [Certificate](#certificate)
  - [Notifications](#notifications)
  - [Messages](#messages)
  - [Profile](#profile)
  - [File downloads](#file-downloads)
- [Connectivity check](#connectivity-check)
- [Known quirks & undocumented behavior](#known-quirks--undocumented-behavior)

## Base URL & transport

The user enters a school-specific base URL at login (e.g.
`https://vinzentinum.digitalesregister.it`), or the app was already
configured with one (`AppState.url` / `wrapper.url`). Every backend, one per
school, hosts the same software.

Two derived URLs are used everywhere (`lib/wrapper.dart:65-66`):

```dart
String get baseAddress => "$url/v2/";
String get loginAddress => "${baseAddress}api/auth/login";
```

So if the user enters `https://vinzentinum.digitalesregister.it`:

- `baseAddress` = `https://vinzentinum.digitalesregister.it/v2/`
- `loginAddress` = `https://vinzentinum.digitalesregister.it/v2/api/auth/login`

**All app-level endpoints below are relative to `baseAddress`** (i.e. they
sit under `/v2/`), **except** the "forgot password" endpoints, which
deliberately live directly under the root URL (no `/v2/`) — see
[Forgot password flow](#forgot-password-flow-logged-out).

### URL normalization (`fixupUrl`, `lib/util.dart:221-234`)

Whatever the user types into the "server" field is normalized like this
before use:

```dart
String fixupUrl(String enteredUrl) {
  var url = enteredUrl;
  if (Uri.parse(url).scheme.isEmpty) {
    // add https:// if there is no uri scheme
    url = "https://$url";
  }
  const defaultUrlPath = "v2/login";
  if (url.endsWith(defaultUrlPath)) {
    // /v2/login is the default path the browser is redirected to when loading
    // the web app. Users might just copy-paste that url, so let's strip it.
    url = url.substring(0, url.length - defaultUrlPath.length);
  }
  return url;
}
```

So `vinzentinum.digitalesregister.it/v2/login` and
`https://vinzentinum.digitalesregister.it` both normalize to
`https://vinzentinum.digitalesregister.it`.

### Transport details

- All API traffic uses **HTTP(S) with cookie-based session auth** — there is
  no bearer token/API key. A cookie jar must be attached to the HTTP client
  and persisted across requests (`dio` + `dio_cookie_manager` +
  `cookie_jar` in the app). On every explicit logout and whenever the target
  server `url` changes, the app clears all cookies
  (`wrapper.dart:546-548`, `_clearCookies()`).
- Request bodies for `POST` requests are sent as **form-style key/value maps**
  via `dio`'s `data:` parameter (dio defaults to `application/json` request
  encoding when given a `Map`, i.e. requests are JSON-encoded — see note
  below).
- The client sets a custom `User-Agent` header (`wrapper.dart:74-76`):
  ```
  Digitales-Register-App <appVersion>; https://github.com/miDeb/digitales_register
  ```
  This is not required by the server (as far as is known) but is good
  etiquette to replicate.
- Response bodies are read as-is; most endpoints return JSON (as an object
  or array), a couple return raw HTML, and one (`api/student/subject_detail`)
  sometimes returns a **JSON string containing JSON** (double-encoded — see
  [quirks](#known-quirks--undocumented-behavior)).

## Client setup requirements

To build a minimal wrapper you need:

1. An HTTP client that automatically stores and resends cookies
   (session-based auth — there's no token to manage yourself).
2. A place to persist the last successful `(username, password, base url)`
   so you can silently re-login (the app does this via `flutter_secure_storage`,
   see `lib/middleware/pass.dart`).
3. A `POST /v2/api/auth/login` call to establish a session (see below).
4. Logic to detect a session that has silently expired
   (see [unexpected-logout detection](#unexpected-logout-detection--retry)),
   since the backend does not respond with an HTTP 401/403 for this — it
   returns HTTP 200 with a body that is actually a client-side redirect
   script meant for a browser.

## Authentication

### Login

```
POST {baseUrl}/v2/api/auth/login
Content-Type: application/json
```

Request body:

```json
{
  "username": "mmustermann",
  "password": "hunter2",
  "two_factor": "123456"
}
```

```js
const result = await client.login("mmustermann", "hunter2");
// retry with a code once the server asks for one:
// await client.login("mmustermann", "hunter2", "123456");
```

- `username`, `password` — required, strings.
- `two_factor` — **omit this key entirely** on the first attempt. Only
  include it on a follow-up request once the server has told you a 2FA code
  is required (see below). (`lib/wrapper.dart:167-173`)

Response body (`200 OK`, JSON object), success case:

```json
{
  "loggedIn": true
}
```

On success, the server also sets a session cookie (via `Set-Cookie`) which
must be stored and sent on subsequent requests.

Response body, failure case:

```json
{
  "loggedIn": false,
  "error": "wrong_credentials",
  "message": "Benutzername oder Passwort falsch."
}
```

`error` is a machine-readable error code; `message` is a human-readable
(German) message meant for display. Known `error` values observed/handled
by the client (`lib/middleware/login.dart:136`,
`lib/wrapper.dart:203-216`):

| `error` value | Meaning | Client behavior |
|---|---|---|
| `password_expired` | Login credentials are correct but the password must be changed before use | App forces the user into the "change password" flow (`api/auth/setNewPassword`, see below) instead of treating this as a failed login |
| `two_factor_needed` | Correct credentials, but the account has 2FA enabled and no code was sent | App prompts for a code, then retries the exact same login request with `two_factor` set |
| `two_factor_wrong` | Correct credentials, but the submitted `two_factor` code was wrong | App prompts again |
| *(anything else)* | Generic failure | `message` is shown to the user as-is |

A client wrapper should treat `loggedIn: false` + `error` in
`{two_factor_needed, two_factor_wrong}` as "retry with a 2FA code", and
anything else as a hard failure.

### Two-factor authentication

There is no separate 2FA endpoint — 2FA is handled by resubmitting the same
`POST /v2/api/auth/login` request with an added `two_factor` field (the code
the user enters), as shown above.

### Session config (HTML scrape)

⚠️ **This is not a JSON API call.** After a successful login, the app issues:

```
GET {baseUrl}/v2/
```

```js
const config = await client.getConfig();
// { userId, autoLogoutSeconds, isStudentOrParent, currentSemesterMaybe }
```

and receives back a full **HTML page** (the web app's own front-end shell).
It then scrapes several values it needs out of embedded `<script>` content
via substring search (`lib/wrapper.dart:284-354`). There is no JSON endpoint
for this data as far as the client code shows. A wrapper wanting this data
must replicate the same string scraping (or find/attempt an equivalent
undocumented JSON endpoint of its own).

Extracted fields, and exactly how they're located in the HTML source
(`source` below is the full HTML text of `GET {baseUrl}/v2/`):

| Field | How it's extracted | Type | Notes |
|---|---|---|---|
| `userId` | Substring right after literal text `currentUserId=`, up to the next `;` | `int` | |
| `autoLogoutSeconds` | Substring right after literal text `auto_logout_seconds: `, up to the next `,` | `int` | How many seconds until the *server* will expire the session |
| `fullName` | Text content of the DOM node found right after the substring `navigationProfilePicture` (between the first `>` and next `<` following it) | `String` | The logged-in user's / student's display name |
| `imgSource` | The `src="..."` attribute value found near `navigationProfilePicture` (same neighborhood as above) | `String` | Absolute/relative URL of the profile picture, e.g. `.../v2/theme/icons/profile_empty.png` |
| `currentSemesterMaybe` | If the HTML contains the literal substring `semesterWechsel=1` → `2`. Else if it contains `semesterWechsel=2` → `1`. Else → `null` | `int?` | **Note the inversion**: presence of `...=1` in the page means the *current* semester is `2` (i.e. that string is the link to switch **to** the other semester) |
| `isStudentOrParent` | `true` unless the HTML contains the literal substring `var isStudentOrParent=0;` | `bool` | The app **refuses to support any other account type** (e.g. teacher accounts) — see below |

Relevant source (`lib/wrapper.dart:289-354`):

```dart
static Config parseConfig(String source) {
  final id = _readUserId(source);
  final fullName = _readFullName(source);
  final imgSource = _readImgSource(source);
  final autoLogout = _readAutoLogoutSeconds(source);
  final currentSemesterMaybe = _readCurrentSemester(source);
  final isStudentOrParent = _readIsStudentOrParent(source);
  return Config((b) => b
    ..userId = id
    ..autoLogoutSeconds = autoLogout
    ..fullName = fullName
    ..imgSource = imgSource
    ..currentSemesterMaybe = currentSemesterMaybe
    ..isStudentOrParent = isStudentOrParent);
}
```

**Important:** after login, the app checks `isStudentOrParent`. If it's
`false` it immediately logs out again and shows an "unsupported account
type" screen — the app (and by extension this API usage) is only exercised
for **student/parent accounts**. Teacher accounts may behave differently
server-side and are unverified.

### Keeping the session alive

The server tells the client, via `autoLogoutSeconds` (above), how long the
session will live from the moment of login. The app proactively extends the
session shortly before that:

```
POST {baseUrl}/v2/api/auth/extendSession
Content-Type: application/json
```

Called ~25 seconds before the computed expiry, and rescheduled to check
again every 5 seconds (`lib/wrapper.dart:497-526`).

Request body:

```json
{
  "lastAction": 1700000000
}
```

```js
const result = await client.extendSession(Math.floor(Date.now() / 1000));
// { forceLogout, newExpiration }
```

`lastAction` = Unix timestamp (**seconds**, not ms) of the user's last
interaction with the app (tap, navigation, etc.) — used by the server to
decide whether to actually extend or to log the session out due to
inactivity.

Response body:

```json
{
  "forceLogout": false,
  "newExpiration": 1700000600
}
```

- `forceLogout: true` → server has decided to end the session anyway (e.g.
  inactivity); client should treat this as logged out.
- `newExpiration` — new Unix timestamp (**milliseconds** — the client does
  `DateTime.fromMillisecondsSinceEpoch((result["newExpiration"] as int) * 1000)`,
  i.e. it multiplies a *seconds* value by 1000, so `newExpiration` itself is
  in **seconds**, same unit as `lastAction`) after which the session expires
  if not extended again.
- If the request throws/fails outright, the client treats that the same as
  `forceLogout: true`.

### Logout

```
GET {baseUrl}/v2/logout
```

```js
await client.logout();
```

Fire-and-forget — the app does not read/await the response body meaningfully,
it just informs the server. This is only called when the logout was
initiated by the client itself (not when the client merely detected that the
server had already ended the session — see `logoutForcedByServer` in
`lib/wrapper.dart:532-535`). Client-side, "logging out" also means: clear
cookies, forget in-memory `user`/`pass`, and (depending on user settings)
erase locally cached data / saved credentials.

### Change password (logged in)

Used both for the ordinary "change my password" settings screen, and to
satisfy a `password_expired` login response.

```
POST {baseUrl}/v2/api/auth/setNewPassword
Content-Type: application/json
```

Request body:

```json
{
  "username": "mmustermann",
  "oldPassword": "hunter2",
  "newPassword": "newHunter3"
}
```

```js
const result = await client.changePassword("mmustermann", "hunter2", "newHunter3");
```

Response body, success:

```json
{}
```

(no `error` key — client just proceeds; it does not require any particular
success payload, just the absence of `"error"`)

Response body, failure:

```json
{
  "error": "wrong_password",
  "message": "Altes Passwort ist falsch."
}
```

### Forgot password flow (logged out)

⚠️ **These two endpoints intentionally sit at the server root, *not* under
`/v2/`** — this is called out explicitly in the source
(`lib/middleware/login.dart:230, 261`: *"the api url DOES NOT contain /v2/ in
the path. This is intentional."*).

**Step 1 — request a reset email:**

```
POST {baseUrl}/api/auth/resetPassword
Content-Type: application/json
```

Request body:

```json
{
  "email": "student@example.com",
  "username": "mmustermann"
}
```

```js
const result = await client.requestPasswordReset("student@example.com", "mmustermann");
```

Response body, success:

```json
{
  "message": "Eine E-Mail wurde versendet."
}
```

Response body, failure:

```json
{
  "error": "user_not_found",
  "message": "Kein Benutzer gefunden."
}
```

The user then receives an email containing a link with `resetmail=true`,
`email=...` and `token=...` query parameters pointing back into the app/web
UI (see [deep links](#known-quirks--undocumented-behavior)).

**Step 2 — submit the new password using the emailed token:**

```
POST {baseUrl}/api/auth/setNewPassword
Content-Type: application/json
```

Request body:

```json
{
  "username": "",
  "token": "<token from the reset email link>",
  "email": "student@example.com",
  "oldPassword": "",
  "newPassword": "newHunter3"
}
```

```js
const result = await client.setNewPasswordWithToken({
  token: "<token from the reset email link>",
  email: "student@example.com",
  newPassword: "newHunter3",
});
```

Note that `username` and `oldPassword` are sent as **empty strings** here
(deliberately — this is the *unauthenticated* reset path, identity is proven
via `token` + `email` instead), which is the opposite of the "change
password while logged in" call above.

Response shape (success/failure) is identical in structure to step 1
(`{"message": "..."}` or `{"error": "...", "message": "..."}`).

## Generic request contract

Every "normal" API call (everything except login, logout, password
reset/change, session-extend, and file downloads) goes through one function,
`Wrapper.send` (`lib/wrapper.dart:412-483`), which is worth understanding
because it defines conventions that apply to *every* endpoint listed further
below:

```dart
Future<dynamic> send(
  String url, {
  Map<String, Object?> args = const <String, Object?>{},
  String method = "POST",
  bool isRetryAfterUnexpectedLogout = false,
}) async {
  ...
  final response = await (method == "POST"
      ? dio.post<dynamic>(baseAddress + url, data: args)
      : method == "GET"
          ? dio.get<dynamic>(baseAddress + url)
          : throw Exception("invalid method: $method; expected POST or GET"));
  ...
}
```

Key conventions:

- `url` is always relative to `baseAddress` (`{baseUrl}/v2/`) and must
  **not** start with `/` (there's a hard `assert` for this).
- **`POST` is the default method.** Only two endpoints in the whole app use
  `GET` through this function: `student/certificate` and the semester-switch
  pseudo-endpoint (`?semesterWechsel=N`).
- For `POST` requests, `args` is sent as the request body (`data:`), a plain
  JSON object — omitted keys are simply not sent (no `null` padding).
- For `GET` requests through `send`, **no query parameters or body are ever
  sent** by this codebase (all current `GET` call sites pass no `args`) —
  though the underlying server may well accept query params for `GET` in
  general; it's just not exercised here.
- Before every call, the client ensures it's logged in
  (`ensureLoggedIn()`), transparently logging back in with the last known
  credentials if the in-memory session looks stale. If that silent re-login
  fails, `send` returns `null` without making the request at all.
- If the request throws (network error, etc.), `send` returns `null` after
  recording the error/no-internet state — **callers must treat a `null`
  response as "request failed", not as a legitimate empty response.**

## Unexpected-logout detection & retry

The backend does not use HTTP status codes to signal "your session expired."
Instead, when a request is made with an invalid/expired session, the server
responds with **HTTP 200** and a body that is literally a snippet of
JavaScript meant to run in a browser and redirect it to the login page:

```html
<script type="text/javascript">
window.location = "https://vinzentinum.digitalesregister.it/v2/login";
</script>
```

The client detects this by matching every response body against this regex
(`lib/wrapper.dart:462-464`):

```
^[\s\n]*<script type="text/javascript">\n?\s*window\.location = "https://.+\.digitalesregister.it/v2/login";\n?\s*</script>[\s\n]*$
```

When matched:

1. If this is already a retry (`isRetryAfterUnexpectedLogout == true`), give
   up and throw an `UnexpectedLogoutException`.
2. Otherwise, transparently attempt to log back in
   (`ensureLoggedIn(isRetryAfterUnexpectedLogout: true)`) and retry the exact
   same request once, with `isRetryAfterUnexpectedLogout: true`.

A wrapper implementation **must** replicate this check on every response
(for `POST`/`GET` calls under `/v2/`, not just some) — otherwise a session
expiry will silently look like "the endpoint returned a script tag as data"
instead of being handled.

There's a built-in cooldown: the app will not attempt more than one silent
re-login-and-retry per **60 seconds**, to avoid infinite loops if something
is structurally broken (`lib/wrapper.dart:369-384`).

## JavaScript reference client

Every JS snippet in this document calls into the small client below, which
mirrors `Wrapper` (`lib/wrapper.dart`) 1:1: `send()` implements the generic
request contract (JSON body, `/v2/`-relative path, unexpected-logout
detect-and-retry), and the auth/download methods wrap the endpoints described
above.

> ⚠️ **Cookies:** `fetch`'s `credentials: "include"` only persists cookies
> automatically in a browser. In Node.js, `fetch` does **not** keep a cookie
> jar between calls by itself — wrap it with something like
> [`fetch-cookie`](https://www.npmjs.com/package/fetch-cookie) (e.g.
> `fetch = fetchCookie(fetch, new CookieJar())`) or these examples will
> silently fail to stay logged in after the first request.

```js
class DigitalesRegisterClient {
  constructor(baseUrl) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.v2 = `${this.baseUrl}/v2/`;
    this.username = null;
    this.password = null;
  }

  // Generic request contract, mirrors Wrapper.send() (lib/wrapper.dart:412-483).
  async send(path, { args = {}, method = "POST", isRetry = false } = {}) {
    const res = await fetch(this.v2 + path, {
      method,
      credentials: "include",
      headers: method === "POST" ? { "Content-Type": "application/json" } : {},
      body: method === "POST" ? JSON.stringify(args) : undefined,
    });
    const text = await res.text();

    // The backend signals an expired session with HTTP 200 + a JS redirect
    // snippet instead of a real 401 — see "Unexpected-logout detection & retry".
    const loggedOutRe =
      /^[\s\n]*<script type="text\/javascript">\n?\s*window\.location = "https:\/\/.+\.digitalesregister\.it\/v2\/login";\n?\s*<\/script>[\s\n]*$/;
    if (loggedOutRe.test(text)) {
      if (isRetry) throw new Error("UnexpectedLogoutException");
      await this.login(this.username, this.password);
      return this.send(path, { args, method, isRetry: true });
    }

    try {
      return JSON.parse(text);
    } catch {
      return text; // e.g. student/certificate returns raw HTML, not JSON
    }
  }

  // POST {baseUrl}/v2/api/auth/login
  async login(username, password, twoFactor) {
    const body = { username, password };
    if (twoFactor) body.two_factor = twoFactor;
    const res = await fetch(`${this.v2}api/auth/login`, {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    const result = await res.json(); // { loggedIn, error?, message? }
    if (result.loggedIn) {
      this.username = username;
      this.password = password;
    }
    return result;
  }

  // GET {baseUrl}/v2/ then scrape the HTML shell — see "Session config (HTML scrape)".
  async getConfig() {
    const res = await fetch(this.v2, { credentials: "include" });
    const html = await res.text();
    return {
      userId: Number(html.split("currentUserId=")[1].split(";")[0].trim()),
      autoLogoutSeconds: Number(
        html.split("auto_logout_seconds: ")[1].split(",")[0].trim(),
      ),
      isStudentOrParent: !html.includes("var isStudentOrParent=0;"),
      currentSemesterMaybe: html.includes("semesterWechsel=1")
        ? 2
        : html.includes("semesterWechsel=2")
          ? 1
          : null,
    };
  }

  // POST {baseUrl}/v2/api/auth/extendSession
  extendSession(lastActionUnixSeconds) {
    return this.send("api/auth/extendSession", {
      args: { lastAction: lastActionUnixSeconds },
    });
  }

  // GET {baseUrl}/v2/logout
  async logout() {
    await fetch(`${this.v2}logout`, { credentials: "include" });
    this.username = this.password = null;
  }

  // POST {baseUrl}/v2/api/auth/setNewPassword (while logged in)
  changePassword(username, oldPassword, newPassword) {
    return this.send("api/auth/setNewPassword", {
      args: { username, oldPassword, newPassword },
    });
  }

  // POST {baseUrl}/api/auth/resetPassword — note: NOT under /v2/, see quirk #6.
  async requestPasswordReset(email, username) {
    const res = await fetch(`${this.baseUrl}/api/auth/resetPassword`, {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, username }),
    });
    return res.json(); // { message } or { error, message }
  }

  // POST {baseUrl}/api/auth/setNewPassword — also NOT under /v2/.
  async setNewPasswordWithToken({ token, email, newPassword }) {
    const res = await fetch(`${this.baseUrl}/api/auth/setNewPassword`, {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        username: "",
        oldPassword: "",
        token,
        email,
        newPassword,
      }),
    });
    return res.json();
  }

  // GET {baseUrl}/v2/<path>?<params> — for the three file-download endpoints.
  async download(path, params) {
    const url = new URL(this.v2 + path);
    url.search = new URLSearchParams(params).toString();
    const res = await fetch(url, { credentials: "include" });
    if (!res.ok) throw new Error(`download failed: HTTP ${res.status}`);
    return res.blob(); // use res.arrayBuffer() in Node
  }
}

const client = new DigitalesRegisterClient(
  "https://vinzentinum.digitalesregister.it",
);
```

## Endpoints

All paths below are relative to `{baseUrl}/v2/` unless stated otherwise.
Method is `POST` unless stated otherwise. Request bodies shown are the exact
`args` maps the client sends; response bodies are either taken verbatim from
`lib/demo.dart` (marked **"real captured example"**) or reconstructed field-
by-field from the parsing code in `lib/reducer/*.dart` (marked
**"reconstructed from parsing code"** — field names/types are accurate, but
the exact sample values are illustrative).

### Dashboard

**`POST api/student/dashboard/dashboard`**

Request body:

```json
{ "viewFuture": false }
```

```js
const days = await client.send("api/student/dashboard/dashboard", {
  args: { viewFuture: false },
});
```

`viewFuture` (`bool`) — `false` = show past/current items ("Was ist fällig"),
`true` = show upcoming items.

Response: JSON array of "day" objects. **Real captured example**
(`lib/demo.dart:25-861`, trimmed):

```json
[
  { "items": [], "date": "2022-10-19" },
  {
    "date": "2022-10-20",
    "items": [
      {
        "id": 1338,
        "category": 1338,
        "type": "gradeGroup",
        "title": "Hausaufgabe Schriftlich",
        "subtitle": "Completare gli esercizi ...",
        "label": "Italienisch",
        "warning": false,
        "checkable": true,
        "checked": false,
        "online": 0,
        "submission": null,
        "deadline": "2022-10-20 07:45:00",
        "deadlineFormatted": "Donnerstag, 20.10.2022, 07:45",
        "deadlineStart": null,
        "deadlineStartFormatted": "Donnerstag, 01.01.1970, 01:00",
        "submissionAllowed": 0,
        "submissionResigned": false,
        "submissionIsNowInOvertime": false,
        "homework": 1,
        "done": null,
        "gradeGroupSubmissions": null
      }
    ]
  }
]
```

Each *day* object: `date` (`"yyyy-MM-dd"`), `items` (array of "homework"
entries).

Each *item* ("homework" in the client's model, `lib/data.dart:77-172`,
parsed in `lib/reducer/dashboard.dart:201-249`) — fields actually read by
the client:

| Field | Type | Notes |
|---|---|---|
| `id` | `int` | |
| `title` | `string` | |
| `subtitle` | `string` | |
| `label` | `string?` | e.g. subject name |
| `type` | `string` | one of `lessonHomework`, `gradeGroup`, `grade`, `observation`, `homework`; anything else is mapped to a local `unknown` value |
| `homework` | `int` (`0`/`1`) | if `0`, the client sets a `warning` flag on the item (this is separate from the boolean `warning` field also present in the payload; the client only reads `homework`, not the top-level boolean `warning`, to decide this — see [quirks](#known-quirks--undocumented-behavior)) |
| `checkable` | `bool?` | whether it can be toggled done/not-done |
| `checked` | `bool?` | current done-state |
| `deleteable` | `bool?` | whether the user is allowed to delete this (only true for user-created reminders) |
| `grade` | `string?` | only meaningful when `type == "grade"`; a dotted grade string, e.g. `"9.00"` |
| `gradeGroupSubmissions` | array or `null` | see below |

If `gradeGroupSubmissions` is present (not `null`), it's an array of
attachment objects:

```json
{
  "file": "abc123.pdf",
  "originalName": "essay.pdf",
  "timestamp": "2022-10-11T12:00:00",
  "typeName": "Abgabe",
  "id": 55,
  "gradeGroupId": 1338,
  "userId": 6619
}
```

### Homework / reminders

These are client-created "reminder" entries (visually similar to homework
items, but user-authored, not from the school).

**`POST api/student/dashboard/save_reminder`**

Request body:

```json
{ "date": "2022-10-25", "text": "Buy new pencils" }
```

```js
const reminder = await client.send("api/student/dashboard/save_reminder", {
  args: { date: "2022-10-25", text: "Buy new pencils" },
});
```

Response — same shape as one dashboard *item* above (**real captured
example**, `lib/demo.dart:16-23`):

```json
{
  "id": -1,
  "title": "test entry",
  "subtitle": "demo value",
  "warning": false,
  "deleteable": false,
  "type": "homework"
}
```

**`POST api/student/dashboard/delete_reminder`**

Request body:

```json
{ "id": 42 }
```

```js
const result = await client.send("api/student/dashboard/delete_reminder", {
  args: { id: 42 },
});
```

Response:

```json
{ "success": true }
```

**`POST api/student/dashboard/toggle_reminder`**

Request body:

```json
{ "id": 42, "type": "homework", "value": true }
```

```js
const result = await client.send("api/student/dashboard/toggle_reminder", {
  args: { id: 42, type: "homework", value: true },
});
```

`type` here is the homework-item `type` string (see above); `value` is the
new checked-state. Response (**real captured example**,
`lib/demo.dart:24`):

```json
{ "success": true }
```

If `success` is not exactly `true`, the client rolls back its optimistic UI
update.

### Absences

**`POST api/student/dashboard/absences`**

No request body (empty `args`).

```js
const absences = await client.send("api/student/dashboard/absences");
```

Response — **real captured example** (`lib/demo.dart:1124-1204`, trimmed;
`selfDeclarationsList` entries are static legal-text templates and omitted
here for brevity, see the source for the full text):

```json
{
  "absences": [
    {
      "justified": 2,
      "reason": null,
      "note": null,
      "reason_signature": null,
      "reason_timestamp": null,
      "group": [
        { "date": "2022-09-12", "hour": 3, "minutes": 50, "minutes_begin": 0, "minutes_end": 0 }
      ]
    }
  ],
  "futureAbsences": [
    {
      "justified": 4,
      "reason": "Arzttermin",
      "note": null,
      "reason_signature": null,
      "reason_timestamp": null,
      "startDate": "2022-11-01",
      "endDate": "2022-11-01",
      "startTime": 1,
      "endTime": 3
    }
  ],
  "canEdit": true,
  "statistics": {
    "counter": 0,
    "counterForSchool": 0,
    "percentage": 0,
    "justified": 0,
    "notJustified": 0,
    "delayed": 0
  },
  "selfDeclarationsList": [ "...static legal text entries, see lib/demo.dart..." ],
  "selfDeclarationsActiveList": [],
  "isAbsencesSelfDeclarationActive": false,
  "isAbsencesSelfDeclarationMandatory": true
}
```

Field notes (`lib/reducer/absences.dart`):

- `justified` (`int`) is an enum-like code, decoded by the client as:
  `2` → justified, `3` → not justified, `4` → "for school" (school-excused
  absence, e.g. a field trip), anything else → not yet justified
  (`lib/data.dart:516-524`).
- `group` is a list of individual absent hours making up one absence entry;
  each has `date`, `hour` (lesson number), `minutes` (duration; `50` = one
  full lesson), `minutes_begin` (minutes late), `minutes_end` (minutes left
  early).
- `percentage` in `statistics` is read/stored as a **string**, and treated as
  absent/`null` if it's an empty string.

### Calendar

**`POST api/calendar/student`**

Request body:

```json
{ "startDate": "2022-10-17" }
```

```js
const week = await client.send("api/calendar/student", {
  args: { startDate: "2022-10-17" }, // must be a Monday
});
```

`startDate` — Monday of the week to fetch (`"yyyy-MM-dd"`); the server
returns that whole week (5 school days).

Response shape (**real captured example** structure from
`lib/demo.dart:1205-4233`, heavily trimmed to one lesson slot):

```json
{
  "2022-10-17": {
    "1": {
      "1": {
        "1": {
          "isLesson": 1,
          "lesson": {
            "id": 3539,
            "date": "2022-10-17",
            "hour": 1,
            "toHour": 2,
            "timeStartObject": { "h": "07", "m": "45" },
            "timeEndObject":   { "h": "08", "m": "30" },
            "rooms": [ { "name": "A101" } ],
            "subject": { "id": 8, "name": "Englisch" },
            "teachers": [ { "id": 5971, "firstName": "Barbara", "lastName": "Mitterrutzner" } ],
            "homeworkExams": [],
            "lessonContents": [
              {
                "id": 1672,
                "name": "Writing conference, worksheet ...",
                "typeName": "Hausaufgabe",
                "lessonContentSubmissions": []
              }
            ],
            "linkedHours": []
          }
        }
      }
    },
    "2": { "...": "same shape, next lesson slot of the day" }
  },
  "2022-10-18": { "...": "next day" }
}
```

This is the single most awkward response shape in the API — read carefully:

- Top level: a map keyed by **date string** (`"yyyy-MM-dd"`) for each of the
  5 days of the requested week.
- Each day is a map keyed by an apparently-arbitrary numeric-string key
  (observed as `"1"`, `"2"`, ... — the client does not interpret this key's
  meaning at all, it just iterates `.values`).
- Each of those values is again a nested map of numeric-string keys, and the
  client always takes `.values.first.values.first` to drill down to the
  actual lesson-slot object (see `lib/reducer/calendar.dart:39-58`
  `_parseCalendarDay`, and `_parseHour` below). In other words, only the
  *first* entry of the two inner map levels is ever used; if there is more
  than one entry at those levels, everything but the first is silently
  ignored by this client.
- Values with `"isLesson": 0` (or `null`) are lesson slots with nothing
  scheduled and are filtered out client-side.
- `lesson.hour` / `lesson.toHour` — first/last lesson-period number covered
  (a "double lesson" spans two).
- `lesson.timeStartObject` / `timeEndObject` / (also seen: `timeToEndObject`)
  — each an object with at least `h` (hour, zero-padded string) and `m`
  (minute, zero-padded string); there are also pre-formatted `text`/`html`/`ts`
  (unix seconds within the day?) fields the client does not use.
- `lesson.linkedHours` — array of further lesson objects (same shape as
  `lesson` itself) representing other periods that are part of the same
  logical lesson block (e.g. a lesson spanning a lunch break, or recurring
  across the week); the client builds one `TimeSpan` per entry (including
  the outer `lesson` itself).
- `lesson.rooms` — array of `{ "name": "..." }`.
- `lesson.subject` — `{ "id": int, "name": string, ... }`.
- `lesson.teachers` — array of `{ "id": int, "firstName": string, "lastName": string }`.
- `lesson.homeworkExams` — array of:
  ```json
  {
    "id": 1,
    "name": "Vocabulary test",
    "homework": 1,
    "online": 0,
    "deadline": "2022-10-20 07:45:00",
    "hasGrades": false,
    "hasGradeGroupSubmissions": false,
    "typeId": 3,
    "typeName": "Schularbeit"
  }
  ```
  Same `homework: 0` ⇒ warning-flag convention as dashboard items (see
  [quirks](#known-quirks--undocumented-behavior)); `online`/`homework` are
  `0`/`1` ints, not booleans, in this payload (unlike the dashboard payload
  where some are already booleans — the two endpoints are inconsistent with
  each other here).
- `lesson.lessonContents` — array of:
  ```json
  {
    "name": "Chapter 3 exercises",
    "typeName": "Hausaufgabe",
    "lessonContentSubmissions": [
      {
        "type": "file",
        "originalName": "homework.pdf",
        "id": "991",
        "lessonContentId": "1672"
      }
    ]
  }
  ```
  Only submissions with `"type": "file"` are kept; everything else is
  dropped client-side. Note `id` and `lessonContentId` here are **strings**,
  not ints (unlike almost every other `id` field in this API).

### Semester switch

```
POST {baseUrl}/v2/?semesterWechsel=1
```

```js
await client.send(`?semesterWechsel=${1}`); // 1 = first semester, 2 = second
```

or `?semesterWechsel=2`. No request body (empty `args`); the URL's query
string is appended directly to the base `/v2/` path — note this is **not** a
normal path segment, the literal request path is `/v2/?semesterWechsel=1`.

This mimics clicking the semester switch in the web UI; the response body is
not read by the client. It must be called (and its response awaited) once
**before** issuing any of the [grades](#grades) requests for the semester
you want data for — the actual semester-scoped grade data comes back from
`api/student/all_subjects`/`api/student/subject_detail` themselves, which are
implicitly scoped to whatever semester the *session* was last switched to
server-side (there is no `semester` parameter on those calls). The client
serializes this to avoid two concurrent requests for different semesters
racing each other (`SemesterLock` in `lib/middleware/grades.dart:172-212`).

### Grades

**`POST api/student/all_subjects`**

Request body:

```json
{ "studentId": 6619 }
```

```js
const subjects = await client.send("api/student/all_subjects", {
  args: { studentId: config.userId },
});
```

`studentId` is the student's `userId`, from the [session config](#session-config-html-scrape).
Must be called once per semester (see [semester switch](#semester-switch)
above) — the returned grades are for whatever semester the session is
currently switched to.

Response — **real captured example** (`lib/demo.dart:863-1123`, trimmed to
2 subjects):

```json
{
  "subjects": [
    {
      "subject": { "id": 17, "name": "Bewegung und Sport" },
      "absences": 0,
      "grades": [
        {
          "grade": "9.00",
          "weight": 100,
          "date": "2022-10-11",
          "type": "Sonstige Bewertung",
          "typeId": 7,
          "studentId": 6619,
          "cancelled": 0,
          "subjectId": 17,
          "semester": 1,
          "createdTimeStamp": "2022-10-17 12:34:12",
          "cancelledTimeStamp": null,
          "description": ""
        }
      ],
      "averageSemester": 0,
      "averageYear": 0,
      "subjectId": 17,
      "student": { "id": 6619, "firstName": "Michael", "lastName": "Debertol" },
      "countCompetences": 0,
      "countDescriptions": 0,
      "countObservations": 1
    },
    {
      "subject": { "id": 6, "name": "Deutsch" },
      "absences": 0,
      "grades": [],
      "averageSemester": 0,
      "averageYear": 0,
      "subjectId": 6,
      "student": { "id": 6619, "firstName": "Michael", "lastName": "Debertol" },
      "countCompetences": 0,
      "countDescriptions": 0,
      "countObservations": 0
    }
  ]
}
```

Only these fields are read by the client (`lib/reducer/grades.dart:64-110, 191-200`):

- `subjects[].subject.id`, `subjects[].subject.name`
- `subjects[].grades[]` — each: `grade` (dotted string, e.g. `"9.00"`;
  parsed client-side into an integer "centi-grade", `900`, via
  `major*100 + minor`, so the two digits after the dot **must** always be
  present — `"9.0"` would break parsing, `"9.00"` is required), `weight`
  (`int`, percent weight of this grade in the average), `date`
  (`"yyyy-MM-dd"`), `cancelled` (`int`, `0`/non-`0`), `type` (`string`,
  display name of the grade type — note this is `subjects[].grades[].type`
  here, but the *detail* endpoint below calls the equivalent field
  `typeName` instead — another cross-endpoint naming inconsistency).

All other fields shown (`absences`, `averageSemester`, `averageYear`,
`student`, `countCompetences`, `countDescriptions`, `countObservations`,
`weight`'s sibling fields like `typeId`/`studentId`/`subjectId`/
`createdTimeStamp`/`cancelledTimeStamp`/`description`) are present in real
responses but **not read** by this client — they may be relevant for a more
complete wrapper, they're just not exercised here. (`subjects[].grades[]`
here is a summary; use the detail endpoint below for full data per subject,
including per-grade IDs, descriptions, competences, and observations.)

---

**`POST api/student/subject_detail`**

Request body:

```json
{ "studentId": 6619, "subjectId": 17 }
```

```js
let detail = await client.send("api/student/subject_detail", {
  args: { studentId: config.userId, subjectId: 17 },
});
if (typeof detail === "string") detail = JSON.parse(detail); // see the double-encoding note below
```

Response — **real captured example** (`lib/demo.dart:14-15`) — ⚠️ note this
one is returned as a **JSON string, not a JSON object** (i.e. double-encoded:
the HTTP body is a JSON string literal whose *contents*, once parsed again,
are the actual object). The client explicitly handles this
(`lib/middleware/grades.dart:97-99`: `if (data is String) data = json.decode(data);`).
A robust wrapper should attempt a second `json.decode` if the first decode
yields a string.

Decoded contents:

```json
{
  "grades": [
    {
      "id": 1202,
      "grade": "9.00",
      "weight": 100,
      "typeId": 7,
      "typeName": "Sonstige Bewertung",
      "name": "Ausdauertest",
      "description": "",
      "date": "2022-10-11",
      "cancelled": 0,
      "created": "Von Gernot Wachtler am 17.10.2022 eingetragen",
      "subjectId": 17,
      "classId": 60,
      "studentId": 6619,
      "createdTimeStamp": "2022-10-17 12:34:12",
      "cancelledTimeStamp": null,
      "competences": []
    }
  ],
  "absences": [],
  "observations": [
    {
      "id": 980,
      "note": "",
      "date": "2022-10-11",
      "typeId": 19,
      "typeName": "Plus",
      "cancelled": 0,
      "created": "Von Gernot Wachtler am 17.10.2022 eingetragen",
      "subjectId": 17,
      "classId": 60,
      "studentId": 6619,
      "hidden": 0
    }
  ],
  "averageSemester": 0,
  "averageYear": 0,
  "showGrades": 2,
  "showGradesStudentView": 2,
  "isClassHasNoGrades": false,
  "countCompetences": 0,
  "countDescriptions": 0,
  "countObservations": 1
}
```

Fields read by the client (`lib/reducer/grades.dart:168-231`):

- `grades[]`: `id`, `grade` (same dotted-string convention as above),
  `date`, `weight`, `cancelled` (`bool` here — note: **boolean** in this
  endpoint vs **int `0`/non-`0`** in `all_subjects` above), `typeName`
  (display name — cf. `type` in the summary endpoint), `created` (already a
  human-readable, German, server-formatted string — not just a timestamp),
  `name`, `description`, `competences[]` — each `{ "typeName": string,
  "grade": "0".."5" as a decimal string, parsed to `int` }`.
- `observations[]`: `typeName`, `cancelled` (compared against `!= 0`, so int
  semantics again here, inconsistent with `grades[].cancelled`'s boolean
  parsing two lines above in the same response!), `created`, `note`, `date`.

### Cancelled-grade description

**`POST api/student/entry/getGrade`**

Used to fetch the reason text for a grade that has `cancelled: true`.

Request body:

```json
{ "gradeId": 1202 }
```

```js
const result = await client.send("api/student/entry/getGrade", {
  args: { gradeId: 1202 },
});
```

Response (reconstructed from parsing code,
`lib/reducer/grades.dart:222-226`):

```json
{ "cancelledDescription": "Aufgabe wegen Krankheit storniert." }
```

Only `cancelledDescription` is read.

### Certificate

**`GET student/certificate`** (⚠️ `GET`, unlike almost everything else)

```js
const html = await client.send("student/certificate", { method: "GET" });
```

No request body. Response is **raw HTML** (not JSON) — the certificate/report
card rendered as an HTML fragment, meant to be displayed in a WebView. **Real
captured example** (`lib/demo.dart:4234-4235`):

```html
<div class="student-subject-list"><div class="default-page-container"><h2 class="h2 margin-top">Zeugnis Debertol Michael</h2>Zeugnis noch nicht verfügbar</div></div>
```

("Zeugnis noch nicht verfügbar" = "Report card not yet available" — i.e.
this endpoint returns a placeholder/explanatory HTML snippet, not an error,
when there's nothing to show yet.) The client stores this HTML verbatim and
renders it as-is; there is no structured parsing of it.

### Notifications

**`POST api/notification/unread`**

```js
const notifications = await client.send("api/notification/unread");
```

No request body. Response: JSON array (**real captured example**,
`lib/demo.dart:862`, was empty in the demo fixture; shape reconstructed from
parsing code at `lib/reducer/notifications.dart:45-62`):

```json
[
  {
    "id": 501,
    "title": "Neue Hausaufgabe",
    "subTitle": "Italienisch",
    "type": "message",
    "objectId": 1234,
    "timeSent": "2022-10-19T08:00:00"
  }
]
```

- `type` — a free-form string; the only value the client treats specially is
  `"message"`, in which case `objectId` is the corresponding message's `id`
  (used to mark it read, and to filter it out of "delete all" — see below).
- `subTitle` — capital `T` (`subTitle`, not `subtitle` — unlike the
  dashboard endpoint's `subtitle`, all-lowercase `t`; yet another
  cross-endpoint field-name inconsistency to preserve exactly).

**`POST api/notification/markAsRead`**

Mark one notification read:

```json
{ "id": 501 }
```

```js
await client.send("api/notification/markAsRead", { args: { id: 501 } });
```

Mark **all** notifications read — same endpoint, called with an **empty**
body:

```json
{}
```

```js
await client.send("api/notification/markAsRead"); // args defaults to {}
```

(no response fields are read by the client either way)

### Messages

**`POST api/message/getMyMessages`**

```js
const messages = await client.send("api/message/getMyMessages");
```

No request body. Response: JSON array (**real captured example**,
`lib/demo.dart:4236`, was empty in the demo fixture; shape reconstructed
from `lib/reducer/messages.dart:114-167`):

```json
[
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
        "type": "file",
        "isDownloadable": true,
        "id": 12,
        "messageId": 88,
        "originalName": "einladung.pdf",
        "file": "abcd1234.pdf"
      }
    ]
  }
]
```

- `timeRead` — `null` while unread; once read, an ISO-ish timestamp string
  (the client only ever writes this locally when marking read — see below;
  it does not appear to send timestamps back to the server for this).
- `submissions[]` — attachments; only entries with **both**
  `"type": "file"` **and** `"isDownloadable": true` are surfaced to the
  user; all four of `id`, `messageId`, `originalName`, `file` must be
  non-null or the entry is silently dropped.

**`POST api/message/markAsRead`**

```json
{ "messageId": 88 }
```

```js
await client.send("api/message/markAsRead", { args: { messageId: 88 } });
```

(Response not read by the client — it just optimistically marks the message
read locally, setting a local `timeRead` timestamp of "now"; it does **not**
wait for/trust a server-echoed value.)

### Profile

**`POST api/profile/get`**

```js
const profile = await client.send("api/profile/get");
```

No request body. Response (reconstructed from
`lib/reducer/profile_reducer.dart:42-51`):

```json
{
  "name": "Michael Debertol",
  "email": "student@example.com",
  "roleName": "Schüler",
  "notificationsEnabled": true,
  "username": "mmustermann"
}
```

**`POST api/profile/updateNotificationSettings`**

```json
{ "notificationsEnabled": true }
```

```js
await client.send("api/profile/updateNotificationSettings", {
  args: { notificationsEnabled: true },
});
```

(Response not read beyond a null-check for failure.)

**`POST api/profile/updateProfile`**

Used to change the account's contact email (re-authenticating with the
current password):

```json
{ "email": "new-address@example.com", "password": "hunter2" }
```

```js
const result = await client.send("api/profile/updateProfile", {
  args: { email: "new-address@example.com", password: "hunter2" },
});
```

Response, success:

```json
{ "message": "E-Mail erfolgreich geändert." }
```

Response, failure:

```json
{ "error": "wrong_password", "message": "Passwort ist falsch." }
```

`message` is shown to the user in both cases (`lib/middleware/profile.dart:70-75`);
`error` presence is what distinguishes success from failure (same convention
as the auth endpoints).

### File downloads

Unlike everything above, downloads are **not** issued through `Wrapper.send`
— they're plain authenticated `GET` requests (same session cookies apply)
made directly against `dio`, with `responseType: stream`, and the body is
streamed to disk rather than parsed as JSON (`downloadFile`,
`lib/middleware/middleware.dart:629-670`). All three endpoints live under
`{baseUrl}/v2/` like the rest of the API (they're built from
`wrapper.baseAddress`, same as everything else — not the no-`/v2/` exception
that only applies to password reset).

| Endpoint | Query parameters | Used for |
|---|---|---|
| `GET api/gradeGroup/gradeGroupSubmissionDownloadEntry` | `submissionId`, `parentId` (the `gradeGroupId`) | Dashboard homework attachments |
| `GET api/lessonContent/lessonContentSubmissionDownloadEntry` | `parentId` (the `lessonContentId`), `submissionId` | Calendar lesson-content attachments |
| `GET api/message/messageSubmissionDownloadEntry` | `messageId`, `submissionId` | Message attachments |

Example:

```
GET {baseUrl}/v2/api/message/messageSubmissionDownloadEntry?messageId=88&submissionId=12
```

```js
const blob = await client.download("api/message/messageSubmissionDownloadEntry", {
  messageId: 88,
  submissionId: 12,
});
// the other two download endpoints work the same way, e.g.:
// client.download("api/gradeGroup/gradeGroupSubmissionDownloadEntry", { submissionId, parentId });
// client.download("api/lessonContent/lessonContentSubmissionDownloadEntry", { parentId, submissionId });
```

Response: the raw file bytes, presumably with a `Content-Disposition`
header carrying the filename (the client does not actually read that header
— it already knows the desired filename from the JSON metadata it fetched
earlier, e.g. `originalName`). A `200` status is treated as success; anything
else is treated as a failed download and any partially-written local file is
deleted.

## Connectivity check

Separately from all of the above (and **not** authenticated / not through
`Wrapper` at all — uses the plain `http` package, not `dio`), the app has a
basic reachability check (`lib/util.dart:236-245`):

```dart
Future<bool> cannotConnectTo(String url) async {
  var noInternet = false;
  try {
    final result = await http.get(Uri.parse(url));
    noInternet = result.statusCode != 200;
  } catch (e) {
    noInternet = true;
  }
  return noInternet;
}
```

It's called with either the current `baseAddress` (if a server URL is
already known) or, as a generic "is there internet at all" probe, plain
`https://digitalesregister.it` (the vendor's own marketing/portal domain,
used purely as a reachability ping — not part of the authenticated API).

## Known quirks & undocumented behavior

Collected here for visibility since a from-scratch client is likely to trip
over these:

1. **No HTTP error codes for auth failure.** Expired/invalid sessions come
   back as `200 OK` with an HTML `<script>` redirect body (see
   [unexpected-logout detection](#unexpected-logout-detection--retry)), not
   `401`/`403`. Always check the body shape, not just the status code.

2. **Inconsistent booleans across (and even within) endpoints.** Some fields
   use real JSON booleans, others use `int` `0`/non-`0` you must
   compare/coerce yourself. Notably `cancelled` is a **bool** in
   `api/student/subject_detail`'s `grades[]` but an **int** in that very
   same response's `observations[]`, and also an **int** in
   `api/student/all_subjects`'s `grades[]`.

3. **The `homework` field means the opposite of what it sounds like for
   "warning" purposes.** On dashboard items and calendar `homeworkExams`,
   `homework: 0` is what the client uses to raise a "warning" indicator in
   the UI (`warning = data["homework"] == 0`) — the payload *also* separately
   contains its own top-level `warning` boolean, which the client does **not**
   use for this. Prefer replicating `homework == 0` if you need this signal.

4. **`api/student/subject_detail` returns double-encoded JSON.** The HTTP
   body is a JSON string; decode once to get a string, decode again to get
   the actual object. (`api/student/all_subjects`, by contrast, returns a
   plain object directly — the two "grades" endpoints are inconsistent with
   each other here too.)

5. **Grade strings need exactly two decimal digits.** Grades are transmitted
   as strings like `"9.00"`/`"9.75"`, parsed client-side as
   `major*100 + minor` where `minor` is taken as-is from the string after the
   dot (so it must be exactly two digits; `"9.5"` would parse as `major=9,
   minor=5` → `905`, not the presumably-intended `950`). The four
   "quarter-grade" fractional values in practical use are `.00`, `.25`,
   `.50`, `.75`.

6. **`?semesterWechsel=N` is a real endpoint disguised as a query string on
   the base path**, not a normal REST resource — it mutates *session* state
   server-side (which semester subsequent grade requests are scoped to)
   rather than returning meaningful data itself.

7. **The calendar response's outer map keys are effectively meaningless to
   the client** — it always drills into `.values.first.values.first` two
   levels deep and ignores the string keys entirely, and also ignores every
   sibling but the first at those two levels. If those keys carry real
   information (e.g. distinguishing multiple lessons happening at the exact
   same hour), this client discards it; don't assume "first" is always
   "only."

8. **Field names are not consistent across endpoints for the same concept.**
   Examples: grade-type display name is `type` in `all_subjects` but
   `typeName` in `subject_detail`; notification subtitle is `subTitle`
   (capital T) but dashboard item subtitle is `subtitle` (lowercase);
   `lessonContentId`/submission `id` are strings in the calendar endpoint
   but ints almost everywhere else IDs appear.

9. **A raw `"0": true` key has been observed on real dashboard items**
   (see the dashboard example above) that is not read/explained anywhere in
   the client code. Treat unknown keys as safe to ignore, but be aware the
   server does send some fields nobody has reverse-engineered yet.

10. **Deep links.** The app registers itself for URLs of the form
    `{baseUrl}/v2/login?resetmail=true&email=...&token=...` (routes into the
    password-reset-step-2 screen) and `{baseUrl}/v2/login?username=...` /
    `&redirect=...`, and `{baseUrl}/v2/?semesterWechsel=1|2` as a *plain
    top-level app open* also toggles which semester is shown after login
    (`lib/middleware/middleware.dart:506-558`). These aren't server API
    calls themselves, but if you're replicating "what the official app does
    when a user taps an email link," this is the logic for it.

11. **A built-in offline "demo" login exists** (`lib/demo.dart`,
    `isDemoUser(...)` gate in `lib/wrapper.dart:118`) that bypasses the real
    network entirely for a hardcoded demo username/URL combination and
    serves the canned responses this document quotes as examples from. It's
    irrelevant to talking to a real server, but explains where these example
    payloads originated.

12. **Only student/parent accounts are supported end-to-end.** Teacher (or
    other role) accounts are explicitly detected via
    `isStudentOrParent` (see [session config](#session-config-html-scrape))
    and rejected client-side after login succeeds. The server-side login
    call itself does succeed for such accounts (`loggedIn: true`) — it's
    purely a client-side policy decision to log back out and refuse to
    proceed, so a from-scratch client is *not* technically blocked from
    exploring further for such accounts, but no endpoint behavior has been
    verified for them here.
