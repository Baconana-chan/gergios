//! # Job Control — Background & Foreground Job Management
//!
//! Implements POSIX job control for minish:
//! - Job tracking with unique IDs
//! - Process group management
//! - Background execution (`cmd &`)
//! - `jobs`, `fg`, `bg` builtins
//!
//! ## Architecture
//!
//! ```text
//! JobManager          — owns all tracked jobs
//!   ├── Job { id, pgrp, pid, command, state }
//!   │     └── JobState::Running | Stopped | Done(i32) | Killed(i32)
//!   ├── add()         — register a new job
//!   ├── update_state()— transition job state by pgrp
//!   └── find_by_id()  — lookup job by number
//! ```

/// State of a tracked job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobState {
    /// Process(es) currently running.
    Running,
    /// Process(es) stopped by SIGTSTP (Ctrl+Z).
    Stopped,
    /// Process(es) exited with a code.
    Done(i32),
    /// Process(es) killed by a signal.
    Killed(i32),
}

/// A single tracked job (background or stopped).
#[derive(Debug, Clone)]
pub struct Job {
    /// Job number (displayed as [1], [2], etc.).
    pub id: u32,
    /// Process group ID (all processes in the pipeline share this).
    pub pgrp: i32,
    /// Leader PID (first process in the pipeline, used for display).
    pub pid: u32,
    /// Command line text for display.
    pub command: String,
    /// Current job state.
    pub state: JobState,
}

/// Central job manager.
///
/// Tracks background and stopped jobs. Provides methods for
/// registration, lookup, state updates, and removal.
#[derive(Debug)]
pub struct JobManager {
    jobs: Vec<Job>,
    next_id: u32,
}

impl JobManager {
    /// Create a new empty job manager.
    pub fn new() -> Self {
        JobManager {
            jobs: Vec::new(),
            next_id: 1,
        }
    }

    /// Register a new job and return its job ID.
    ///
    /// `pgrp` is the process group ID (usually == `pid` for the leader).
    /// `pid` is the leader PID (first process in the pipeline).
    pub fn add(&mut self, pgrp: i32, pid: u32, command: &str) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.jobs.push(Job {
            id,
            pgrp,
            pid,
            command: command.to_string(),
            state: JobState::Running,
        });
        id
    }

    /// Remove a job by its ID.
    pub fn remove(&mut self, id: u32) {
        self.jobs.retain(|j| j.id != id);
    }

    /// Get a reference to a job by ID.
    pub fn get(&self, id: u32) -> Option<&Job> {
        self.jobs.iter().find(|j| j.id == id)
    }

    /// Get a mutable reference to a job by ID.
    #[allow(dead_code)]
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    /// Get all jobs (immutable).
    pub fn list(&self) -> &[Job] {
        &self.jobs
    }

    /// Find a job index by its process group ID.
    pub fn find_by_pgrp(&self, pgrp: i32) -> Option<usize> {
        self.jobs.iter().position(|j| j.pgrp == pgrp)
    }

    /// Update the state of a job identified by its pgrp.
    /// Returns the job ID if found, for notification.
    pub fn update_state(&mut self, pgrp: i32, state: JobState) -> Option<u32> {
        if let Some(idx) = self.find_by_pgrp(pgrp) {
            self.jobs[idx].state = state;
            Some(self.jobs[idx].id)
        } else {
            None
        }
    }

    /// Parse a job specification string.
    ///
    /// Accepted formats:
    /// - `"1"` → `Ok(1)`
    /// - `"%1"` → `Ok(1)`
    /// - other → `Err(...)`
    pub fn parse_job_spec(s: &str) -> Result<u32, String> {
        let s = if s.starts_with('%') { &s[1..] } else { s };
        s.parse::<u32>()
            .map_err(|_| format!("invalid job spec: '{}'", s))
    }

    /// Check if a job ID exists.
    pub fn has(&self, id: u32) -> bool {
        self.jobs.iter().any(|j| j.id == id)
    }

    /// Remove all completed (Done/Killed) jobs.
    pub fn reap(&mut self) {
        self.jobs.retain(|j| matches!(j.state, JobState::Running | JobState::Stopped));
    }

    /// Print notification about background jobs before prompt.
    pub fn print_notifications(&self) {
        for job in &self.jobs {
            match job.state {
                JobState::Done(code) => {
                    println!("[{}]  Done({})          {}", job.id, code, job.command);
                }
                JobState::Killed(sig) => {
                    println!("[{}]  Killed({})        {}", job.id, sig, job.command);
                }
                JobState::Stopped => {
                    println!("[{}]  Stopped           {}", job.id, job.command);
                }
                _ => {}
            }
        }
    }

    /// Check if there are any running background jobs.
    pub fn has_running(&self) -> bool {
        self.jobs.iter().any(|j| j.state == JobState::Running)
    }
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Builtin implementations (jobs, fg, bg)
// ============================================================================

