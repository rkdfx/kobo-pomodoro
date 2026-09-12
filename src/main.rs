//! Pomodoro timer application for Kobo using Cobalt SDK.

use kobo_sdk::{
    action_id, ActionId, Context, Heartbeat, KoboApp, LogLevel, Screen, ScreenBuilder, Space,
    StoreResult, TaskId, TaskOutcome,
};
use std::process::ExitCode;

const STORE_KEY: &str = "pomodoro-state-v1";

const ACTION_START: &str = "start";
const ACTION_PAUSE: &str = "pause";
const ACTION_RESUME: &str = "resume";
const ACTION_RESET: &str = "reset";
const ACTION_SKIP: &str = "skip";

/// The mode of the Pomodoro timer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Focus,
    ShortBreak,
    LongBreak,
}

impl Mode {
    /// Duration of each session mode in seconds.
    pub const fn duration_secs(self) -> u32 {
        match self {
            Self::Focus => 25 * 60,      // 25 min = 1500 sec
            Self::ShortBreak => 5 * 60,  // 5 min = 300 sec
            Self::LongBreak => 15 * 60, // 15 min = 900 sec
        }
    }

    /// Display label for the current mode.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Focus => "Focus Session",
            Self::ShortBreak => "Short Break",
            Self::LongBreak => "Long Break",
        }
    }

    /// Detailed description of the mode.
    pub const fn summary(self) -> &'static str {
        match self {
            Self::Focus => "25 minutes of deep focus work.",
            Self::ShortBreak => "5 minutes to rest and recharge.",
            Self::LongBreak => "15 minutes long break.",
        }
    }
}

/// Execution state of the timer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerState {
    Ready,
    Running,
    Paused,
}

impl TimerState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Running => "Running",
            Self::Paused => "Paused",
        }
    }
}

/// Encodes persisted application state into UTF-8 bytes.
pub fn encode(completed_focus_count: u32) -> Vec<u8> {
    format!("completed={completed_focus_count}\n").into_bytes()
}

/// Decodes persisted application state from UTF-8 bytes.
pub fn decode(bytes: &[u8]) -> u32 {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return 0;
    };
    for line in text.lines() {
        if let Some(val_str) = line.strip_prefix("completed=") {
            if let Ok(count) = val_str.trim().parse::<u32>() {
                return count;
            }
        }
    }
    0
}

pub struct Pomodoro {
    mode: Mode,
    state: TimerState,
    remaining_secs: u32,
    completed_focus_count: u32,
    clock: Heartbeat,
    loaded: bool,
}

impl Default for Pomodoro {
    fn default() -> Self {
        let mode = Mode::Focus;
        Self {
            mode,
            state: TimerState::Ready,
            remaining_secs: mode.duration_secs(),
            completed_focus_count: 0,
            clock: Heartbeat::every(1),
            loaded: false,
        }
    }
}

impl Pomodoro {
    /// Formats seconds into MM:SS format.
    pub fn format_time(seconds: u32) -> String {
        let minutes = seconds / 60;
        let secs = seconds % 60;
        format!("{minutes:02}:{secs:02}")
    }

    /// Computes which session out of 4 in the current Pomodoro cycle.
    pub fn cycle_progress(&self) -> (u32, u32) {
        let current_in_cycle = (self.completed_focus_count % 4) + 1;
        (current_in_cycle, 4)
    }

    fn show(&mut self, context: &mut Context) {
        let screen = self.render_screen();
        context.set_screen(screen);
    }

    fn render_screen(&self) -> Screen {
        let mut screen = ScreenBuilder::new("Pomodoro").top_bar("Pomodoro Timer");

        if !self.loaded {
            return screen.skeleton(4).build();
        }

        let time_display = Self::format_time(self.remaining_secs);
        let mode_label = self.mode.label();

        // Large time display and current mode heading
        screen = screen
            .heading(format!("{time_display} - {mode_label}"))
            .text(self.mode.summary());

        // Key metrics / status details
        let (current_cycle, max_cycle) = self.cycle_progress();
        let cycle_str = if self.mode == Mode::Focus {
            format!("{current_cycle} of {max_cycle}")
        } else {
            format!("Completed {} total", self.completed_focus_count)
        };

        screen = screen.facts([
            ("Status", self.state.label()),
            ("Mode", self.mode.label()),
            ("Completed Focus", &self.completed_focus_count.to_string()),
            ("Cycle Progress", &cycle_str),
        ]);

        screen = screen.spacer(Space::Medium);

        // Control Buttons
        match self.state {
            TimerState::Ready => {
                let start_label = match self.mode {
                    Mode::Focus => "Start Focus (25m)",
                    Mode::ShortBreak => "Start Short Break (5m)",
                    Mode::LongBreak => "Start Long Break (15m)",
                };
                screen = screen.primary_button(ACTION_START, start_label);
                screen = screen.buttons([
                    (ACTION_RESET, "Reset"),
                    (ACTION_SKIP, "Skip Session"),
                ]);
            }
            TimerState::Running => {
                screen = screen.primary_button(ACTION_PAUSE, "Pause Timer");
                screen = screen.buttons([
                    (ACTION_RESET, "Reset"),
                    (ACTION_SKIP, "Skip Session"),
                ]);
            }
            TimerState::Paused => {
                screen = screen.primary_button(ACTION_RESUME, "Resume Timer");
                screen = screen.buttons([
                    (ACTION_RESET, "Reset"),
                    (ACTION_SKIP, "Skip Session"),
                ]);
            }
        }

        screen.build()
    }

