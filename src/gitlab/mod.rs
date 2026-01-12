pub mod client;
pub mod models;

pub use client::GitLabClient;
pub use models::{Job, JobStatus, MergeRequest, Note, Pipeline, PipelineStatus};
