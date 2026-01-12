use crate::gitlab::{Job, MergeRequest, Note, Pipeline};

#[derive(Debug, Clone)]
pub enum Action {
    // User Input Actions
    Quit,
    NextMr,
    PrevMr,
    NextJob,
    PrevJob,
    NextPipeline,
    PrevPipeline,
    SelectMr,
    OpenSelectedJobLog,
    Refresh,
    RemoveCurrentMr,
    AddMr(u64), // Add MR by IID
    ShowHelp,
    HideHelp,
    ToggleCommentsView,
    NextNote,
    PrevNote,
    CloseLogViewer,
    ScrollLogUp,
    ScrollLogDown,
    ScrollLogPageUp,
    ScrollLogPageDown,
    ScrollLogHome,
    ScrollLogEnd,
    ToggleTimestampMode,
    StartSearch,
    UpdateSearchQuery(String),
    ExecuteSearch,
    NextSearchResult,
    PrevSearchResult,
    CancelSearch,
    OpenMrInBrowser,

    // API Response Actions
    MergeRequestsLoaded(Vec<MergeRequest>),
    PipelinesLoaded {
        mr_index: usize,
        pipelines: Vec<Pipeline>,
    },
    JobsLoaded {
        mr_index: usize,
        pipeline_id: u64,
        jobs: Vec<Job>,
    },
    JobTraceLoaded {
        job_id: u64,
        job_name: String,
        trace: String,
    },
    NotesLoaded {
        mr_index: usize,
        notes: Vec<Note>,
    },

    // Error Actions
    ApiError(String),

    // Tick for auto-refresh
    Tick,

    // No-op
    None,
}

#[derive(Debug, Clone)]
pub enum Effect {
    FetchMergeRequests { project_id: u64 },
    FetchMergeRequestsByBranch { project_id: u64, source_branch: String },
    FetchPipelines { mr_index: usize, project_id: u64, mr_iid: u64 },
    FetchJobs { mr_index: usize, project_id: u64, pipeline_id: u64 },
    FetchJobTrace { project_id: u64, job_id: u64, job_name: String },
    FetchNotes { mr_index: usize, project_id: u64, mr_iid: u64 },
    OpenInEditor(String),
    RefreshAll { project_id: u64, source_branch: Option<String> },
    OpenUrl(String),
}