/// List all tracked jobs (`jobs` builtin).
pub fn builtin_jobs(args: &[String], jobs: &JobManager) -> i32 {
    // Optional: show only stopped jobs
    let only_stopped = args.iter().any(|a| a == "-s" || a == "--stopped");

    let job_list = jobs.list();
    if job_list.is_empty() {
        println!("minish: no jobs");
        return 0;
    }

    for job in job_list {
        if only_stopped && job.state != JobState::Stopped {
            continue;
        }

        let state_str = match job.state {
            JobState::Running => "Running",
            JobState::Stopped => "Stopped",
            JobState::Done(c) => return 0, // reaped already
            JobState::Killed(_) => return 0, // reaped already
        };

        println!("[{}]  {:<12} {}", job.id, state_str, job.command);
    }

    0
}

/// Bring a job to the foreground (`fg` builtin).
///
/// Sends SIGCONT, waits for the job to complete/stop,
/// then returns the exit code.
pub fn builtin_fg(args: &[String], jobs: &mut JobManager) -> i32 {
    // Determine which job to foreground
    let job_id = if args.is_empty() {
        // Select the most recent stopped or running job
        match jobs.list().iter().rev().find(|j| {
            matches!(j.state, JobState::Stopped | JobState::Running)
        }) {
            Some(j) => j.id,
            None => {
                eprintln!("fg: no current job");
                return 0;
            }
        }
    } else if args[0].starts_with('%') || args[0].chars().all(|c| c.is_ascii_digit()) {
        match JobManager::parse_job_spec(&args[0]) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("fg: {}: {}", args[0], e);
                return 1;
            }
        }
    } else {
        eprintln!("fg: invalid argument: {}", args[0]);
        return 1;
    };

    #[cfg(unix)]
    {
        return builtin_fg_unix(job_id, jobs);
    }
    #[cfg(not(unix))]
    {
        let _ = job_id;
        eprintln!("fg: not supported on this platform");
        return 1;
    }
}

/// Unix implementation of `fg` — sends SIGCONT and waits.
///
/// `job_id` is already resolved by `builtin_fg`.
#[cfg(unix)]
fn builtin_fg_unix(job_id: u32, jobs: &mut JobManager) -> i32 {
    let job = match jobs.get(job_id) {
        Some(j) => j.clone(),
        None => {
            eprintln!("fg: job not found: {}", job_id);
            return 1;
        }
    };

    println!("{}", job.command);

    // Send SIGCONT to the process group
    unsafe {
        libc::kill(-job.pgrp, libc::SIGCONT);
    }

    // Wait for the job to complete or stop
    let pgrp = job.pgrp;
    let exit_code = wait_for_pgrp(pgrp, jobs);

    // Remove the job if it completed
    if jobs.get(job_id).is_some() {
        if let Some(idx) = jobs.find_by_pgrp(pgrp) {
            if matches!(jobs.list()[idx].state, JobState::Done(_) | JobState::Killed(_)) {
                jobs.remove(job_id);
            }
        }
    }

    exit_code
}

