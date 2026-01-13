//! Terminal progress indicator support using OSC 9;4 escape sequences.
//!
//! FIXME: This could be part of console-rs/indicatif in the future?
//! - <https://github.com/console-rs/indicatif/issues/596>
//! - The uv folks are interested too: <https://github.com/astral-sh/uv/issues/11121#issuecomment-3566780089>
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// How often to re-emit the progress indicator (in milliseconds).
/// Balances responsiveness against terminal write overhead.
const REFRESH_INTERVAL_MS: u64 = 100;

/// Progress state for OSC 9;4 sequences.
/// See: https://conemu.github.io/en/AnsiEscapeCodes.html#ConEmu_specific_OSC
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgressState {
    /// Remove/clear the progress indicator (state 0)
    Remove,
    /// Set progress value 0-100 (state 1)
    Progress(u8),
    /// Error state (state 2)
    Error,
    /// Indeterminate/pulsing progress (state 3)
    #[allow(dead_code)]
    Indeterminate,
    /// Paused state (state 4)
    #[allow(dead_code)]
    Paused,
}

impl ProgressState {
    fn state_code(&self) -> u8 {
        match self {
            ProgressState::Remove => 0,
            ProgressState::Progress(_) => 1,
            ProgressState::Error => 2,
            ProgressState::Indeterminate => 3,
            ProgressState::Paused => 4,
        }
    }

    fn progress_value(&self) -> u8 {
        match self {
            ProgressState::Progress(v) => (*v).min(100),
            ProgressState::Error => 100,
            _ => 0,
        }
    }
}

/// Check if the current terminal supports OSC 9;4 progress sequences.
///
/// Detection is based on environment variables set by known supporting terminals.
///
/// For reference: <https://github.com/nextest-rs/nextest/blob/main/nextest-runner/src/reporter/displayer/progress.rs#L647>
///
/// FIXME: Maybe there will be a better way to do this in the future? This feels clunky.
pub fn terminal_supports_progress() -> bool {
    use std::env::var_os;

    // Check if explicitly disabled
    if var_os("RV_NO_PROGRESS_BAR").is_some() {
        return false;
    }

    // Windows Terminal
    if var_os("WT_SESSION").is_some() {
        return true;
    }

    // ConEmu (must be set to "ON")
    if var_os("ConEmuANSI").is_some_and(|v| v == "ON") {
        return true;
    }

    // Check the TERM_PROGRAM env var for various terminals
    const SUPPORTED_TERM_PROGRAMS: &[&str] = &["ghostty", "iTerm.app", "mintty", "WezTerm"];
    if let Some(term_program) = var_os("TERM_PROGRAM")
        && SUPPORTED_TERM_PROGRAMS.iter().any(|&p| term_program == p)
    {
        return true;
    }

    false
}

/// Write an OSC 9;4 progress sequence to stderr.
///
/// Format: `ESC ] 9 ; 4 ; <state> ; <progress> ST`
/// Where ST (string terminator) is `ESC \`
fn write_progress(state: ProgressState) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    write!(
        stderr,
        "\x1b]9;4;{};{}\x1b\\",
        state.state_code(),
        state.progress_value()
    )?;
    stderr.flush()
}

/// Shared state for the work progress tracker.
struct WorkProgressInner {
    /// Current phase (0, 1, 2, ...)
    current_phase: AtomicU64,
    /// Total items in the current phase
    phase_total: AtomicU64,
    /// Completed items in the current phase
    phase_completed: AtomicU64,
    /// Base percentage from completed phases
    base_percent: AtomicU64,
    /// Percentage allocated to current phase
    phase_percent: AtomicU64,
    /// Whether the refresh thread should keep running
    running: AtomicBool,
    /// True while a phase transition is in progress (prevents race conditions)
    transitioning: AtomicBool,
    /// True if error state was set (prevents Drop from clearing it)
    error_set: AtomicBool,
}

/// A thread-safe work progress tracker that updates the terminal progress indicator.
///
/// This tracks progress across multiple phases, where each phase gets a weighted
/// portion of the total progress bar. Progress never goes backward.
///
/// A background thread periodically re-emits progress to handle terminals
///   where other output might clear the progress indicator.
///
/// # Example
/// ```ignore
/// let progress = WorkProgress::new();
///
/// // Phase 1: Downloads (0-40%)
/// progress.start_phase(100, 40);
/// for _ in 0..100 { progress.complete_one(); }
///
/// // Phase 2: Installs (40-80%)
/// progress.start_phase(100, 40);
/// for _ in 0..100 { progress.complete_one(); }
///
/// // Phase 3: Compiles (80-100%)
/// progress.start_phase(20, 20);
/// for _ in 0..20 { progress.complete_one(); }
/// ```
pub struct WorkProgress {
    inner: Arc<WorkProgressInner>,
    enabled: bool,
    _refresh_thread: Option<std::thread::JoinHandle<()>>,
}

