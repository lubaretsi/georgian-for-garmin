//! The watch report: what a watch owner shares in a public GitHub issue, and the new-issue link
//! that fills in the repository's issue form (`.github/ISSUE_TEMPLATE/watch-report.yml`).
//!
//! Everything in a report comes from an allowlist of fields, so the unit ID, serial number and
//! every other part of `GarminDevice.xml` stay out: the model's description, part number and
//! software version; each `UpdateFile`'s path, file name, part number and version; the
//! `GARMIN/ExtData` listing (name, size, and SHA-256 only of files `inspect` reads anyway);
//! this build's host choice, packs and per-part state; the watch's status in the support list;
//! the last install or restore; and the program's version, OS and CPU architecture.
//!
//! GitHub and browsers refuse very long URLs, so the report is kept compact; one that would
//! still make the link longer than [`MAX_URL`] is left out of it, and the form's report field
//! stays empty for a paste from the clipboard.

use std::fmt::Write as _;
use std::sync::LazyLock;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::busy::{Busy, HolderKind};
use crate::error::{Error, ErrorCode};
use crate::fonts::Kind;
use crate::garmin_device::GarminDevice;
use crate::ops::{APP_VERSION, FileState, GeorgianState, StageStatus, TestStatus};
use crate::support::{Support, SupportStatus};

/// Where watch reports are filed.
pub const NEW_ISSUE: &str = "https://github.com/lubaretsi/georgian-for-garmin/issues/new";
/// The repository's issues; issue `n` is `{ISSUES}/{n}`.
pub const ISSUES: &str = "https://github.com/lubaretsi/georgian-for-garmin/issues";

/// The page of GitHub issue `number`.
pub fn issue_url(number: u32) -> String {
    format!("{ISSUES}/{number}")
}

/// The issue form, in `.github/ISSUE_TEMPLATE/`.
pub const TEMPLATE: &str = "watch-report.yml";
/// Longest link we make. GitHub answers 414 somewhat above 8 KB; browsers allow more.
pub const MAX_URL: usize = 8000;
/// The phone page that sends the test notification.
pub const PHONE_PAGE: &str = "https://lubaretsi.github.io/georgian-for-garmin/";

/// The computer the report was made on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Platform {
    /// e.g. "Mac OS", "Windows", "Ubuntu".
    pub os: String,
    /// e.g. "15.6.1".
    pub os_version: String,
    /// CPU architecture this program was built for, e.g. "aarch64".
    pub arch: String,
}

/// This computer, read once per process.
static CURRENT: LazyLock<Platform> = LazyLock::new(|| {
    let info = os_info::get();
    Platform {
        os: info.os_type().to_string(),
        os_version: info.version().to_string(),
        arch: std::env::consts::ARCH.to_owned(),
    }
});

impl Platform {
    /// This computer.
    pub fn current() -> Platform {
        CURRENT.clone()
    }
}

/// How the watch was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Connection {
    /// USB file transfer (MTP): the mode `install` needs.
    Mtp,
    /// A mounted USB drive (mass storage); read-only, nothing can be installed.
    UsbDrive,
}

/// An `UpdateFile` from `GarminDevice.xml`'s `MassStorageMode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeclaredFile {
    /// e.g. "Garmin/ExtData".
    pub path: String,
    pub file_name: String,
    pub part_number: String,
    /// e.g. "2.10"; null if the watch gives none.
    pub version: Option<String>,
}

/// A `Garmin/ExtData` update file from `GarminDevice.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportedFile {
    pub part_number: String,
    /// e.g. "2.10"; null if the watch gives none.
    pub version: Option<String>,
    pub file_name: String,
}

/// A file in the watch's `GARMIN/ExtData` folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListedFile {
    pub name: String,
    pub bytes: u64,
    /// Only for files `inspect` reads anyway (named for a part of this build, with the size of
    /// one of its versions); null for all others.
    pub sha256: Option<String>,
}

/// Whether `ExtData`'s hidden children could be listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HiddenStatus {
    /// Listed with Garmin's vendor `GetObjectIds` (0x9002).
    Listed,
    /// The watch doesn't list that operation.
    Unsupported,
    /// Listing or reading failed; `note` says how. Nothing else is affected.
    Error,
    /// Not looked at: `ExtData` wasn't found, or the watch is a USB drive.
    NotRead,
}

/// A child of `GARMIN/ExtData` that the normal listing leaves out: on the Venu 2, the read-only
/// objects, such as imported fonts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HiddenFile {
    pub name: String,
    pub bytes: u64,
    /// The object's MTP ProtectionStatus is other than No Protection; null when the backend
    /// can't tell.
    pub read_only: Option<bool>,
    /// What it holds, only for a file named for a part this build installs or restores on the
    /// watch (`inspect`'s file rule); null for every other file, which isn't read.
    pub file: Option<FileState>,
}