/// Continue a stopped job in the background (`bg` builtin).
pub fn builtin_bg(args: &[String], jobs: &mut JobManager) -> i32 {
    let job_id = if args.is_empty() {
        match jobs.list().iter().rev().find(|j| j.state == JobState::Stopped) {
            Some(j) => j.id,
            None => {
                eprintln!("bg: no current job");
                return 0;
            }
        }
    } else if args[0].starts_with('%') || args[0].chars().all(|c| c.is_ascii_digit()) {
        match JobManager::parse_job_spec(&args[0]) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("bg: {}: {}", args[0], e);
                return 1;
            }
        }
    } else {
        eprintln!("bg: invalid argument: {}", args[0]);
        return 1;
    };

    let job = match jobs.get(job_id) {
        Some(j) => j.clone(),
        None => {
            eprintln!("bg: job not found: {}", job_id);
            return 1;
        }
    };

    if job.state != JobState::Stopped {
        eprintln!("bg: job {} is already running", job_id);
        return 1;
    }

    #[cfg(unix)]
    {
        unsafe { libc::kill(-job.pgrp, libc::SIGCONT); }
        jobs.update_state(job.pgrp, JobState::Running);
        println!("[{}] {} &", job.id, job.command);
        0
    }
    #[cfg(not(unix))]
    {
        let _ = job;
        eprintln!("bg: not supported on this platform");
        1
    }
}

