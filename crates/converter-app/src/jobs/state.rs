use crate::JobState;
pub fn active(state: JobState) -> bool {
    matches!(
        state,
        JobState::Downloading
            | JobState::DownloadRetryWait
            | JobState::Importing
            | JobState::Inspecting
            | JobState::Converting
            | JobState::Validating
            | JobState::Publishing
    )
}
pub fn success(state: JobState) -> bool {
    matches!(state, JobState::Succeeded | JobState::SucceededWithWarnings)
}