/// `ExtData`'s hidden children; install and restore identify a part's file from them where the
/// normal listing doesn't show it, and never delete one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HiddenListing {
    pub status: HiddenStatus,
    pub files: Vec<HiddenFile>,
    /// A short note on what failed, with `error`.
    pub note: Option<String>,
}

impl HiddenListing {
    pub fn not_read() -> Self {
        HiddenListing {
            status: HiddenStatus::NotRead,
            files: Vec::new(),
            note: None,
        }
    }
}

/// Which kind of font gets Georgian on this watch, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportedHosts {
    pub kind: Option<Kind>,
    pub reason: String,
}

/// A part of this build that the watch lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportedPart {
    pub part: String,
    /// `install` writes it.
    pub target: bool,
    /// What its file in `GARMIN/ExtData` held when the report was made (`inspect`'s rule).
    pub file: FileState,
    pub tested: TestStatus,
    /// Beta builds: the test mark's variant ("A", "B", …) in this part's Georgian build.
    pub mark: Option<String>,
}

/// The watch's font list for sharing. It never includes the unit ID, serial number or any
/// other `GarminDevice.xml` field outside the allowlist in the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WatchReport {
    /// This program's version.
    pub app_version: String,
    pub platform: Platform,
    pub connection: Connection,
    pub model: String,
    pub model_part: String,
    pub software_version: String,
    /// Ids of the packs `install` writes, in registry order; empty if none.
    pub packs: Vec<String>,
    /// The overall status, as in `inspect`'s `tested`.
    pub tested: TestStatus,
    /// The watch's status in the support list, as in `inspect`'s `support`; unknown when absent
    /// (reports from before this field).
    #[serde(default)]
    pub support: Support,
    /// Georgian on the watch, as in `inspect`'s `state`.
    pub state: GeorgianState,
    pub hosts: ReportedHosts,
    /// Every part of this build the watch lists, in registry order.
    pub parts: Vec<ReportedPart>,
    /// This is a beta build whose Georgian fonts carry test marks.
    pub test_marks: bool,
    /// Every `UpdateFile` the watch declares, in its order.
    pub declared_files: Vec<DeclaredFile>,
    /// The declared files whose path is `Garmin/ExtData`, in the watch's order.
    pub extdata_files: Vec<ReportedFile>,
    /// What `GARMIN/ExtData` holds; null if the folder wasn't found.
    pub extdata_listing: Option<Vec<ListedFile>>,
    /// `ExtData`'s children the normal listing leaves out; `not_read` when absent (reports from
    /// before this field).
    #[serde(default = "HiddenListing::not_read")]
    pub extdata_hidden: HiddenListing,
    /// A title for the issue.
    pub title: String,
    /// The report as compact text for the form's report field (rendered as a code block),
    /// without an outcome or answers.
    pub text: String,
}

/// What the parts of a report other than the device come from.
pub struct ReportInput {
    pub connection: Connection,
    pub packs: Vec<String>,
    pub tested: TestStatus,
    pub support: Support,
    pub state: GeorgianState,
    pub hosts: ReportedHosts,
    pub parts: Vec<ReportedPart>,
    pub test_marks: bool,
    pub extdata_listing: Option<Vec<ListedFile>>,
    pub extdata_hidden: HiddenListing,
}

/// The report for `device`. Built only from the fields named here.
pub fn watch_report(device: &GarminDevice, input: ReportInput) -> WatchReport {
    let declared_files = device
        .update_files
        .iter()
        .map(|f| DeclaredFile {
            path: f.path.clone(),
            file_name: f.file_name.clone(),
            part_number: f.part_number.clone(),
            version: f.version.clone(),
        })
        .collect();
    let extdata_files = device
        .extdata_files()
        .map(|f| ReportedFile {
            part_number: f.part_number.clone(),
            version: f.version.clone(),
            file_name: f.file_name.clone(),
        })
        .collect();
    let mut report = WatchReport {
        app_version: APP_VERSION.into(),
        platform: Platform::current(),
        connection: input.connection,
        model: device.description.clone(),
        model_part: device.part_number.clone(),
        software_version: device.software_version.clone(),
        packs: input.packs,
        tested: input.tested,
        support: input.support,
        state: input.state,
        hosts: input.hosts,
        parts: input.parts,
        test_marks: input.test_marks,
        declared_files,
        extdata_files,
        extdata_listing: input.extdata_listing,
        extdata_hidden: input.extdata_hidden,
        title: String::new(),
        text: String::new(),
    };
    report.title = title(&report);
    report.text = text(&report, None, None);
    report
}

/// Which operation an outcome is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageOperation {
    Install,
    RestoreStock,
}