/// Wait for all processes in a process group to complete or stop.
/// Updates the JobManager state accordingly.
/// Returns the exit code of the last process.
#[cfg(unix)]
pub fn wait_for_pgrp(pgrp: i32, jobs: &mut JobManager) -> i32 {
    let mut exit_code = 0;
    loop {
        let mut status: i32 = 0;
        // Wait for ANY process in the pgrp (negative pgrp argument)
        let ret = unsafe { libc::waitpid(-pgrp, &mut status, libc::WUNTRACED) };

        if ret == -1 {
            // No more children in this pgrp
            break;
        }

        if unsafe { libc::WIFEXITED(status) } {
            let code = unsafe { libc::WEXITSTATUS(status) };
            exit_code = code as i32;
            // Check if all processes in the pgrp are done
            let more = unsafe { libc::waitpid(-pgrp, &mut 0, libc::WNOHANG) };
            if more == -1 || more == 0 {
                // No (more) children — job is done
                // But we're in a loop that waits, so break if no more
                break;
            }
        } else if unsafe { libc::WIFSIGNALED(status) } {
            let sig = unsafe { libc::WTERMSIG(status) };
            jobs.update_state(pgrp, JobState::Killed(sig));
            exit_code = 128 + sig as i32;
            // Continue waiting — other processes in the pgrp may still be running
        } else if unsafe { libc::WIFSTOPPED(status) } {
            let sig = unsafe { libc::WSTOPSIG(status) };
            jobs.update_state(pgrp, JobState::Stopped);
            if sig == libc::SIGTSTP {
                println!("\n[{}]+  Stopped", 
                    jobs.find_by_pgrp(pgrp).and_then(|i| {
                        jobs.list().get(i).map(|j| j.id)
                    }).unwrap_or(0));
            }
            // Stop waiting — job is suspended
            break;
        }
    }

    exit_code
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_manager_new() {
        let jm = JobManager::new();
        assert_eq!(jm.list().len(), 0);
        assert_eq!(jm.next_id, 1);
    }

    #[test]
    fn test_job_manager_add() {
        let mut jm = JobManager::new();
        let id = jm.add(12345, 12345, "sleep 10");
        assert_eq!(id, 1);
        assert_eq!(jm.list().len(), 1);
        assert_eq!(jm.list()[0].command, "sleep 10");
        assert_eq!(jm.list()[0].state, JobState::Running);
    }

    #[test]
    fn test_job_manager_add_multiple() {
        let mut jm = JobManager::new();
        let id1 = jm.add(100, 100, "cmd1");
        let id2 = jm.add(200, 200, "cmd2");
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(jm.list().len(), 2);
    }

    #[test]
    fn test_job_manager_get() {
        let mut jm = JobManager::new();
        jm.add(123, 123, "test job");
        let job = jm.get(1).unwrap();
        assert_eq!(job.command, "test job");
        assert_eq!(job.pgrp, 123);
        assert_eq!(job.pid, 123);

        assert!(jm.get(99).is_none());
    }

    #[test]
    fn test_job_manager_remove() {
        let mut jm = JobManager::new();
        jm.add(123, 123, "job1");
        jm.add(456, 456, "job2");
        jm.remove(1);
        assert_eq!(jm.list().len(), 1);
        assert_eq!(jm.list()[0].id, 2);
    }

    #[test]
    fn test_job_manager_update_state() {
        let mut jm = JobManager::new();
        jm.add(777, 777, "test");
        jm.update_state(777, JobState::Stopped);
        assert_eq!(jm.list()[0].state, JobState::Stopped);
    }

    #[test]
    fn test_job_manager_has() {
        let mut jm = JobManager::new();
        jm.add(1, 1, "job");
        assert!(jm.has(1));
        assert!(!jm.has(2));
    }

    #[test]
    fn test_job_manager_reap() {
        let mut jm = JobManager::new();
        jm.add(1, 1, "running");
        jm.add(2, 2, "stopped");
        jm.update_state(2, JobState::Stopped);
        jm.add(3, 3, "done");
        jm.update_state(3, JobState::Done(0));
        jm.add(4, 4, "killed");
        jm.update_state(4, JobState::Killed(9));

        jm.reap();
        assert_eq!(jm.list().len(), 2);
        assert_eq!(jm.list()[0].id, 1);
        assert_eq!(jm.list()[1].id, 2);
    }

    #[test]
    fn test_parse_job_spec() {
        assert_eq!(JobManager::parse_job_spec("1"), Ok(1));
        assert_eq!(JobManager::parse_job_spec("%1"), Ok(1));
        assert_eq!(JobManager::parse_job_spec("%42"), Ok(42));
        assert!(JobManager::parse_job_spec("abc").is_err());
        assert!(JobManager::parse_job_spec("").is_err());
    }

    #[test]
    fn test_builtin_jobs_empty() {
        let jm = JobManager::new();
        assert_eq!(builtin_jobs(&[], &jm), 0);
    }

    #[test]
    fn test_builtin_jobs_with_running() {
        let mut jm = JobManager::new();
        jm.add(100, 100, "sleep 30");
        assert_eq!(builtin_jobs(&[], &jm), 0);
    }

    #[test]
    fn test_builtin_bg_no_stopped_jobs() {
        let mut jm = JobManager::new();
        jm.add(100, 100, "sleep 10");
        // No stopped jobs, should error
        let result = builtin_bg(&[], &mut jm);
        assert_eq!(result, 0); // "no current job" → 0
    }

    #[test]
    fn test_builtin_bg_invalid_spec() {
        let mut jm = JobManager::new();
        assert_eq!(builtin_bg(&["abc".to_string()], &mut jm), 1);
    }

    #[test]
    fn test_builtin_fg_invalid_spec() {
        let mut jm = JobManager::new();
        assert_eq!(builtin_fg(&["abc".to_string()], &mut jm), 1);
    }

    #[test]
    fn test_builtin_fg_nonexistent_job() {
        let mut jm = JobManager::new();
        assert_eq!(builtin_fg(&["%99".to_string()], &mut jm), 1);
    }

    #[test]
    fn test_job_state_equality() {
        assert_eq!(JobState::Running, JobState::Running);
        assert_eq!(JobState::Stopped, JobState::Stopped);
        assert_eq!(JobState::Done(0), JobState::Done(0));
        assert_ne!(JobState::Done(0), JobState::Done(1));
        assert_eq!(JobState::Killed(9), JobState::Killed(9));
    }

    #[test]
    fn test_job_manager_find_by_pgrp() {
        let mut jm = JobManager::new();
        jm.add(111, 111, "job1");
        jm.add(222, 222, "job2");
        assert!(jm.find_by_pgrp(111).is_some());
        assert!(jm.find_by_pgrp(222).is_some());
        assert!(jm.find_by_pgrp(333).is_none());
    }

    #[test]
    fn test_job_manager_has_running() {
        let mut jm = JobManager::new();
        assert!(!jm.has_running());
        jm.add(1, 1, "job1");
        assert!(jm.has_running());
        jm.update_state(1, JobState::Stopped);
        assert!(!jm.has_running());
    }

    #[test]
    fn test_job_manager_print_notifications_no_panic() {
        let jm = JobManager::new();
        jm.print_notifications(); // no panics with empty list
    }
}