    fn save(&mut self, context: &mut Context) {
        let bytes = encode(self.completed_focus_count);
        context.store().save(STORE_KEY, bytes);
    }

    fn handle_timer_tick(&mut self, context: &mut Context) {
        if self.state != TimerState::Running {
            return;
        }

        if self.remaining_secs > 0 {
            self.remaining_secs -= 1;
        }

        if self.remaining_secs == 0 {
            self.clock.stop(context);
            self.state = TimerState::Ready;

            // Transition logic after session completion
            match self.mode {
                Mode::Focus => {
                    self.completed_focus_count += 1;
                    self.save(context);
                    if self.completed_focus_count % 4 == 0 {
                        self.mode = Mode::LongBreak;
                    } else {
                        self.mode = Mode::ShortBreak;
                    }
                }
                Mode::ShortBreak | Mode::LongBreak => {
                    self.mode = Mode::Focus;
                }
            }
            self.remaining_secs = self.mode.duration_secs();
        }

        self.show(context);
    }

    fn start_timer(&mut self, context: &mut Context) {
        self.state = TimerState::Running;
        self.clock.start(context);
        self.show(context);
    }

    fn pause_timer(&mut self, context: &mut Context) {
        self.state = TimerState::Paused;
        self.clock.stop(context);
        self.show(context);
    }

    fn resume_timer(&mut self, context: &mut Context) {
        self.state = TimerState::Running;
        self.clock.start(context);
        self.show(context);
    }

    fn reset_timer(&mut self, context: &mut Context) {
        self.clock.stop(context);
        self.state = TimerState::Ready;
        self.remaining_secs = self.mode.duration_secs();
        self.show(context);
    }

    fn skip_session(&mut self, context: &mut Context) {
        self.clock.stop(context);
        self.state = TimerState::Ready;
        match self.mode {
            Mode::Focus => {
                // Determine next break mode without completing focus session
                let next_completed = self.completed_focus_count + 1;
                if next_completed % 4 == 0 {
                    self.mode = Mode::LongBreak;
                } else {
                    self.mode = Mode::ShortBreak;
                }
            }
            Mode::ShortBreak | Mode::LongBreak => {
                self.mode = Mode::Focus;
            }
        }
        self.remaining_secs = self.mode.duration_secs();
        self.show(context);
    }
}

impl KoboApp for Pomodoro {
    fn on_start(&mut self, context: &mut Context) {
        context.store().load(STORE_KEY);
        self.show(context);
    }

    fn on_store(&mut self, context: &mut Context, result: StoreResult) {
        match result {
            StoreResult::Loaded { value, .. } => {
                self.completed_focus_count = value.map(|bytes| decode(&bytes)).unwrap_or(0);
                self.loaded = true;
                self.show(context);
            }
            StoreResult::Denied(reason) => {
                self.loaded = true;
                context.log(
                    LogLevel::Warn,
                    format!("Pomodoro state load/save issue: {reason}"),
                );
                self.show(context);
            }
            _ => {}
        }
    }

    fn on_task(&mut self, context: &mut Context, task: TaskId, outcome: TaskOutcome) {
        if self.clock.on_task(context, task, &outcome) {
            self.handle_timer_tick(context);
        }
    }

    fn on_action(&mut self, context: &mut Context, action: ActionId) {
        if action == action_id(ACTION_START) {
            self.start_timer(context);
        } else if action == action_id(ACTION_PAUSE) {
            self.pause_timer(context);
        } else if action == action_id(ACTION_RESUME) {
            self.resume_timer(context);
        } else if action == action_id(ACTION_RESET) {
            self.reset_timer(context);
        } else if action == action_id(ACTION_SKIP) {
            self.skip_session(context);
        }
    }
}