/// How the last install or restore went: its status, or its error code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub operation: StageOperation,
    /// Set when it succeeded.
    #[serde(default)]
    pub status: Option<StageStatus>,
    /// Set when it failed.
    #[serde(default)]
    pub error: Option<ErrorCode>,
    /// The parts it wrote (or found already there), when it succeeded.
    #[serde(default)]
    #[schemars(inner(pattern(r"^[0-9]{3}-[A-Z][0-9]{4}-[0-9]{2}$")))]
    pub parts: Vec<String>,
}

/// What the notification looked like on the watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Seen {
    /// Georgian letters showed.
    Georgian,
    /// Boxes instead of letters.
    Boxes,
    /// Question marks instead of letters (the fenix 6S Pro, #6).
    QuestionMarks,
    /// Some letters, some boxes.
    SomeLetters,
    /// The notification didn't reach the watch.
    NoNotification,
    /// Georgian didn't install.
    NotInstalled,
}

impl Seen {
    /// Every answer, in the form's order.
    pub const ALL: [Seen; 6] = [
        Seen::Georgian,
        Seen::Boxes,
        Seen::QuestionMarks,
        Seen::SomeLetters,
        Seen::NoNotification,
        Seen::NotInstalled,
    ];

    /// The option's label in the issue form.
    pub fn label(self) -> &'static str {
        match self {
            Seen::Georgian => "Georgian letters",
            Seen::Boxes => "Boxes instead of letters",
            Seen::QuestionMarks => "Question marks (?) instead of letters",
            Seen::SomeLetters => "Some letters, some boxes",
            Seen::NoNotification => "The notification didn't arrive",
            Seen::NotInstalled => "Georgian didn't install",
        }
    }
}

/// Where on the watch the notification was looked at.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum View {
    /// The pop-up when it arrives.
    Popup,
    /// The notification list.
    List,
    /// The opened notification.
    Opened,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Popup => "Pop-up when it arrives",
            View::List => "Notification list",
            View::Opened => "Opened notification",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Phone {
    Android,
    Iphone,
}

impl Phone {
    pub fn label(self) -> &'static str {
        match self {
            Phone::Android => "Android",
            Phone::Iphone => "iPhone",
        }
    }
}

/// The marker answer meaning none of the pictures matched.
pub const NO_MARKER: &str = "none";

/// The marker option's label in the issue form.
pub fn marker_label(marker: &str) -> &str {
    if marker == NO_MARKER {
        "None of the pictures"
    } else {
        marker
    }
}

/// The watch owner's answers to the check screen's questions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Answers {
    #[serde(default)]
    pub result: Option<Seen>,
    /// Views that showed Georgian letters.
    #[serde(default)]
    pub views: Vec<View>,
    #[serde(default)]
    pub phone: Option<Phone>,
    /// Beta builds: the test mark's variant seen on the watch, or "none".
    #[serde(default)]
    #[schemars(regex(pattern = r"^([A-Z]|none)$"))]
    pub marker: Option<String>,
    /// The Garmin Connect app's version, if known.
    #[serde(default)]
    #[schemars(length(max = 40))]
    pub connect_version: Option<String>,
}

/// `issue_link`'s input.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IssueLinkInput {
    /// The `report` from `inspect` or `inspect_drive`. Its `title` and `text` are ignored and
    /// rebuilt from the other fields. Absent for a watch another program held before it could
    /// be read: then `busy` is needed, and `outcome` and `answers` aren't taken.
    #[serde(default)]
    pub report: Option<WatchReport>,
    /// `device_busy`'s `busy`: who held the watch. Adds the report's `busy` line.
    #[serde(default)]
    pub busy: Option<Busy>,
    /// The last install or restore on this watch, if any.
    #[serde(default)]
    pub outcome: Option<Outcome>,
    #[serde(default)]
    pub answers: Option<Answers>,
}

