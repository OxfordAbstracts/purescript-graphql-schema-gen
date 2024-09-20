use phf::phf_map;

type Table = phf::Map<&'static str, (&'static str, &'static str)>;
type OutsideTypes = phf::Map<&'static str, &'static Table>;
type Id = (&'static str, &'static str);

static USER_ID: Id = ("Data.Id.UserId", "UserId");
static EVENT_ID: Id = ("Data.Id.EventId", "EventId");
static SUBMISSION_ID: Id = ("Data.Id.SubmissionId", "SubmissionId");
static SYMPOSIUM_ID: Id = ("Data.Id.SymposiumId", "SymposiumId");
static SUBMISSION_SERIAL_NUMBER: Id = ("Data.Id.SubmissionSerialNumber", "SubmissionSerialNumber");

pub static OUTSIDE_TYPES: OutsideTypes = phf_map! {
    // tables
    "submissions" => &SUBMISSIONS,
    "users" => &USERS,
    // actions
    "OnboardingCreateEvent" => &ONBOARDING_CREATE_EVENT,
    "OnboardingCreateClient" => &ONBOARDING_CREATE_CLIENT,
};

static ONBOARDING_CREATE_EVENT: Table = phf_map! {
    "frequency" => ("GeneratedPostgres.Enum.FrequencyType", "FrequencyType"),
    "user_id" => USER_ID,
};
static ONBOARDING_CREATE_CLIENT: Table = phf_map! {
    "region" => ("GqlOverrides.ClientRegionActionInput", "ClientRegionActionInput"),
};
static SUBMISSIONS: Table = phf_map! {
  "event_id" => EVENT_ID,
  "submission_id" => SUBMISSION_ID,
  // "symposium_id" => SYMPOSIUM_ID,
  // "serial_number" => SUBMISSION_SERIAL_NUMBER,
};
static USERS: Table = phf_map! {
  "user_id" => USER_ID,
};
