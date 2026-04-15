#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct ReconnectController {
    next_delay_index: usize,
    retry_scheduled: bool,
    retry_blocked: bool,
}

impl ReconnectController {
    const RECONNECT_DELAYS_MS: [u32; 6] = [1_000, 2_000, 5_000, 10_000, 20_000, 30_000];

    pub(super) fn allow_connect(&mut self) {
        self.retry_blocked = false;
    }

    pub(super) fn can_connect_now(&self) -> bool {
        !self.retry_blocked && !self.retry_scheduled
    }

    pub(super) fn schedule_retry(&mut self) -> Option<u32> {
        if self.retry_blocked || self.retry_scheduled {
            return None;
        }

        let delay = Self::RECONNECT_DELAYS_MS[self
            .next_delay_index
            .min(Self::RECONNECT_DELAYS_MS.len() - 1)];
        self.retry_scheduled = true;
        self.next_delay_index =
            (self.next_delay_index + 1).min(Self::RECONNECT_DELAYS_MS.len() - 1);
        Some(delay)
    }

    pub(super) fn retry_fired(&mut self) -> bool {
        if self.retry_blocked || !self.retry_scheduled {
            return false;
        }

        self.retry_scheduled = false;
        true
    }

    pub(super) fn on_open(&mut self) {
        self.next_delay_index = 0;
        self.retry_scheduled = false;
        self.retry_blocked = false;
    }

    pub(super) fn disconnect(&mut self) {
        self.next_delay_index = 0;
        self.retry_scheduled = false;
        self.retry_blocked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::ReconnectController;

    #[test]
    fn retry_delays_progress_and_cap() {
        let mut controller = ReconnectController::default();

        assert_eq!(controller.schedule_retry(), Some(1_000));
        assert!(controller.retry_fired());
        assert_eq!(controller.schedule_retry(), Some(2_000));
        assert!(controller.retry_fired());
        assert_eq!(controller.schedule_retry(), Some(5_000));
        assert!(controller.retry_fired());
        assert_eq!(controller.schedule_retry(), Some(10_000));
        assert!(controller.retry_fired());
        assert_eq!(controller.schedule_retry(), Some(20_000));
        assert!(controller.retry_fired());
        assert_eq!(controller.schedule_retry(), Some(30_000));
        assert!(controller.retry_fired());
        assert_eq!(controller.schedule_retry(), Some(30_000));
    }

    #[test]
    fn successful_open_resets_retry_progression() {
        let mut controller = ReconnectController::default();

        assert_eq!(controller.schedule_retry(), Some(1_000));
        assert!(controller.retry_fired());
        assert_eq!(controller.schedule_retry(), Some(2_000));
        controller.on_open();

        assert_eq!(controller.schedule_retry(), Some(1_000));
    }

    #[test]
    fn disconnect_blocks_retries_until_connect_is_allowed_again() {
        let mut controller = ReconnectController::default();

        controller.disconnect();
        assert_eq!(controller.schedule_retry(), None);
        assert!(!controller.retry_fired());

        controller.allow_connect();
        assert_eq!(controller.schedule_retry(), Some(1_000));
    }

    #[test]
    fn duplicate_retry_schedule_is_prevented() {
        let mut controller = ReconnectController::default();

        assert_eq!(controller.schedule_retry(), Some(1_000));
        assert_eq!(controller.schedule_retry(), None);
        assert!(controller.retry_fired());
        assert_eq!(controller.schedule_retry(), Some(2_000));
    }
}