/// A new-issue link with the form filled in.
#[derive(Debug, Serialize, JsonSchema)]
pub struct IssueLink {
    /// The link to open.
    pub url: String,
    /// The report is in the link; when false, the link leaves the form's report field empty
    /// for a paste and `report_text` should go to the clipboard.
    pub report_in_url: bool,
    pub title: String,
    /// The report field's full text: exactly what is shared besides the answers' own fields.
    pub report_text: String,
    /// Each form field the link fills, by field id, in order (the report field only when it
    /// fit).
    pub fields: Vec<FormField>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FormField {
    pub id: String,
    pub value: String,
}

/// The new-issue link for `input`.
pub fn issue_link(input: &IssueLinkInput) -> Result<IssueLink, Error> {
    if let Some(outcome) = &input.outcome
        && outcome.status.is_some() == outcome.error.is_some()
    {
        return Err(Error::new(
            ErrorCode::InvalidInput,
            "outcome needs exactly one of status and error",
        ));
    }
    let answers = input.answers.as_ref();
    let (title, report_text) = match (&input.report, &input.busy) {
        (Some(report), busy) => {
            let mut text = text(report, input.outcome.as_ref(), answers);
            if let Some(busy) = busy {
                // After the `app` and `connection` lines.
                let at = text
                    .match_indices('\n')
                    .nth(1)
                    .map_or(text.len(), |(i, _)| i + 1);
                text.insert_str(at, &format!("{}\n", busy_line(busy)));
            }
            (title(report), text)
        }
        (None, Some(busy)) => {
            if input.outcome.is_some() || answers.is_some() {
                return Err(Error::new(
                    ErrorCode::InvalidInput,
                    "outcome and answers need a report",
                ));
            }
            (busy_title(busy), busy_text(busy))
        }
        (None, None) => {
            return Err(Error::new(
                ErrorCode::InvalidInput,
                "issue_link needs a report, busy, or both",
            ));
        }
    };
    let mut fields: Vec<(&str, String)> = Vec::new();
    if let Some(answers) = answers {
        if let Some(result) = answers.result {
            fields.push(("result", result.label().into()));
        }
        if !answers.views.is_empty() {
            let labels: Vec<&str> = answers.views.iter().map(|v| v.label()).collect();
            fields.push(("views", labels.join(", ")));
        }
        if let Some(marker) = &answers.marker {
            fields.push(("marker", marker_label(marker).into()));
        }
        if let Some(phone) = answers.phone {
            fields.push(("phone", phone.label().into()));
        }
        if let Some(version) = answers.connect_version.as_deref().map(str::trim)
            && !version.is_empty()
        {
            fields.push(("connect", version.into()));
        }
    }
    let url = |report_field: Option<&str>| -> String {
        let mut params = vec![("template", TEMPLATE), ("title", title.as_str())];
        params.extend(fields.iter().map(|(id, value)| (*id, value.as_str())));
        params.extend(report_field.map(|report| ("report", report)));
        Url::parse_with_params(NEW_ISSUE, params)
            .expect("NEW_ISSUE is a valid URL")
            .into()
    };
    let full = url(Some(&report_text));
    // A report that doesn't fit is left out, not replaced by instructions: the form's report
    // box stays empty for the paste, and GitHub's `required` check asks for it.
    let report_in_url = full.len() <= MAX_URL;
    let url = if report_in_url {
        fields.push(("report", report_text.clone()));
        full
    } else {
        url(None)
    };
    Ok(IssueLink {
        url,
        report_in_url,
        title,
        report_text,
        fields: fields
            .into_iter()
            .map(|(id, value)| FormField {
                id: id.into(),
                value,
            })
            .collect(),
    })
}

/// A watch-provided value on one line: words joined by single spaces, "(none)" if empty.
fn one_line(value: &str) -> String {
    let line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
        "(none)".into()
    } else {
        line
    }
}

/// One space-separated column: "-" when empty; quoted as JSON when it holds anything but
/// printable ASCII without spaces, quotes or backticks, so columns stay unambiguous and a
/// value can't end the code block.
fn column(value: &str) -> String {
    if value.is_empty() {
        "-".into()
    } else if value
        .bytes()
        .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'`')
    {
        value.into()
    } else {
        serde_json::to_string(value)
            .expect("strings serialize")
            .replace('`', "\\u0060")
    }
}

fn snake<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        _ => String::new(),
    }
}

fn title(report: &WatchReport) -> String {
    format!(
        "Watch report: {} ({}), software {}",
        one_line(&report.model),
        one_line(&report.model_part),
        one_line(&report.software_version)
    )
}

/// A busy watch's title: its product and its holder, as far as they are known.
fn busy_title(busy: &Busy) -> String {
    let watch = match (&busy.product_name, busy.product_id) {
        (Some(name), _) => one_line(name),
        (None, Some(id)) => format!("USB product {id:04x}"),
        (None, None) => "unknown model".into(),
    };
    let holder = busy
        .holder
        .name
        .as_deref()
        .map_or_else(|| snake(&busy.holder.kind), one_line);
    format!("Busy watch: {watch}, held by {holder}")
}

/// The `busy` line: `busy <kind> confirmed|running|unknown name=… pid=… product_id=<hex>
/// product=… interface=<class>/<subclass>/<protocol>`, `-` for what isn't known.
fn busy_line(busy: &Busy) -> String {
    let h = &busy.holder;
    let seen = match (h.confirmed, h.kind) {
        (true, _) => "confirmed",
        (false, HolderKind::Unknown) => "unknown",
        (false, _) => "running",
    };
    let pid = h.pid.map_or_else(|| "-".to_owned(), |p| p.to_string());
    let product_id = busy
        .product_id
        .map_or_else(|| "-".to_owned(), |p| format!("{p:04x}"));
    let interface = busy.interface.map_or_else(
        || "-".to_owned(),
        |i| format!("{:02x}/{:02x}/{:02x}", i.class, i.subclass, i.protocol),
    );
    format!(
        "busy {} {seen} name={} pid={pid} product_id={product_id} product={} interface={interface}",
        snake(&h.kind),
        column(h.name.as_deref().unwrap_or("")),
        column(busy.product_name.as_deref().unwrap_or("")),
    )
}

