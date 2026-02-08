#[derive(Debug, Clone, PartialEq)]
pub enum ExtractionState {
    Scanning { skip_count: u32 },
    Locked { consecutive_text_frames: u32 },
    Cooldown { remaining_frames: u32 },
}

impl ExtractionState {
    pub fn new() -> Self {
        ExtractionState::Scanning { skip_count: 5 }
    }

    pub fn transition(
        &self,
        has_text: bool,
        is_duplicate: bool,
        config: &StateConfig,
    ) -> (ExtractionState, StateAction) {
        match self {
            ExtractionState::Scanning { skip_count } => {
                if has_text {
                    if 1 >= config.min_lock_frames {
                        (
                            ExtractionState::Cooldown {
                                remaining_frames: config.cooldown_frames,
                            },
                            if is_duplicate {
                                StateAction::Drop
                            } else {
                                StateAction::Extract
                            },
                        )
                    } else {
                        (
                            ExtractionState::Locked {
                                consecutive_text_frames: 1,
                            },
                            StateAction::Continue,
                        )
                    }
                } else {
                    let new_skip = (*skip_count + 1).min(config.max_skip);
                    (
                        ExtractionState::Scanning {
                            skip_count: new_skip,
                        },
                        StateAction::SkipFrames(*skip_count),
                    )
                }
            }

            ExtractionState::Locked {
                consecutive_text_frames,
            } => {
                if has_text {
                    let new_count = consecutive_text_frames + 1;
                    if new_count >= config.min_lock_frames {
                        (
                            ExtractionState::Cooldown {
                                remaining_frames: config.cooldown_frames,
                            },
                            if is_duplicate {
                                StateAction::Drop
                            } else {
                                StateAction::Extract
                            },
                        )
                    } else {
                        (
                            ExtractionState::Locked {
                                consecutive_text_frames: new_count,
                            },
                            StateAction::Continue,
                        )
                    }
                } else {
                    if *consecutive_text_frames >= config.min_lock_frames / 2 {
                        (
                            ExtractionState::Cooldown {
                                remaining_frames: config.cooldown_frames,
                            },
                            if is_duplicate {
                                StateAction::Drop
                            } else {
                                StateAction::Extract
                            },
                        )
                    } else {
                        (
                            ExtractionState::Scanning {
                                skip_count: config.initial_skip,
                            },
                            StateAction::Continue,
                        )
                    }
                }
            }

            ExtractionState::Cooldown { remaining_frames } => {
                let new_remaining = remaining_frames.saturating_sub(1);
                if new_remaining == 0 {
                    (
                        ExtractionState::Scanning {
                            skip_count: config.initial_skip,
                        },
                        StateAction::Continue,
                    )
                } else {
                    (
                        ExtractionState::Cooldown {
                            remaining_frames: new_remaining,
                        },
                        StateAction::SkipFrames(1),
                    )
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StateAction {
    Continue,
    SkipFrames(u32),
    Extract,
    Drop,
}

#[derive(Debug, Clone)]
pub struct StateConfig {
    pub initial_skip: u32,
    pub max_skip: u32,
    pub min_lock_frames: u32,
    pub cooldown_frames: u32,
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            initial_skip: 5,
            max_skip: 15,
            min_lock_frames: 3,
            cooldown_frames: 30,
        }
    }
}

impl StateConfig {
    pub fn for_high_motion() -> Self {
        Self {
            initial_skip: 2,
            max_skip: 8,
            min_lock_frames: 2,
            cooldown_frames: 20,
        }
    }

    pub fn for_low_motion() -> Self {
        Self {
            initial_skip: 8,
            max_skip: 20,
            min_lock_frames: 5,
            cooldown_frames: 45,
        }
    }
}

pub struct StateMachine {
    state: ExtractionState,
    config: StateConfig,
    frame_counter: u64,
}

impl StateMachine {
    pub fn new() -> Self {
        Self::with_config(StateConfig::default())
    }

    pub fn with_config(config: StateConfig) -> Self {
        Self {
            state: ExtractionState::Scanning {
                skip_count: config.initial_skip,
            },
            config,
            frame_counter: 0,
        }
    }

    pub fn process_frame(&mut self, has_text: bool, is_duplicate: bool) -> StateAction {
        self.frame_counter += 1;

        let (new_state, action) = self.state.transition(has_text, is_duplicate, &self.config);
        self.state = new_state;

        action
    }

    pub fn current_state(&self) -> &ExtractionState {
        &self.state
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_counter
    }

    pub fn reset(&mut self) {
        self.state = ExtractionState::new();
        self.frame_counter = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanning_to_locked() {
        let mut sm = StateMachine::new();

        assert!(matches!(
            sm.current_state(),
            ExtractionState::Scanning { .. }
        ));

        let action = sm.process_frame(true, false);
        assert!(matches!(sm.current_state(), ExtractionState::Locked { .. }));
        assert_eq!(action, StateAction::Continue);
    }

    #[test]
    fn test_locked_to_extract() {
        let config = StateConfig {
            min_lock_frames: 2,
            ..Default::default()
        };
        let mut sm = StateMachine::with_config(config);

        sm.process_frame(true, false);
        let action = sm.process_frame(true, false);

        assert_eq!(action, StateAction::Extract);
        assert!(matches!(
            sm.current_state(),
            ExtractionState::Cooldown { .. }
        ));
    }

    #[test]
    fn test_duplicate_frame_dropped() {
        let config = StateConfig {
            min_lock_frames: 1,
            cooldown_frames: 1,
            initial_skip: 3,
            ..Default::default()
        };
        let mut sm = StateMachine::with_config(config);

        let action1 = sm.process_frame(true, false);
        assert_eq!(action1, StateAction::Extract);
        assert!(matches!(
            sm.current_state(),
            ExtractionState::Cooldown { .. }
        ));

        sm.process_frame(false, false);
        assert!(matches!(
            sm.current_state(),
            ExtractionState::Scanning { .. }
        ));

        let action2 = sm.process_frame(true, true);
        assert_eq!(action2, StateAction::Drop);
    }

    #[test]
    fn test_cooldown_to_scanning() {
        let config = StateConfig {
            min_lock_frames: 1,
            cooldown_frames: 2,
            initial_skip: 3,
            ..Default::default()
        };
        let mut sm = StateMachine::with_config(config);

        sm.process_frame(true, false);
        assert!(matches!(
            sm.current_state(),
            ExtractionState::Cooldown {
                remaining_frames: 2
            }
        ));

        sm.process_frame(false, false);
        assert!(matches!(
            sm.current_state(),
            ExtractionState::Cooldown {
                remaining_frames: 1
            }
        ));

        sm.process_frame(false, false);
        assert!(matches!(
            sm.current_state(),
            ExtractionState::Scanning { skip_count: 3 }
        ));
    }

    #[test]
    fn test_scanning_skip_increment() {
        let config = StateConfig {
            initial_skip: 2,
            max_skip: 5,
            ..Default::default()
        };
        let mut sm = StateMachine::with_config(config);

        let action1 = sm.process_frame(false, false);
        assert!(matches!(action1, StateAction::SkipFrames(2)));
        assert!(matches!(
            sm.current_state(),
            ExtractionState::Scanning { skip_count: 3 }
        ));

        let action2 = sm.process_frame(false, false);
        assert!(matches!(action2, StateAction::SkipFrames(3)));
        assert!(matches!(
            sm.current_state(),
            ExtractionState::Scanning { skip_count: 4 }
        ));
    }
}