impl WorkProgress {
    /// Create a new work progress tracker.
    ///
    /// The progress indicator won't be shown until `start_phase` is called.
    pub fn new() -> Self {
        let enabled = terminal_supports_progress();
        // Don't show anything yet - wait for start_phase to be called

        let inner = Arc::new(WorkProgressInner {
            current_phase: AtomicU64::new(0),
            phase_total: AtomicU64::new(0),
            phase_completed: AtomicU64::new(0),
            base_percent: AtomicU64::new(0),
            phase_percent: AtomicU64::new(0), // Start at 0 so no progress shows initially
            running: AtomicBool::new(true),
            transitioning: AtomicBool::new(false),
            error_set: AtomicBool::new(false),
        });

        // Spawn a background thread to periodically refresh the progress
        // This helps when other terminal output clears the OSC 9;4 state
        let refresh_thread = if enabled {
            let inner_clone = Arc::clone(&inner);
            Some(std::thread::spawn(move || {
                while inner_clone.running.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(REFRESH_INTERVAL_MS));
                    if !inner_clone.running.load(Ordering::SeqCst) {
                        break;
                    }
                    // Only emit progress if a phase has been started and not transitioning
                    if !inner_clone.transitioning.load(Ordering::SeqCst)
                        && inner_clone.current_phase.load(Ordering::Relaxed) > 0
                    {
                        let percent = compute_percent(&inner_clone);
                        let _ = write_progress(ProgressState::Progress(percent));
                    }
                }
            }))
        } else {
            None
        };

        Self {
            inner,
            enabled,
            _refresh_thread: refresh_thread,
        }
    }

    /// Returns whether progress reporting is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Start a new phase with the given number of items and percentage allocation.
    ///
    /// The percentage is how much of the total progress bar this phase should use.
    /// For example, if downloads should be 0-40%, call `start_phase(num_downloads, 40)`.
    pub fn start_phase(&self, total_items: u64, percent_allocation: u64) {
        // Set transitioning flag to prevent race with refresh thread
        self.inner.transitioning.store(true, Ordering::SeqCst);

        // Add current phase's allocation to base (completing the previous phase)
        let prev_phase_percent = self.inner.phase_percent.load(Ordering::Relaxed);
        let prev_phase_total = self.inner.phase_total.load(Ordering::Relaxed);
        if prev_phase_total > 0 {
            // Previous phase existed, add its full allocation to base
            self.inner
                .base_percent
                .fetch_add(prev_phase_percent, Ordering::Relaxed);
        }

        // Set up the new phase
        self.inner.current_phase.fetch_add(1, Ordering::Relaxed);
        self.inner.phase_total.store(total_items, Ordering::Relaxed);
        self.inner.phase_completed.store(0, Ordering::Relaxed);
        self.inner
            .phase_percent
            .store(percent_allocation, Ordering::Relaxed);

        // Clear transitioning flag
        self.inner.transitioning.store(false, Ordering::SeqCst);

        self.update_progress();
    }

    /// Mark one work item as completed and update the progress indicator.
    pub fn complete_one(&self) {
        self.inner.phase_completed.fetch_add(1, Ordering::Relaxed);
        self.update_progress();
    }

    /// Get current progress as a percentage (0-100).
    pub fn percent(&self) -> u8 {
        compute_percent(&self.inner)
    }

    /// Update the terminal progress indicator.
    fn update_progress(&self) {
        if !self.enabled {
            return;
        }
        let _ = write_progress(ProgressState::Progress(self.percent()));
    }

    /// Set error state on the progress indicator.
    ///
    /// This shows a red/error indicator in terminals that support it.
    /// The error state persists until the terminal is cleared or another
    /// command updates the progress (Drop will not clear it).
    pub fn set_error(&self) {
        if self.enabled {
            self.inner.error_set.store(true, Ordering::SeqCst);
            let _ = write_progress(ProgressState::Error);
        }
    }

    /// Clear/remove the progress indicator.
    pub fn clear(&self) {
        // Stop the refresh thread
        self.inner.running.store(false, Ordering::SeqCst);
        if self.enabled {
            let _ = write_progress(ProgressState::Remove);
        }
    }
}

