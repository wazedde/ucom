use crossterm::{execute, style::Print};
use std::io::{IsTerminal, stderr};

pub enum NativeProgressState {
    /// Hides the native progress indicator.
    Hidden,
    /// Shows a determinate progress value from 0 to 100.
    Determinate(u8),
    /// Shows the native progress indicator in an error state, optionally with progress from 0 to 100.
    #[expect(dead_code)]
    Error(Option<u8>),
    /// Shows indeterminate activity using the native busy animation.
    Indeterminate,
    /// Shows the native progress indicator in a warning state with progress from 0 to 100.
    #[expect(dead_code)]
    Warning(u8),
}

/// Manages the terminal's native progress indicator using OSC 9;4 sequences.
pub struct NativeProgressBar {
    progress_state: NativeProgressState,
    supports_native_progress: bool,
}

impl Drop for NativeProgressBar {
    fn drop(&mut self) {
        if !matches!(self.progress_state, NativeProgressState::Hidden) {
            self.update(NativeProgressState::Hidden);
        }
    }
}

impl NativeProgressBar {
    pub fn new() -> Self {
        NativeProgressBar {
            progress_state: NativeProgressState::Hidden,
            supports_native_progress: stderr().is_terminal(),
        }
    }

    pub fn with_state(state: NativeProgressState) -> Self {
        let mut bar = NativeProgressBar::new();
        if !matches!(state, NativeProgressState::Hidden) {
            bar.update(state);
        }
        bar
    }

    fn write_osc_sequence(state: &NativeProgressState) {
        // Format: OSC 9;4;ST;PR ST
        // ST = State, PR = Percentage
        const ESC: &str = "\x1b";
        const ST: &str = "\x1b\\";

        let seq = match state {
            NativeProgressState::Hidden => format!("{ESC}]9;4;0{ST}"),
            NativeProgressState::Determinate(perc) => format!("{ESC}]9;4;1;{perc}{ST}"),
            NativeProgressState::Error(perc) => {
                if let Some(p) = perc {
                    format!("{ESC}]9;4;2;{p}{ST}")
                } else {
                    format!("{ESC}]9;4;2{ST}")
                }
            }
            NativeProgressState::Indeterminate => format!("{ESC}]9;4;3{ST}"),
            NativeProgressState::Warning(perc) => format!("{ESC}]9;4;4;{perc}{ST}"),
        };

        // We use stderr because progress bars usually live there
        // to keep stdout clean for actual data piping.
        let _ = execute!(stderr(), Print(seq));
    }

    pub fn update(&mut self, state: NativeProgressState) {
        self.progress_state = state;
        if self.supports_native_progress {
            Self::write_osc_sequence(&self.progress_state);
        }
    }
}
