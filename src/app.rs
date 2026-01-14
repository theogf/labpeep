use crate::events::actions::{Action, Effect};
use crate::gitlab::{Job, JobStatus, MergeRequest, Note, Pipeline};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum TimestampDisplayMode {
    Hidden,      // Don't show timestamps
    DateOnly,    // Show date only (e.g., "2024-01-15")
    Full,        // Show full timestamp (e.g., "2024-01-15 10:30:45")
}

pub struct App {
    // UI State
    pub should_quit: bool,
    pub selected_mr_index: usize,
    pub selected_job_index: usize,

    // Data State
    pub tracked_mrs: Vec<TrackedMergeRequest>,
    pub project_id: u64,
    pub current_branch: Option<String>,
    pub focus_current_branch: bool,

    // UI Modes
    pub mode: AppMode,

    // Log Viewer State
    pub log_content: Option<String>,
    pub log_processed_lines: Vec<ratatui::text::Line<'static>>, // Cached processed lines
    pub log_scroll_offset: usize,
    pub log_viewport_height: usize, // Height of visible log area (set by renderer)
    pub log_job_name: Option<String>,
    pub timestamp_mode: TimestampDisplayMode,
    pub search_query: String,
    pub search_results: Vec<usize>, // Line numbers where matches are found
    pub current_search_result: usize, // Index into search_results
    pub is_searching: bool, // Whether in search input mode

    // Status
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub last_refresh: Option<chrono::DateTime<chrono::Utc>>,

    // Auto-refresh
    pub last_auto_refresh: Instant,
    pub auto_refresh_interval_minutes: u64,
    pub refetch_notes_after_refresh: bool, // Flag to refetch notes after refresh completes
}

#[derive(Debug, Clone)]
pub struct TrackedMergeRequest {
    pub mr: MergeRequest,
    pub pipelines: Vec<Pipeline>,
    pub jobs: HashMap<u64, Vec<Job>>, // pipeline_id -> jobs
    pub job_logs_cache: HashMap<u64, String>, // job_id -> cached log content
    pub notes: Vec<Note>,              // MR comments/notes
    pub notes_loaded: bool,            // Track if notes have been fetched
    pub selected_pipeline_index: usize,
    pub selected_note_index: usize,    // Track selected comment for navigation
    pub loading: bool,
    #[allow(dead_code)]
    pub error: Option<String>,         // Reserved for future per-MR error tracking
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,           // Viewing MRs and jobs
    ViewingComments,  // Viewing MR comments instead of jobs
    ViewingLog,       // Viewing job log internally
    SelectingMr,      // MR selection dialog
    ShowingHelp,      // Help popup visible
}

impl App {
    pub fn new(project_id: u64, current_branch: Option<String>, focus_current_branch: bool, auto_refresh_interval_minutes: u64) -> Self {
        let status_message = if focus_current_branch && current_branch.is_some() {
            Some(format!("Loading MR for branch '{}'...", current_branch.as_ref().unwrap()))
        } else {
            Some("Loading merge requests...".to_string())
        };

        Self {
            should_quit: false,
            selected_mr_index: 0,
            selected_job_index: 0,
            tracked_mrs: Vec::new(),
            project_id,
            current_branch,
            focus_current_branch,
            mode: AppMode::Normal,
            log_content: None,
            log_processed_lines: Vec::new(),
            log_scroll_offset: 0,
            log_viewport_height: 30, // Default, will be updated by renderer
            log_job_name: None,
            timestamp_mode: TimestampDisplayMode::Hidden,
            search_query: String::new(),
            search_results: Vec::new(),
            current_search_result: 0,
            is_searching: false,
            status_message,
            error_message: None,
            last_refresh: None,
            last_auto_refresh: Instant::now(),
            auto_refresh_interval_minutes,
            refetch_notes_after_refresh: false,
        }
    }

    pub fn get_selected_mr(&self) -> Option<&TrackedMergeRequest> {
        self.tracked_mrs.get(self.selected_mr_index)
    }

    pub fn get_selected_mr_mut(&mut self) -> Option<&mut TrackedMergeRequest> {
        self.tracked_mrs.get_mut(self.selected_mr_index)
    }