fn main() -> ExitCode {
    match kobo_sdk::run("pomodoro", Pomodoro::default()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pomodoro: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_time_produces_mm_ss() {
        assert_eq!(Pomodoro::format_time(1500), "25:00");
        assert_eq!(Pomodoro::format_time(300), "05:00");
        assert_eq!(Pomodoro::format_time(900), "15:00");
        assert_eq!(Pomodoro::format_time(65), "01:05");
        assert_eq!(Pomodoro::format_time(0), "00:00");
    }

    #[test]
    fn encode_decode_round_trip() {
        let bytes = encode(7);
        assert_eq!(decode(&bytes), 7);
    }

    #[test]
    fn decode_handles_invalid_or_empty_bytes() {
        assert_eq!(decode(b"invalid data"), 0);
        assert_eq!(decode(b""), 0);
    }

    #[test]
    fn mode_durations_are_correct() {
        assert_eq!(Mode::Focus.duration_secs(), 1500);
        assert_eq!(Mode::ShortBreak.duration_secs(), 300);
        assert_eq!(Mode::LongBreak.duration_secs(), 900);
    }

    #[test]
    fn initial_state_defaults() {
        let app = Pomodoro::default();
        assert_eq!(app.mode, Mode::Focus);
        assert_eq!(app.state, TimerState::Ready);
        assert_eq!(app.remaining_secs, 1500);
        assert_eq!(app.completed_focus_count, 0);
    }

    #[test]
    fn timer_controls_and_state_transitions() {
        let mut app = Pomodoro::default();
        let mut context = Context::default();

        // Start
        app.start_timer(&mut context);
        assert_eq!(app.state, TimerState::Running);

        // Pause
        app.pause_timer(&mut context);
        assert_eq!(app.state, TimerState::Paused);

        // Resume
        app.resume_timer(&mut context);
        assert_eq!(app.state, TimerState::Running);

        // Reset
        app.remaining_secs = 500;
        app.reset_timer(&mut context);
        assert_eq!(app.state, TimerState::Ready);
        assert_eq!(app.remaining_secs, 1500);
    }

    #[test]
    fn focus_completion_switches_to_short_break() {
        let mut app = Pomodoro::default();
        let mut context = Context::default();

        app.state = TimerState::Running;
        app.remaining_secs = 1;
        app.handle_timer_tick(&mut context);

        assert_eq!(app.completed_focus_count, 1);
        assert_eq!(app.mode, Mode::ShortBreak);
        assert_eq!(app.remaining_secs, 300);
        assert_eq!(app.state, TimerState::Ready);
    }

    #[test]
    fn four_focus_sessions_switches_to_long_break() {
        let mut app = Pomodoro::default();
        let mut context = Context::default();

        app.completed_focus_count = 3;
        app.state = TimerState::Running;
        app.remaining_secs = 1;
        app.handle_timer_tick(&mut context);

        assert_eq!(app.completed_focus_count, 4);
        assert_eq!(app.mode, Mode::LongBreak);
        assert_eq!(app.remaining_secs, 900);
        assert_eq!(app.state, TimerState::Ready);
    }

    #[test]
    fn break_completion_switches_back_to_focus() {
        let mut app = Pomodoro::default();
        let mut context = Context::default();

        app.mode = Mode::ShortBreak;
        app.remaining_secs = 1;
        app.state = TimerState::Running;
        app.handle_timer_tick(&mut context);

        assert_eq!(app.mode, Mode::Focus);
        assert_eq!(app.remaining_secs, 1500);
        assert_eq!(app.state, TimerState::Ready);
    }

    #[test]
    fn skip_advances_to_next_mode() {
        let mut app = Pomodoro::default();
        let mut context = Context::default();

        // Focus -> ShortBreak
        app.skip_session(&mut context);
        assert_eq!(app.mode, Mode::ShortBreak);
        assert_eq!(app.remaining_secs, 300);

        // ShortBreak -> Focus
        app.skip_session(&mut context);
        assert_eq!(app.mode, Mode::Focus);
        assert_eq!(app.remaining_secs, 1500);

        // 3 completed Focus sessions + Skip -> LongBreak
        app.completed_focus_count = 3;
        app.skip_session(&mut context);
        assert_eq!(app.mode, Mode::LongBreak);
        assert_eq!(app.remaining_secs, 900);
    }
}