/// Compute current progress percentage from inner state.
fn compute_percent(inner: &WorkProgressInner) -> u8 {
    let base = inner.base_percent.load(Ordering::Relaxed);
    let phase_total = inner.phase_total.load(Ordering::Relaxed);
    let phase_completed = inner.phase_completed.load(Ordering::Relaxed);
    let phase_percent = inner.phase_percent.load(Ordering::Relaxed);

    if phase_total == 0 {
        return base.min(100) as u8;
    }

    let phase_progress = (phase_completed * phase_percent) / phase_total;
    (base + phase_progress).min(100) as u8
}

impl Default for WorkProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WorkProgress {
    fn drop(&mut self) {
        // Stop the refresh thread
        self.inner.running.store(false, Ordering::SeqCst);

        // Wait for the thread to finish
        if let Some(handle) = self._refresh_thread.take() {
            let _ = handle.join();
        }

        // Don't clear if error state was set - let the error indicator persist
        if self.enabled && !self.inner.error_set.load(Ordering::SeqCst) {
            let _ = write_progress(ProgressState::Remove);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_state_codes() {
        assert_eq!(ProgressState::Remove.state_code(), 0);
        assert_eq!(ProgressState::Progress(50).state_code(), 1);
        assert_eq!(ProgressState::Error.state_code(), 2);
        assert_eq!(ProgressState::Indeterminate.state_code(), 3);
        assert_eq!(ProgressState::Paused.state_code(), 4);
    }

    #[test]
    fn test_progress_values() {
        assert_eq!(ProgressState::Remove.progress_value(), 0);
        assert_eq!(ProgressState::Progress(50).progress_value(), 50);
        assert_eq!(ProgressState::Progress(150).progress_value(), 100); // clamped
        assert_eq!(ProgressState::Error.progress_value(), 100);
        assert_eq!(ProgressState::Indeterminate.progress_value(), 0);
    }

    #[test]
    fn test_compute_percent_empty_phase() {
        let inner = WorkProgressInner {
            current_phase: AtomicU64::new(0),
            phase_total: AtomicU64::new(0),
            phase_completed: AtomicU64::new(0),
            base_percent: AtomicU64::new(0),
            phase_percent: AtomicU64::new(0),
            running: AtomicBool::new(true),
            transitioning: AtomicBool::new(false),
            error_set: AtomicBool::new(false),
        };
        assert_eq!(compute_percent(&inner), 0);
    }

    #[test]
    fn test_compute_percent_with_base() {
        let inner = WorkProgressInner {
            current_phase: AtomicU64::new(1),
            phase_total: AtomicU64::new(0),
            phase_completed: AtomicU64::new(0),
            base_percent: AtomicU64::new(40),
            phase_percent: AtomicU64::new(40),
            running: AtomicBool::new(true),
            transitioning: AtomicBool::new(false),
            error_set: AtomicBool::new(false),
        };
        // No items in phase, should return base
        assert_eq!(compute_percent(&inner), 40);
    }

    #[test]
    fn test_compute_percent_partial_phase() {
        let inner = WorkProgressInner {
            current_phase: AtomicU64::new(1),
            phase_total: AtomicU64::new(100),
            phase_completed: AtomicU64::new(50),
            base_percent: AtomicU64::new(0),
            phase_percent: AtomicU64::new(40),
            running: AtomicBool::new(true),
            transitioning: AtomicBool::new(false),
            error_set: AtomicBool::new(false),
        };
        // 50% of 40% phase = 20%
        assert_eq!(compute_percent(&inner), 20);
    }

    #[test]
    fn test_compute_percent_complete_phase() {
        let inner = WorkProgressInner {
            current_phase: AtomicU64::new(1),
            phase_total: AtomicU64::new(100),
            phase_completed: AtomicU64::new(100),
            base_percent: AtomicU64::new(0),
            phase_percent: AtomicU64::new(40),
            running: AtomicBool::new(true),
            transitioning: AtomicBool::new(false),
            error_set: AtomicBool::new(false),
        };
        // 100% of 40% phase = 40%
        assert_eq!(compute_percent(&inner), 40);
    }

    #[test]
    fn test_compute_percent_second_phase() {
        let inner = WorkProgressInner {
            current_phase: AtomicU64::new(2),
            phase_total: AtomicU64::new(100),
            phase_completed: AtomicU64::new(50),
            base_percent: AtomicU64::new(40), // First phase completed
            phase_percent: AtomicU64::new(40),
            running: AtomicBool::new(true),
            transitioning: AtomicBool::new(false),
            error_set: AtomicBool::new(false),
        };
        // Base 40% + 50% of 40% phase = 40% + 20% = 60%
        assert_eq!(compute_percent(&inner), 60);
    }

    #[test]
    fn test_compute_percent_clamped_to_100() {
        let inner = WorkProgressInner {
            current_phase: AtomicU64::new(3),
            phase_total: AtomicU64::new(10),
            phase_completed: AtomicU64::new(10),
            base_percent: AtomicU64::new(80),
            phase_percent: AtomicU64::new(30), // Would exceed 100%
            running: AtomicBool::new(true),
            transitioning: AtomicBool::new(false),
            error_set: AtomicBool::new(false),
        };
        // Base 80% + 100% of 30% = 110%, clamped to 100%
        assert_eq!(compute_percent(&inner), 100);
    }

    #[test]
    fn test_work_progress_phase_progression() {
        // Create progress with detection disabled (won't write to terminal)
        let inner = Arc::new(WorkProgressInner {
            current_phase: AtomicU64::new(0),
            phase_total: AtomicU64::new(0),
            phase_completed: AtomicU64::new(0),
            base_percent: AtomicU64::new(0),
            phase_percent: AtomicU64::new(0),
            running: AtomicBool::new(true),
            transitioning: AtomicBool::new(false),
            error_set: AtomicBool::new(false),
        });

        // Simulate phase 1: downloads (0-40%)
        inner.current_phase.fetch_add(1, Ordering::Relaxed);
        inner.phase_total.store(100, Ordering::Relaxed);
        inner.phase_percent.store(40, Ordering::Relaxed);

        // Complete half of downloads
        inner.phase_completed.store(50, Ordering::Relaxed);
        assert_eq!(compute_percent(&inner), 20); // 50% of 40%

        // Complete all downloads
        inner.phase_completed.store(100, Ordering::Relaxed);
        assert_eq!(compute_percent(&inner), 40); // 100% of 40%

        // Simulate phase 2: installs (40-80%)
        inner.base_percent.fetch_add(40, Ordering::Relaxed); // Add phase 1's allocation
        inner.current_phase.fetch_add(1, Ordering::Relaxed);
        inner.phase_total.store(100, Ordering::Relaxed);
        inner.phase_completed.store(0, Ordering::Relaxed);
        inner.phase_percent.store(40, Ordering::Relaxed);

        assert_eq!(compute_percent(&inner), 40); // Base only, no progress yet

        // Complete half of installs
        inner.phase_completed.store(50, Ordering::Relaxed);
        assert_eq!(compute_percent(&inner), 60); // 40% base + 20%

        // Complete all installs
        inner.phase_completed.store(100, Ordering::Relaxed);
        assert_eq!(compute_percent(&inner), 80); // 40% base + 40%

        // Simulate phase 3: compiles (80-100%)
        inner.base_percent.fetch_add(40, Ordering::Relaxed);
        inner.current_phase.fetch_add(1, Ordering::Relaxed);
        inner.phase_total.store(20, Ordering::Relaxed);
        inner.phase_completed.store(0, Ordering::Relaxed);
        inner.phase_percent.store(20, Ordering::Relaxed);

        assert_eq!(compute_percent(&inner), 80); // Base only

        // Complete half of compiles
        inner.phase_completed.store(10, Ordering::Relaxed);
        assert_eq!(compute_percent(&inner), 90); // 80% base + 10%

        // Complete all compiles
        inner.phase_completed.store(20, Ordering::Relaxed);
        assert_eq!(compute_percent(&inner), 100); // 80% base + 20%
    }

    #[test]
    fn test_work_progress_no_compiles() {
        // Test scenario where there are no native extensions to compile
        let inner = Arc::new(WorkProgressInner {
            current_phase: AtomicU64::new(2),
            phase_total: AtomicU64::new(100),
            phase_completed: AtomicU64::new(100),
            base_percent: AtomicU64::new(40),
            phase_percent: AtomicU64::new(40),
            running: AtomicBool::new(true),
            transitioning: AtomicBool::new(false),
            error_set: AtomicBool::new(false),
        });

        // After installs complete, we're at 80%
        assert_eq!(compute_percent(&inner), 80);

        // If no compile phase is started (0 native extensions),
        // progress stays at 80% until clear() is called
    }
}