/// The report field's text for a busy watch that couldn't be read: this program and computer,
/// and the `busy` line.
fn busy_text(busy: &Busy) -> String {
    let p = Platform::current();
    format!(
        "app {} {} {} {}\n{}\n",
        column(APP_VERSION),
        column(&p.os),
        column(&p.os_version),
        column(&p.arch),
        busy_line(busy)
    )
}

/// The `support` line, for triage at a glance: `support expected issue=#5`, with `unlisted`
/// when the model isn't in the support list, `new_software` when an unsupported model's
/// software version isn't, and `predicted=<family>` for a model only predicted to work (an
/// `unknown` one shows the prediction was wrong).
fn support_line(support: &Support) -> String {
    let mut line = format!("support {}", snake(&support.status));
    if let Some(issue) = support.issue {
        write!(line, " issue=#{issue}").unwrap();
    }
    if !support.listed {
        line.push_str(" unlisted");
    }
    if support.status == SupportStatus::Unsupported && support.report_wanted {
        line.push_str(" new_software");
    }
    if let Some(family) = &support.prediction {
        write!(line, " predicted={}", snake(family)).unwrap();
    }
    line
}

/// The report field's text. Short words, one item per line, space-separated columns: it has
/// to fit in a URL.
pub fn text(report: &WatchReport, outcome: Option<&Outcome>, answers: Option<&Answers>) -> String {
    let mut out = String::new();
    let p = &report.platform;
    writeln!(
        out,
        "app {} {} {} {}",
        column(&report.app_version),
        column(&p.os),
        column(&p.os_version),
        column(&p.arch)
    )
    .unwrap();
    writeln!(out, "connection {}", snake(&report.connection)).unwrap();
    writeln!(
        out,
        "watch {} {} {}",
        column(&report.model),
        column(&report.model_part),
        column(&report.software_version)
    )
    .unwrap();
    writeln!(out, "{}", support_line(&report.support)).unwrap();
    let kind = report
        .hosts
        .kind
        .map_or_else(|| "none".to_owned(), |k| snake(&k));
    writeln!(out, "hosts {kind}: {}", one_line(&report.hosts.reason)).unwrap();
    if report.packs.is_empty() {
        out.push_str("packs none\n");
    } else {
        writeln!(
            out,
            "packs {} ({})",
            report.packs.join(" "),
            snake(&report.tested)
        )
        .unwrap();
    }
    // With an outcome, the state and files are from the inspect before that operation.
    let before = outcome
        .map(|o| format!(" (before {})", snake(&o.operation)))
        .unwrap_or_default();
    writeln!(out, "state {}{before}", snake(&report.state)).unwrap();
    if let Some(o) = outcome {
        let result = match (o.status, o.error) {
            (Some(status), _) => snake(&status),
            (None, Some(error)) => format!("error {}", error.as_str()),
            (None, None) => "unknown".into(),
        };
        write!(out, "last {} {result}", snake(&o.operation)).unwrap();
        for part in &o.parts {
            write!(out, " {}", column(part)).unwrap();
        }
        out.push('\n');
    }
    if let Some(a) = answers {
        let mut line = String::from("answers");
        if let Some(result) = a.result {
            write!(line, " result={}", snake(&result)).unwrap();
        }
        if !a.views.is_empty() {
            let views: Vec<String> = a.views.iter().map(snake).collect();
            write!(line, " views={}", views.join(",")).unwrap();
        }
        if let Some(phone) = a.phone {
            write!(line, " phone={}", snake(&phone)).unwrap();
        }
        if let Some(marker) = &a.marker {
            write!(line, " marker={}", column(marker)).unwrap();
        }
        if let Some(version) = a.connect_version.as_deref().map(str::trim)
            && !version.is_empty()
        {
            write!(line, " connect={}", column(version)).unwrap();
        }
        // No answers picked yet: no line.
        if line.len() > "answers".len() {
            writeln!(out, "{line}").unwrap();
        }
    }
    let marks = if report.test_marks { "beta" } else { "off" };
    writeln!(out, "marks {marks}").unwrap();
    writeln!(out, "parts{before}: part file target tested mark").unwrap();
    for part in &report.parts {
        writeln!(
            out,
            "{} {} {} {} {}",
            column(&part.part),
            snake(&part.file),
            if part.target { "yes" } else { "no" },
            snake(&part.tested),
            column(part.mark.as_deref().unwrap_or(""))
        )
        .unwrap();
    }
    out.push_str("declared: path file part version\n");
    for f in &report.declared_files {
        writeln!(
            out,
            "{} {} {} {}",
            column(&f.path),
            column(&f.file_name),
            column(&f.part_number),
            column(f.version.as_deref().unwrap_or(""))
        )
        .unwrap();
    }
    match &report.extdata_listing {
        None => out.push_str("extdata not found\n"),
        Some(files) => {
            out.push_str("extdata: name bytes sha256\n");
            for f in files {
                writeln!(
                    out,
                    "{} {} {}",
                    column(&f.name),
                    f.bytes,
                    column(f.sha256.as_deref().unwrap_or(""))
                )
                .unwrap();
            }
        }
    }
    let hidden = &report.extdata_hidden;
    match hidden.status {
        HiddenStatus::Unsupported => out.push_str("hidden: unsupported\n"),
        HiddenStatus::NotRead => out.push_str("hidden: not read\n"),
        HiddenStatus::Error if hidden.files.is_empty() => writeln!(
            out,
            "hidden: error {}",
            one_line(hidden.note.as_deref().unwrap_or(""))
        )
        .unwrap(),
        HiddenStatus::Listed | HiddenStatus::Error => {
            let files: Vec<String> = hidden
                .files
                .iter()
                .map(|f| {
                    let mut entry = format!(
                        "{} {} {}",
                        column(&f.name),
                        f.bytes,
                        match f.read_only {
                            Some(true) => "ro",
                            Some(false) => "rw",
                            None => "?",
                        }
                    );
                    if let Some(file) = f.file {
                        write!(entry, " {}", snake(&file)).unwrap();
                    }
                    entry
                })
                .collect();
            let list = if files.is_empty() {
                "none".to_owned()
            } else {
                files.join("; ")
            };
            write!(out, "hidden: {list}").unwrap();
            if let Some(note) = &hidden.note {
                write!(out, "; error {}", one_line(note)).unwrap();
            }
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> WatchReport {
        let mut r = WatchReport {
            app_version: "0.3.0-beta.1".into(),
            platform: Platform {
                os: "Mac OS".into(),
                os_version: "15.6.1".into(),
                arch: "aarch64".into(),
            },
            connection: Connection::Mtp,
            model: "Venu 2".into(),
            model_part: "006-B3703-00".into(),
            software_version: "19.05".into(),
            packs: vec!["armenian-noto-sans-regular".into()],
            tested: TestStatus::Tested,
            support: Support {
                status: SupportStatus::Tested,
                listed: true,
                issue: None,
                reason: None,
                prediction: None,
                report_wanted: false,
            },
            state: GeorgianState::NotInstalled,
            hosts: ReportedHosts {
                kind: Some(Kind::Bitmap),
                reason: "Bitmap fonts.".into(),
            },
            parts: vec![ReportedPart {
                part: "006-D9035-01".into(),
                target: true,
                file: FileState::None,
                tested: TestStatus::Tested,
                mark: Some("B".into()),
            }],
            test_marks: true,
            declared_files: vec![DeclaredFile {
                path: "Garmin/ExtData".into(),
                file_name: "006-D9035-01.tmp".into(),
                part_number: "006-D9035-01".into(),
                version: Some("2.10".into()),
            }],
            extdata_files: vec![],
            extdata_listing: Some(vec![ListedFile {
                name: "a b`.bin".into(),
                bytes: 12,
                sha256: None,
            }]),
            extdata_hidden: HiddenListing {
                status: HiddenStatus::Listed,
                files: vec![
                    HiddenFile {
                        name: "006-D9971-09.bin".into(),
                        bytes: 41124,
                        read_only: Some(true),
                        file: Some(FileState::Georgian),
                    },
                    HiddenFile {
                        name: "006-D8448-25.fnt".into(),
                        bytes: 900,
                        read_only: None,
                        file: None,
                    },
                ],
                note: None,
            },
            title: String::new(),
            text: String::new(),
        };
        r.title = title(&r);
        r.text = text(&r, None, None);
        r
    }

    fn query(url: &str) -> Vec<(String, String)> {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }

    fn input(report: WatchReport, answers: Option<Answers>) -> IssueLinkInput {
        IssueLinkInput {
            report: Some(report),
            busy: None,
            outcome: Some(Outcome {
                operation: StageOperation::Install,
                status: Some(StageStatus::StagedVerified),
                error: None,
                parts: vec!["006-D9035-01".into()],
            }),
            answers,
        }
    }

    fn busy() -> Busy {
        use crate::busy::{Holder, UsbInterface};
        Busy {
            holder: Holder::named("ptpcamerad", Some(512), true),
            product_id: Some(0x4E77),
            product_name: Some("Lily 2 Active".into()),
            interface: Some(UsbInterface {
                class: 6,
                subclass: 1,
                protocol: 1,
            }),
        }
    }

    #[test]
    fn a_busy_watch_is_reported_without_a_watch_report() {
        let link = issue_link(&IssueLinkInput {
            report: None,
            busy: Some(busy()),
            outcome: None,
            answers: None,
        })
        .unwrap();
        assert_eq!(link.title, "Busy watch: Lily 2 Active, held by ptpcamerad");
        let lines: Vec<&str> = link.report_text.lines().collect();
        assert_eq!(lines.len(), 2, "{}", link.report_text);
        assert!(lines[0].starts_with("app "), "{}", lines[0]);
        assert_eq!(
            lines[1],
            "busy macos_camera_service confirmed name=ptpcamerad pid=512 product_id=4e77 \
             product=\"Lily 2 Active\" interface=06/01/01"
        );
        assert!(link.report_in_url);
        assert!(query(&link.url).contains(&("report".into(), link.report_text.clone())));

        // Nothing known: dashes, and the kind for a title.
        let unknown = Busy::unknown();
        assert_eq!(
            busy_title(&unknown),
            "Busy watch: unknown model, held by unknown"
        );
        assert_eq!(
            busy_line(&unknown),
            "busy unknown unknown name=- pid=- product_id=- product=- interface=-"
        );
        let running = Busy {
            holder: crate::busy::Holder::named("express.exe", Some(4), false),
            ..Busy::unknown()
        };
        assert!(busy_line(&running).starts_with("busy garmin_express running name=express.exe"));
    }

    #[test]
    fn a_busy_line_follows_connection_in_a_watch_report() {
        let mut with_busy = input(report(), None);
        with_busy.busy = Some(busy());
        let text = issue_link(&with_busy).unwrap().report_text;
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[1].starts_with("connection "), "{text}");
        assert!(lines[2].starts_with("busy macos_camera_service "), "{text}");
        assert!(lines[3].starts_with("watch "), "{text}");
        let without = issue_link(&input(report(), None)).unwrap().report_text;
        assert_eq!(
            text.replace(&format!("{}\n", busy_line(&busy())), ""),
            without
        );
    }

    #[test]
    fn issue_link_needs_a_report_or_busy() {
        let mut nothing = input(report(), None);
        nothing.report = None;
        nothing.outcome = None;
        assert_eq!(
            issue_link(&nothing).unwrap_err().code,
            ErrorCode::InvalidInput
        );
        // A busy watch was never read: no outcome, no answers.
        let mut outcome = input(report(), None);
        outcome.report = None;
        outcome.busy = Some(busy());
        assert_eq!(
            issue_link(&outcome).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }

    #[test]
    fn a_report_that_fits_fills_every_field() {
        let answers = Answers {
            result: Some(Seen::Georgian),
            views: vec![View::Popup, View::Opened],
            phone: Some(Phone::Android),
            marker: Some("B".into()),
            connect_version: Some(" 5.10 ".into()),
        };
        let link = issue_link(&input(report(), Some(answers))).unwrap();
        assert!(link.report_in_url);
        assert!(link.url.starts_with(NEW_ISSUE));
        let q = query(&link.url);
        let keys: Vec<&str> = q.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            [
                "template", "title", "result", "views", "marker", "phone", "connect", "report"
            ]
        );
        assert_eq!(q[0].1, TEMPLATE);
        assert_eq!(
            q[1].1,
            "Watch report: Venu 2 (006-B3703-00), software 19.05"
        );
        assert_eq!(q[2].1, "Georgian letters");
        assert_eq!(q[3].1, "Pop-up when it arrives, Opened notification");
        assert_eq!(q[6].1, "5.10");
        assert_eq!(q[7].1, link.report_text);
        for line in [
            "app 0.3.0-beta.1 \"Mac OS\" 15.6.1 aarch64",
            "watch \"Venu 2\" 006-B3703-00 19.05",
            "support tested",
            "state not_installed (before install)",
            "last install staged_verified 006-D9035-01",
            "answers result=georgian views=popup,opened phone=android marker=B connect=5.10",
            "marks beta",
            "parts (before install): part file target tested mark",
            "006-D9035-01 none yes tested B",
            "Garmin/ExtData 006-D9035-01.tmp 006-D9035-01 2.10",
            "\"a b\\u0060.bin\" 12 -",
            "hidden: 006-D9971-09.bin 41124 ro georgian; 006-D8448-25.fnt 900 ?",
        ] {
            assert!(
                link.report_text.lines().any(|l| l == line),
                "{line:?} missing from\n{}",
                link.report_text
            );
        }
        let last = link.fields.last().unwrap();
        assert_eq!(
            (last.id.as_str(), &last.value),
            ("report", &link.report_text)
        );
    }

    #[test]
    fn a_report_too_long_for_a_link_leaves_the_report_box_empty() {
        let mut r = report();
        let file = r.declared_files[0].clone();
        r.declared_files = vec![file; 400];
        let link = issue_link(&input(r, Some(Answers::default()))).unwrap();
        assert!(!link.report_in_url);
        assert!(link.url.len() <= MAX_URL);
        let q = query(&link.url);
        let keys: Vec<&str> = q.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["template", "title"], "no report parameter: {keys:?}");
        assert!(link.fields.iter().all(|f| f.id != "report"));
        assert!(link.report_text.lines().count() > 400);
    }

    #[test]
    fn the_limit_is_on_the_encoded_url() {
        // Georgian letters take nine bytes each once percent-encoded.
        let mut r = report();
        r.model = "ქ".repeat(1000);
        assert!(text(&r, None, None).len() < MAX_URL);
        assert!(!issue_link(&input(r, None)).unwrap().report_in_url);
    }

    #[test]
    fn an_outcome_has_a_status_or_an_error() {
        let mut i = input(report(), None);
        i.outcome.as_mut().unwrap().error = Some(ErrorCode::Conflict);
        assert_eq!(issue_link(&i).unwrap_err().code, ErrorCode::InvalidInput);
        let o = i.outcome.as_mut().unwrap();
        o.status = None;
        o.parts.clear();
        let link = issue_link(&i).unwrap();
        assert!(link.report_text.contains("\nlast install error conflict\n"));
    }

    /// The link's field ids and option labels are the issue form's.
    #[test]
    fn labels_match_the_issue_form() {
        let form = include_str!("../../../.github/ISSUE_TEMPLATE/watch-report.yml");
        for id in [
            "result", "views", "marker", "phone", "connect", "report", "notes",
        ] {
            assert!(form.contains(&format!("id: {id}\n")), "form lacks id {id}");
        }
        let labels = Seen::ALL
            .iter()
            .map(|s| s.label())
            .chain([View::Popup, View::List, View::Opened].map(View::label))
            .chain([Phone::Android, Phone::Iphone].map(Phone::label))
            .chain([marker_label(NO_MARKER)]);
        for label in labels {
            let option = format!("- \"{label}\"\n");
            let checkbox = format!("- label: \"{label}\"\n");
            assert!(
                form.contains(&option) || form.contains(&checkbox),
                "form lacks option {label:?}"
            );
        }
        for variant in 'A'..='T' {
            assert!(form.contains(&format!("- \"{variant}\"\n")), "{variant}");
        }
    }
}

