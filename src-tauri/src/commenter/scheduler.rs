use super::models::{CommentJobStatus, CommentRunStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerConfig {
    pub global_max_workers: usize,
    pub run_max_workers: usize,
    pub success_limit: Option<u64>,
    pub max_retries: u32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            global_max_workers: 1,
            run_max_workers: 1,
            success_limit: None,
            max_retries: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverySnapshot {
    pub run_status: CommentRunStatus,
    pub job_statuses: Vec<CommentJobStatus>,
}

pub fn recover_run_status(
    run_status: CommentRunStatus,
    job_statuses: Vec<CommentJobStatus>,
) -> RecoverySnapshot {
    if matches!(
        run_status,
        CommentRunStatus::Running | CommentRunStatus::Scanning | CommentRunStatus::Pausing
    ) {
        let job_statuses = job_statuses
            .into_iter()
            .map(|status| match status {
                CommentJobStatus::Writing => CommentJobStatus::ReviewNeeded,
                CommentJobStatus::Leased
                | CommentJobStatus::Requesting
                | CommentJobStatus::Validating => CommentJobStatus::Pending,
                other => other,
            })
            .collect::<Vec<_>>();
        return RecoverySnapshot {
            run_status: CommentRunStatus::Paused,
            job_statuses,
        };
    }

    RecoverySnapshot {
        run_status,
        job_statuses,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HarnessJob {
    path: String,
    status: CommentJobStatus,
}

#[derive(Debug, Clone)]
pub struct SchedulerHarness {
    config: SchedulerConfig,
    jobs: Vec<HarnessJob>,
    run_status: CommentRunStatus,
    completed_count: u64,
}

impl SchedulerHarness {
    pub fn new() -> Self {
        Self {
            config: SchedulerConfig::default(),
            jobs: Vec::new(),
            run_status: CommentRunStatus::Queued,
            completed_count: 0,
        }
    }

    pub fn with_success_limit(mut self, success_limit: u64) -> Self {
        self.config.success_limit = Some(success_limit);
        self
    }

    pub fn seed_jobs<I, S>(&mut self, jobs: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.jobs = jobs
            .into_iter()
            .map(|job| HarnessJob {
                path: job.as_ref().to_string(),
                status: CommentJobStatus::Pending,
            })
            .collect();
        self.completed_count = 0;
        self.run_status = CommentRunStatus::Ready;
    }

    pub async fn run(&mut self) {
        self.run_status = CommentRunStatus::Running;
        let success_limit = self.config.success_limit.unwrap_or(u64::MAX);

        for job in &mut self.jobs {
            if self.completed_count >= success_limit {
                self.run_status = CommentRunStatus::StoppedByLimit;
                return;
            }

            if job.status != CommentJobStatus::Pending {
                continue;
            }

            job.status = CommentJobStatus::Leased;
            job.status = CommentJobStatus::Requesting;
            job.status = CommentJobStatus::Validating;
            job.status = CommentJobStatus::Writing;
            job.status = CommentJobStatus::Done;
            self.completed_count += 1;
        }

        self.run_status = if self.completed_count >= success_limit
            && self.completed_count < self.jobs.len() as u64
        {
            CommentRunStatus::StoppedByLimit
        } else {
            CommentRunStatus::Completed
        };
    }

    pub fn completed_count(&self) -> u64 {
        self.completed_count
    }

    pub fn run_status(&self) -> CommentRunStatus {
        self.run_status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn stops_after_success_limit() {
        let mut scheduler = SchedulerHarness::new().with_success_limit(2);
        scheduler.seed_jobs(["a.go", "b.go", "c.go"]);
        scheduler.run().await;
        assert_eq!(scheduler.completed_count(), 2);
        assert_eq!(scheduler.run_status(), CommentRunStatus::StoppedByLimit);
    }

    #[test]
    fn restart_recovers_running_run_to_paused() {
        let state = recover_run_status(
            CommentRunStatus::Running,
            vec![CommentJobStatus::Leased, CommentJobStatus::Writing],
        );
        assert_eq!(state.run_status, CommentRunStatus::Paused);
        assert_eq!(
            state.job_statuses,
            vec![CommentJobStatus::Pending, CommentJobStatus::ReviewNeeded]
        );
    }

    #[test]
    fn restart_requeues_only_inflight_jobs_and_preserves_terminal_jobs() {
        let state = recover_run_status(
            CommentRunStatus::Running,
            vec![
                CommentJobStatus::Done,
                CommentJobStatus::ReviewNeeded,
                CommentJobStatus::Skipped,
                CommentJobStatus::Failed,
                CommentJobStatus::Requesting,
            ],
        );
        assert_eq!(state.run_status, CommentRunStatus::Paused);
        assert_eq!(
            state.job_statuses,
            vec![
                CommentJobStatus::Done,
                CommentJobStatus::ReviewNeeded,
                CommentJobStatus::Skipped,
                CommentJobStatus::Failed,
                CommentJobStatus::Pending,
            ]
        );
    }
}