    pub fn get_selected_pipeline(&self) -> Option<&Pipeline> {
        self.get_selected_mr()
            .and_then(|mr| mr.pipelines.get(mr.selected_pipeline_index))
    }

    pub fn get_selected_jobs(&self) -> Option<&[Job]> {
        if let Some(mr) = self.get_selected_mr() {
            if let Some(pipeline) = mr.pipelines.get(mr.selected_pipeline_index) {
                return mr.jobs.get(&pipeline.id).map(|jobs| jobs.as_slice());
            }
        }
        None
    }

    pub fn get_selected_notes(&self) -> Option<&[Note]> {
        self.get_selected_mr()
            .map(|mr| mr.notes.as_slice())
    }

    pub fn is_viewing_comments(&self) -> bool {
        self.mode == AppMode::ViewingComments
    }

    /// Center a line in the log viewer viewport
    fn center_log_line(&mut self, line_number: usize) {
        let total_lines = self.log_processed_lines.len();
        if total_lines == 0 {
            return;
        }

        // Calculate offset to center the line
        let half_viewport = self.log_viewport_height / 2;

        // Try to position the line in the middle
        if line_number >= half_viewport {
            self.log_scroll_offset = line_number - half_viewport;
        } else {
            // If line is near the top, just show from the beginning
            self.log_scroll_offset = 0;
        }

        // Don't scroll past the end
        let max_offset = total_lines.saturating_sub(self.log_viewport_height);
        self.log_scroll_offset = self.log_scroll_offset.min(max_offset);
    }