#[cfg(test)]
mod marks_off {
    use super::*;

    #[test]
    fn a_stable_report_says_marks_off() {
        let device = crate::garmin_device::parse(
            b"<Device><Model><PartNumber>006-B3703-00</PartNumber>\
              <SoftwareVersion>1905</SoftwareVersion><Description>Venu 2</Description></Model>\
              </Device>",
        )
        .unwrap();
        let report = watch_report(
            &device,
            ReportInput {
                connection: Connection::Mtp,
                packs: vec![],
                tested: TestStatus::Untested,
                support: Support::default(),
                state: GeorgianState::Unsupported,
                hosts: ReportedHosts {
                    kind: None,
                    reason: "None.".into(),
                },
                parts: vec![],
                test_marks: false,
                extdata_listing: None,
                extdata_hidden: HiddenListing {
                    status: HiddenStatus::Error,
                    files: vec![],
                    note: Some("GetObjectIds failed".into()),
                },
            },
        );
        assert!(
            report
                .text
                .lines()
                .any(|l| l == "hidden: error GetObjectIds failed"),
            "{}",
            report.text
        );
        assert!(
            report.text.lines().any(|l| l == "marks off"),
            "{}",
            report.text
        );
        assert!(report.text.lines().any(|l| l == "state unsupported"));
        assert!(
            report.text.lines().any(|l| l == "support unknown unlisted"),
            "{}",
            report.text
        );
    }

    #[test]
    fn the_support_line_has_status_issue_and_what_is_new() {
        let line = |status, listed, issue, report_wanted| {
            support_line(&Support {
                status,
                listed,
                issue,
                reason: None,
                prediction: None,
                report_wanted,
            })
        };
        use SupportStatus as S;
        assert_eq!(line(S::Tested, true, None, false), "support tested");
        assert_eq!(
            line(S::Expected, true, Some(5), true),
            "support expected issue=#5"
        );
        assert_eq!(
            line(S::Expected, false, None, true),
            "support expected unlisted"
        );
        assert_eq!(
            line(S::Unsupported, true, Some(16), false),
            "support unsupported issue=#16"
        );
        assert_eq!(
            line(S::Unsupported, true, Some(16), true),
            "support unsupported issue=#16 new_software"
        );
        assert_eq!(
            line(S::Unknown, false, None, true),
            "support unknown unlisted"
        );
        let predicted = Support {
            listed: true,
            prediction: Some(crate::support::PredictionFamily::BitmapArmenian),
            ..Support::default()
        };
        assert_eq!(
            support_line(&predicted),
            "support unknown predicted=bitmap_armenian"
        );
    }
}