    pub fn update(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::Quit => {
                self.should_quit = true;
                None
            }

            Action::NextMr => {
                if !self.tracked_mrs.is_empty() {
                    self.selected_mr_index = (self.selected_mr_index + 1) % self.tracked_mrs.len();
                    self.selected_job_index = 0;
                }
                None
            }

            Action::PrevMr => {
                if !self.tracked_mrs.is_empty() {
                    self.selected_mr_index = self
                        .selected_mr_index
                        .checked_sub(1)
                        .unwrap_or(self.tracked_mrs.len() - 1);
                    self.selected_job_index = 0;
                }
                None
            }

            Action::NextJob => {
                if let Some(jobs) = self.get_selected_jobs() {
                    if !jobs.is_empty() {
                        self.selected_job_index = (self.selected_job_index + 1) % jobs.len();
                    }
                }
                None
            }

            Action::PrevJob => {
                if let Some(jobs) = self.get_selected_jobs() {
                    if !jobs.is_empty() {
                        self.selected_job_index = self
                            .selected_job_index
                            .checked_sub(1)
                            .unwrap_or(jobs.len() - 1);
                    }
                }
                None
            }

            Action::NextPipeline => {
                let mr_index = self.selected_mr_index;
                let project_id = self.project_id;

                if let Some(mr) = self.tracked_mrs.get_mut(mr_index) {
                    if !mr.pipelines.is_empty() {
                        mr.selected_pipeline_index =
                            (mr.selected_pipeline_index + 1) % mr.pipelines.len();

                        // Fetch jobs for this pipeline if we don't have them yet
                        if let Some(pipeline) = mr.pipelines.get(mr.selected_pipeline_index) {
                            let pipeline_id = pipeline.id;
                            let needs_fetch = !mr.jobs.contains_key(&pipeline_id);

                            // Drop the borrow so we can modify self
                            let _ = mr;
                            self.selected_job_index = 0;

                            if needs_fetch {
                                return Some(Effect::FetchJobs {
                                    mr_index,
                                    project_id,
                                    pipeline_id,
                                });
                            }
                        }
                    }
                }

                self.selected_job_index = 0;
                None
            }

            Action::PrevPipeline => {
                let mr_index = self.selected_mr_index;
                let project_id = self.project_id;

                if let Some(mr) = self.tracked_mrs.get_mut(mr_index) {
                    if !mr.pipelines.is_empty() {
                        mr.selected_pipeline_index = mr
                            .selected_pipeline_index
                            .checked_sub(1)
                            .unwrap_or(mr.pipelines.len() - 1);

                        // Fetch jobs for this pipeline if we don't have them yet
                        if let Some(pipeline) = mr.pipelines.get(mr.selected_pipeline_index) {
                            let pipeline_id = pipeline.id;
                            let needs_fetch = !mr.jobs.contains_key(&pipeline_id);

                            // Drop the borrow so we can modify self
                            let _ = mr;
                            self.selected_job_index = 0;

                            if needs_fetch {
                                return Some(Effect::FetchJobs {
                                    mr_index,
                                    project_id,
                                    pipeline_id,
                                });
                            }
                        }
                    }
                }

                self.selected_job_index = 0;
                None
            }

            Action::OpenSelectedJobLog => {
                let job_info = self.get_selected_jobs()
                    .and_then(|jobs| jobs.get(self.selected_job_index))
                    .map(|job| (job.name.clone(), job.id));

                if let Some((job_name, job_id)) = job_info {
                    // Check if log is already cached
                    if let Some(mr) = self.tracked_mrs.get(self.selected_mr_index) {
                        if let Some(cached_log) = mr.job_logs_cache.get(&job_id) {
                            // Use cached log
                            self.status_message = None;
                            self.log_processed_lines = crate::log_processor::process_log_content(cached_log, &self.timestamp_mode);
                            self.log_content = Some(cached_log.clone());
                            self.log_job_name = Some(job_name);
                            self.log_scroll_offset = 0;
                            self.mode = AppMode::ViewingLog;
                            return None;
                        }
                    }

                    // Not cached, fetch from API
                    self.status_message = Some(format!("Fetching log for job '{}'...", job_name));
                    return Some(Effect::FetchJobTrace {
                        project_id: self.project_id,
                        job_id,
                        job_name,
                    });
                }
                None
            }

            Action::Refresh => {
                // Reset auto-refresh timer on manual refresh
                self.last_auto_refresh = Instant::now();

                // Set flag to refetch notes after refresh if currently viewing comments
                self.refetch_notes_after_refresh = self.mode == AppMode::ViewingComments;

                // Clear all cached data including notes and job logs
                for mr in &mut self.tracked_mrs {
                    mr.notes_loaded = false;
                    mr.notes.clear();
                    mr.job_logs_cache.clear();
                }

                self.status_message = Some("Refreshing...".to_string());
                Some(Effect::RefreshAll {
                    project_id: self.project_id,
                    source_branch: if self.focus_current_branch {
                        self.current_branch.clone()
                    } else {
                        None
                    },
                })
            }

            Action::RemoveCurrentMr => {
                if !self.tracked_mrs.is_empty() {
                    self.tracked_mrs.remove(self.selected_mr_index);
                    if self.selected_mr_index > 0 {
                        self.selected_mr_index -= 1;
                    }
                    self.selected_job_index = 0;
                }
                None
            }

            Action::MergeRequestsLoaded(mrs) => {
                // Initialize tracked MRs with the loaded data
                for mr in mrs {
                    if !self.tracked_mrs.iter().any(|tmr| tmr.mr.iid == mr.iid) {
                        let tracked_mr = TrackedMergeRequest {
                            mr: mr.clone(),
                            pipelines: Vec::new(),
                            jobs: HashMap::new(),
                            job_logs_cache: HashMap::new(),
                            notes: Vec::new(),
                            notes_loaded: false,
                            selected_pipeline_index: 0,
                            selected_note_index: 0,
                            loading: true,
                            error: None,
                        };
                        self.tracked_mrs.push(tracked_mr);
                    }
                }

                self.status_message = Some(format!("Loaded {} merge requests", self.tracked_mrs.len()));

                // Fetch pipelines for each MR
                let effects: Vec<Effect> = self
                    .tracked_mrs
                    .iter()
                    .enumerate()
                    .map(|(index, tmr)| Effect::FetchPipelines {
                        mr_index: index,
                        project_id: self.project_id,
                        mr_iid: tmr.mr.iid,
                    })
                    .collect();

                // Return the first effect; in a real implementation, we'd handle multiple
                effects.into_iter().next()
            }

            Action::PipelinesLoaded { mr_index, pipelines } => {
                if let Some(mr) = self.tracked_mrs.get_mut(mr_index) {
                    mr.pipelines = pipelines;
                    mr.loading = false;

                    // Check if we need to refetch notes after refresh (only for selected MR)
                    if self.refetch_notes_after_refresh && mr_index == self.selected_mr_index {
                        self.refetch_notes_after_refresh = false;
                        self.status_message = Some("Reloading comments...".to_string());
                        return Some(Effect::FetchNotes {
                            mr_index,
                            project_id: self.project_id,
                            mr_iid: mr.mr.iid,
                        });
                    }

                    // Fetch jobs for the latest pipeline
                    if let Some(pipeline) = mr.pipelines.first() {
                        return Some(Effect::FetchJobs {
                            mr_index,
                            project_id: self.project_id,
                            pipeline_id: pipeline.id,
                        });
                    }
                }
                None
            }

            Action::JobsLoaded {
                mr_index,
                pipeline_id,
                mut jobs,
            } => {
                if let Some(mr) = self.tracked_mrs.get_mut(mr_index) {
                    // Sort jobs: failed first, then running, pending, etc.
                    jobs.sort_by_key(|job| match job.status {
                        JobStatus::Failed => 0,
                        JobStatus::Running => 1,
                        JobStatus::Pending => 2,
                        JobStatus::Canceled => 3,
                        JobStatus::Created => 4,
                        JobStatus::Manual => 5,
                        JobStatus::Success => 6,
                        JobStatus::Skipped => 7,
                    });
                    mr.jobs.insert(pipeline_id, jobs);
                }
                self.last_refresh = Some(chrono::Utc::now());
                None
            }

            Action::JobTraceLoaded { job_id, job_name, trace } => {
                self.status_message = None;

                // Cache the log in the current MR
                if let Some(mr) = self.tracked_mrs.get_mut(self.selected_mr_index) {
                    mr.job_logs_cache.insert(job_id, trace.clone());
                }

                // Process all lines upfront for fast rendering
                self.log_processed_lines = crate::log_processor::process_log_content(&trace, &self.timestamp_mode);
                self.log_content = Some(trace);
                self.log_job_name = Some(job_name);
                self.log_scroll_offset = 0;
                self.mode = AppMode::ViewingLog;
                None
            }

            Action::CloseLogViewer => {
                self.mode = AppMode::Normal;
                self.log_content = None;
                self.log_processed_lines.clear();
                self.log_job_name = None;
                self.log_scroll_offset = 0;
                self.search_query.clear();
                self.search_results.clear();
                self.current_search_result = 0;
                self.is_searching = false;
                None
            }

            Action::ScrollLogUp => {
                if self.mode == AppMode::ViewingLog {
                    self.log_scroll_offset = self.log_scroll_offset.saturating_sub(1);
                }
                None
            }

            Action::ScrollLogDown => {
                if self.mode == AppMode::ViewingLog {
                    self.log_scroll_offset = self.log_scroll_offset.saturating_add(1);
                }
                None
            }

            Action::ScrollLogPageUp => {
                if self.mode == AppMode::ViewingLog {
                    self.log_scroll_offset = self.log_scroll_offset.saturating_sub(10);
                }
                None
            }

            Action::ScrollLogPageDown => {
                if self.mode == AppMode::ViewingLog {
                    self.log_scroll_offset = self.log_scroll_offset.saturating_add(10);
                }
                None
            }

            Action::ScrollLogHome => {
                if self.mode == AppMode::ViewingLog {
                    self.log_scroll_offset = 0;
                }
                None
            }

            Action::ScrollLogEnd => {
                if self.mode == AppMode::ViewingLog {
                    if let Some(content) = &self.log_content {
                        let total_lines = content.lines().count();
                        self.log_scroll_offset = total_lines.saturating_sub(1);
                    }
                }
                None
            }

            Action::ToggleTimestampMode => {
                if self.mode == AppMode::ViewingLog {
                    self.timestamp_mode = match self.timestamp_mode {
                        TimestampDisplayMode::Hidden => TimestampDisplayMode::DateOnly,
                        TimestampDisplayMode::DateOnly => TimestampDisplayMode::Full,
                        TimestampDisplayMode::Full => TimestampDisplayMode::Hidden,
                    };
                    // Reprocess lines with new timestamp mode
                    if let Some(ref content) = self.log_content {
                        self.log_processed_lines = crate::log_processor::process_log_content(content, &self.timestamp_mode);
                    }
                }
                None
            }

            Action::StartSearch => {
                if self.mode == AppMode::ViewingLog {
                    self.is_searching = true;
                    self.search_query.clear();
                }
                None
            }

            Action::UpdateSearchQuery(query) => {
                if self.is_searching {
                    self.search_query = query;
                }
                None
            }

            Action::ExecuteSearch => {
                if let Some(content) = &self.log_content {
                    self.search_results.clear();

                    if !self.search_query.is_empty() {
                        // Find all lines containing the search query (case-insensitive)
                        let query_lower = self.search_query.to_lowercase();
                        for (idx, line) in content.lines().enumerate() {
                            if line.to_lowercase().contains(&query_lower) {
                                self.search_results.push(idx);
                            }
                        }
                    }

                    self.is_searching = false;
                    self.current_search_result = 0;

                    // Jump to first result if any, centered in viewport
                    if !self.search_results.is_empty() {
                        self.center_log_line(self.search_results[0]);
                    }
                }
                None
            }

            Action::NextSearchResult => {
                if !self.search_results.is_empty() && self.mode == AppMode::ViewingLog {
                    self.current_search_result = (self.current_search_result + 1) % self.search_results.len();
                    self.center_log_line(self.search_results[self.current_search_result]);
                }
                None
            }

            Action::PrevSearchResult => {
                if !self.search_results.is_empty() && self.mode == AppMode::ViewingLog {
                    self.current_search_result = if self.current_search_result == 0 {
                        self.search_results.len() - 1
                    } else {
                        self.current_search_result - 1
                    };
                    self.center_log_line(self.search_results[self.current_search_result]);
                }
                None
            }

            Action::CancelSearch => {
                self.is_searching = false;
                self.search_query.clear();
                None
            }

            Action::ApiError(error) => {
                self.error_message = Some(error.clone());
                self.status_message = None;
                None
            }

            Action::ShowHelp => {
                self.mode = AppMode::ShowingHelp;
                None
            }

            Action::HideHelp => {
                self.mode = AppMode::Normal;
                None
            }

            Action::ToggleCommentsView => {
                self.mode = match self.mode {
                    AppMode::ViewingComments => AppMode::Normal,
                    AppMode::Normal => {
                        // Check if we need to fetch notes
                        if let Some(mr) = self.get_selected_mr() {
                            if !mr.notes_loaded {
                                let mr_index = self.selected_mr_index;
                                let project_id = self.project_id;
                                let mr_iid = mr.mr.iid;

                                self.status_message = Some("Loading comments...".to_string());
                                self.mode = AppMode::ViewingComments;

                                return Some(Effect::FetchNotes {
                                    mr_index,
                                    project_id,
                                    mr_iid,
                                });
                            }
                        }
                        AppMode::ViewingComments
                    }
                    _ => self.mode.clone(), // Don't toggle in other modes
                };
                None
            }

            Action::NotesLoaded { mr_index, notes } => {
                if let Some(mr) = self.tracked_mrs.get_mut(mr_index) {
                    mr.notes = notes;
                    mr.notes_loaded = true;
                    mr.selected_note_index = 0;

                    // After notes are loaded following a refresh, continue to fetch jobs
                    if let Some(pipeline) = mr.pipelines.first() {
                        self.status_message = None;
                        return Some(Effect::FetchJobs {
                            mr_index,
                            project_id: self.project_id,
                            pipeline_id: pipeline.id,
                        });
                    }
                }
                self.status_message = None;
                None
            }

            Action::NextNote => {
                if self.mode == AppMode::ViewingComments {
                    // Get the length of user notes (excluding system notes)
                    let user_notes_len = self
                        .get_selected_notes()
                        .map(|notes| notes.iter().filter(|n| !n.system).count())
                        .unwrap_or(0);
                    if user_notes_len > 0 {
                        if let Some(mr) = self.tracked_mrs.get_mut(self.selected_mr_index) {
                            mr.selected_note_index = (mr.selected_note_index + 1) % user_notes_len;
                        }
                    }
                }
                None
            }

            Action::PrevNote => {
                if self.mode == AppMode::ViewingComments {
                    // Get the length of user notes (excluding system notes)
                    let user_notes_len = self
                        .get_selected_notes()
                        .map(|notes| notes.iter().filter(|n| !n.system).count())
                        .unwrap_or(0);
                    if user_notes_len > 0 {
                        if let Some(mr) = self.tracked_mrs.get_mut(self.selected_mr_index) {
                            mr.selected_note_index = mr
                                .selected_note_index
                                .checked_sub(1)
                                .unwrap_or(user_notes_len - 1);
                        }
                    }
                }
                None
            }

            Action::OpenMrInBrowser => {
                if let Some(mr) = self.get_selected_mr() {
                    return Some(Effect::OpenUrl(mr.mr.web_url.clone()));
                }
                None
            }

            Action::Tick => {
                // Check if it's time for an auto-refresh
                let elapsed = self.last_auto_refresh.elapsed();
                let refresh_interval = std::time::Duration::from_secs(self.auto_refresh_interval_minutes * 60);

                if elapsed >= refresh_interval {
                    // Trigger auto-refresh
                    self.last_auto_refresh = Instant::now();

                    // Set flag to refetch notes after refresh if currently viewing comments
                    self.refetch_notes_after_refresh = self.mode == AppMode::ViewingComments;

                    // Clear all cached data including notes and job logs
                    for mr in &mut self.tracked_mrs {
                        mr.notes_loaded = false;
                        mr.notes.clear();
                        mr.job_logs_cache.clear();
                    }

                    self.status_message = Some("Auto-refreshing...".to_string());
                    Some(Effect::RefreshAll {
                        project_id: self.project_id,
                        source_branch: if self.focus_current_branch {
                            self.current_branch.clone()
                        } else {
                            None
                        },
                    })
                } else {
                    None
                }
            }

            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitlab::{JobStatus, PipelineStatus};
    use crate::gitlab::models::User;
    use chrono::Utc;

    fn create_test_mr(id: u64, iid: u64, title: &str) -> MergeRequest {
        MergeRequest {
            id,
            iid,
            title: title.to_string(),
            author: User {
                id: 1,
                username: "testuser".to_string(),
                name: "Test User".to_string(),
            },
            state: "opened".to_string(),
            web_url: format!("https://gitlab.com/test/-/merge_requests/{}", iid),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_pipeline(id: u64, status: PipelineStatus) -> Pipeline {
        Pipeline {
            id,
            iid: id,
            status,
            ref_name: "main".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            web_url: format!("https://gitlab.com/test/-/pipelines/{}", id),
        }
    }

    fn create_test_job(id: u64, name: &str, status: JobStatus) -> Job {
        Job {
            id,
            name: name.to_string(),
            status,
            stage: "test".to_string(),
            created_at: Utc::now(),
            started_at: Some(Utc::now()),
            finished_at: Some(Utc::now()),
            duration: Some(120.0),
            web_url: format!("https://gitlab.com/test/-/jobs/{}", id),
        }
    }

    #[test]
    fn test_app_new() {
        let app = App::new(123, None, false, 1);
        assert_eq!(app.project_id, 123);
        assert!(!app.should_quit);
        assert_eq!(app.selected_mr_index, 0);
        assert_eq!(app.selected_job_index, 0);
        assert!(app.tracked_mrs.is_empty());
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_quit_action() {
        let mut app = App::new(123, None, false, 1);
        assert!(!app.should_quit);

        app.update(Action::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn test_next_mr() {
        let mut app = App::new(123, None, false, 1);

        // Add some MRs
        let mr1 = create_test_mr(1, 10, "MR 1");
        let mr2 = create_test_mr(2, 20, "MR 2");

        app.tracked_mrs.push(TrackedMergeRequest {
            mr: mr1,
            pipelines: vec![],
            jobs: HashMap::new(),
            job_logs_cache: HashMap::new(),
            notes: Vec::new(),
            notes_loaded: false,
            selected_pipeline_index: 0,
            selected_note_index: 0,
            loading: false,
            error: None,
        });

        app.tracked_mrs.push(TrackedMergeRequest {
            mr: mr2,
            pipelines: vec![],
            jobs: HashMap::new(),
            job_logs_cache: HashMap::new(),
            notes: Vec::new(),
            notes_loaded: false,
            selected_pipeline_index: 0,
            selected_note_index: 0,
            loading: false,
            error: None,
        });

        assert_eq!(app.selected_mr_index, 0);
        app.update(Action::NextMr);
        assert_eq!(app.selected_mr_index, 1);
        app.update(Action::NextMr);
        assert_eq!(app.selected_mr_index, 0); // Wraps around
    }

    #[test]
    fn test_prev_mr() {
        let mut app = App::new(123, None, false, 1);

        let mr1 = create_test_mr(1, 10, "MR 1");
        let mr2 = create_test_mr(2, 20, "MR 2");

        app.tracked_mrs.push(TrackedMergeRequest {
            mr: mr1,
            pipelines: vec![],
            jobs: HashMap::new(),
            job_logs_cache: HashMap::new(),
            notes: Vec::new(),
            notes_loaded: false,
            selected_pipeline_index: 0,
            selected_note_index: 0,
            loading: false,
            error: None,
        });

        app.tracked_mrs.push(TrackedMergeRequest {
            mr: mr2,
            pipelines: vec![],
            jobs: HashMap::new(),
            job_logs_cache: HashMap::new(),
            notes: Vec::new(),
            notes_loaded: false,
            selected_pipeline_index: 0,
            selected_note_index: 0,
            loading: false,
            error: None,
        });

        assert_eq!(app.selected_mr_index, 0);
        app.update(Action::PrevMr);
        assert_eq!(app.selected_mr_index, 1); // Wraps around to end
        app.update(Action::PrevMr);
        assert_eq!(app.selected_mr_index, 0);
    }

    #[test]
    fn test_merge_requests_loaded() {
        let mut app = App::new(123, None, false, 1);

        let mrs = vec![
            create_test_mr(1, 10, "MR 1"),
            create_test_mr(2, 20, "MR 2"),
        ];

        app.update(Action::MergeRequestsLoaded(mrs));
        assert_eq!(app.tracked_mrs.len(), 2);
        assert_eq!(app.tracked_mrs[0].mr.title, "MR 1");
        assert_eq!(app.tracked_mrs[1].mr.title, "MR 2");
    }

    #[test]
    fn test_pipelines_loaded() {
        let mut app = App::new(123, None, false, 1);

        let mr = create_test_mr(1, 10, "Test MR");
        app.tracked_mrs.push(TrackedMergeRequest {
            mr,
            pipelines: vec![],
            jobs: HashMap::new(),
            job_logs_cache: HashMap::new(),
            notes: Vec::new(),
            notes_loaded: false,
            selected_pipeline_index: 0,
            selected_note_index: 0,
            loading: true,
            error: None,
        });

        let pipelines = vec![
            create_test_pipeline(100, PipelineStatus::Success),
            create_test_pipeline(101, PipelineStatus::Failed),
        ];

        app.update(Action::PipelinesLoaded {
            mr_index: 0,
            pipelines,
        });

        assert_eq!(app.tracked_mrs[0].pipelines.len(), 2);
        assert!(!app.tracked_mrs[0].loading);
        assert_eq!(app.tracked_mrs[0].pipelines[0].status, PipelineStatus::Success);
    }

    #[test]
    fn test_jobs_loaded() {
        let mut app = App::new(123, None, false, 1);

        let mr = create_test_mr(1, 10, "Test MR");
        let pipeline = create_test_pipeline(100, PipelineStatus::Running);

        app.tracked_mrs.push(TrackedMergeRequest {
            mr,
            pipelines: vec![pipeline],
            jobs: HashMap::new(),
            job_logs_cache: HashMap::new(),
            notes: Vec::new(),
            notes_loaded: false,
            selected_pipeline_index: 0,
            selected_note_index: 0,
            loading: false,
            error: None,
        });

        let jobs = vec![
            create_test_job(200, "build", JobStatus::Success),
            create_test_job(201, "test", JobStatus::Failed),
        ];

        app.update(Action::JobsLoaded {
            mr_index: 0,
            pipeline_id: 100,
            jobs,
        });

        assert!(app.tracked_mrs[0].jobs.contains_key(&100));
        let loaded_jobs = &app.tracked_mrs[0].jobs[&100];
        assert_eq!(loaded_jobs.len(), 2);
        // Jobs are sorted by status: Failed jobs come first
        assert_eq!(loaded_jobs[0].name, "test"); // Failed
        assert_eq!(loaded_jobs[1].name, "build"); // Success
    }

    #[test]
    fn test_api_error() {
        let mut app = App::new(123, None, false, 1);

        app.update(Action::ApiError("Test error".to_string()));
        assert_eq!(app.error_message, Some("Test error".to_string()));
        assert!(app.status_message.is_none());
    }

    #[test]
    fn test_remove_current_mr() {
        let mut app = App::new(123, None, false, 1);

        let mr1 = create_test_mr(1, 10, "MR 1");
        let mr2 = create_test_mr(2, 20, "MR 2");

        app.tracked_mrs.push(TrackedMergeRequest {
            mr: mr1,
            pipelines: vec![],
            jobs: HashMap::new(),
            job_logs_cache: HashMap::new(),
            notes: Vec::new(),
            notes_loaded: false,
            selected_pipeline_index: 0,
            selected_note_index: 0,
            loading: false,
            error: None,
        });

        app.tracked_mrs.push(TrackedMergeRequest {
            mr: mr2,
            pipelines: vec![],
            jobs: HashMap::new(),
            job_logs_cache: HashMap::new(),
            notes: Vec::new(),
            notes_loaded: false,
            selected_pipeline_index: 0,
            selected_note_index: 0,
            loading: false,
            error: None,
        });

        assert_eq!(app.tracked_mrs.len(), 2);
        app.update(Action::RemoveCurrentMr);
        assert_eq!(app.tracked_mrs.len(), 1);
        assert_eq!(app.tracked_mrs[0].mr.title, "MR 2");
    }

    #[test]
    fn test_get_selected_mr() {
        let mut app = App::new(123, None, false, 1);

        assert!(app.get_selected_mr().is_none());

        let mr = create_test_mr(1, 10, "Test MR");
        app.tracked_mrs.push(TrackedMergeRequest {
            mr,
            pipelines: vec![],
            jobs: HashMap::new(),
            job_logs_cache: HashMap::new(),
            notes: Vec::new(),
            notes_loaded: false,
            selected_pipeline_index: 0,
            selected_note_index: 0,
            loading: false,
            error: None,
        });

        let selected = app.get_selected_mr();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().mr.title, "Test MR");
    }

    #[test]
    fn test_get_selected_jobs() {
        let mut app = App::new(123, None, false, 1);

        let mr = create_test_mr(1, 10, "Test MR");
        let pipeline = create_test_pipeline(100, PipelineStatus::Running);
        let job = create_test_job(200, "test-job", JobStatus::Success);

        let mut jobs_map = HashMap::new();
        jobs_map.insert(100, vec![job]);

        app.tracked_mrs.push(TrackedMergeRequest {
            mr,
            pipelines: vec![pipeline],
            jobs: jobs_map,
            job_logs_cache: HashMap::new(),
            notes: Vec::new(),
            notes_loaded: false,
            selected_pipeline_index: 0,
            selected_note_index: 0,
            loading: false,
            error: None,
        });

        let jobs = app.get_selected_jobs();
        assert!(jobs.is_some());
        assert_eq!(jobs.unwrap().len(), 1);
        assert_eq!(jobs.unwrap()[0].name, "test-job");
    }
}
