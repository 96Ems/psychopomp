//! Request eligibility, invalidation and completion belong together. This module
//! owns no Playback clock, pixels, window or GPU. Submission equality includes
//! time/phase; completion freshness intentionally accepts older same-revision samples.
use crate::render::{GridLinePalette, Theme};
use kinograph::playback::PlaybackSample;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RequestStamp {
    pub slide_index: usize,
    pub sample: PlaybackSample,
    pub palette: GridLinePalette,
    pub theme: Theme,
    pub debug: bool,
}
impl RequestStamp {
    fn is_current(self, current: Self) -> bool {
        self.slide_index == current.slide_index
            && self.sample.revision == current.sample.revision
            && self.palette == current.palette
            && self.theme == current.theme
            && self.debug == current.debug
    }
}

pub(super) enum Event {
    SlideChanged,
    AppearanceChanged,
    InspectionChanged,
    Repaint,
    VisibilityChanged(bool),
    Presented,
    Retry { now: Instant, interval: Duration },
}
pub(super) struct FrameScheduler {
    busy: bool,
    submitted: Option<PlaybackSample>,
    next_frame: Option<Instant>,
    dirty: bool,
    occluded: bool,
}
impl FrameScheduler {
    pub(super) fn new() -> Self {
        Self {
            busy: false,
            submitted: None,
            next_frame: None,
            dirty: true,
            occluded: false,
        }
    }
    pub(super) fn can_sample(&self) -> bool {
        !self.busy && !self.occluded
    }
    pub(super) fn needs_paint(&self) -> bool {
        self.dirty
    }
    pub(super) fn event(&mut self, event: Event) {
        match event {
            Event::SlideChanged => {
                self.submitted = None;
                self.dirty = false;
            }
            Event::AppearanceChanged => {
                self.submitted = None;
                self.next_frame = None;
            }
            Event::InspectionChanged => self.next_frame = None,
            Event::Repaint => self.dirty = true,
            Event::VisibilityChanged(occluded) => {
                self.occluded = occluded;
                if !occluded {
                    self.dirty = true;
                }
            }
            Event::Presented => {
                self.occluded = false;
                self.dirty = false;
            }
            Event::Retry { now, interval } => self.next_frame = Some(now + interval),
        }
    }
    /// Commit only a successful send. Eligibility uses pre-send time; the next
    /// deadline uses post-send time, preserving the host's existing attribution.
    pub(super) fn request<E>(
        &mut self,
        sample: PlaybackSample,
        interval: Duration,
        mut now: impl FnMut() -> Instant,
        send: impl FnOnce() -> Result<(), E>,
    ) -> Result<bool, E> {
        if !self.can_sample() || self.submitted == Some(sample) {
            return Ok(false);
        }
        let same_revision = self
            .submitted
            .is_some_and(|old| old.revision == sample.revision);
        if same_revision && self.next_frame.is_some_and(|deadline| now() < deadline) {
            return Ok(false);
        }
        send()?;
        self.busy = true;
        self.submitted = Some(sample);
        self.next_frame = Some(next_deadline(self.next_frame, now(), interval));
        Ok(true)
    }
    /// Stale completions release the worker but never replace front pixels or
    /// acknowledge an explicitly invalidated submission. Sample current Playback
    /// after releasing the slot, as in the original event handler.
    pub(super) fn complete(
        &mut self,
        completed: RequestStamp,
        current: impl FnOnce() -> RequestStamp,
    ) -> bool {
        self.busy = false;
        let accepted = completed.is_current(current());
        if accepted {
            self.dirty = true;
        }
        accepted
    }
    /// Waiting deliberately retains its deadline, even if an explicit redraw
    /// could submit a new revision immediately. Do not clamp a past deadline.
    pub(super) fn wake_at(
        &self,
        sample: PlaybackSample,
        drawable: bool,
        now: impl FnOnce() -> Instant,
    ) -> Option<Instant> {
        if !self.can_sample() || !drawable || (self.submitted == Some(sample) && !self.dirty) {
            return None;
        }
        Some(self.next_frame.unwrap_or_else(now))
    }
}
fn next_deadline(previous: Option<Instant>, now: Instant, interval: Duration) -> Instant {
    let Some(deadline) = previous else {
        return now + interval;
    };
    if deadline > now {
        return deadline;
    }
    let remainder = now.duration_since(deadline).as_nanos() % interval.as_nanos();
    now + interval - Duration::from_nanos(remainder as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinograph::playback::PlaybackPhase;
    const INTERVAL: Duration = Duration::from_millis(10);

    #[test]
    fn phase_changes_resubmit_but_inspection_of_an_identical_hold_does_not() {
        let now = Instant::now();
        let playing = stamp(0, 0);
        let held = RequestStamp {
            sample: PlaybackSample {
                phase: PlaybackPhase::Held,
                ..playing.sample
            },
            ..playing
        };
        let mut s = FrameScheduler::new();
        assert!(send(&mut s, playing, now));
        assert!(s.complete(playing, || held));
        s.event(Event::Presented);
        assert!(send(&mut s, held, now + INTERVAL));
        assert!(s.complete(held, || held));
        s.event(Event::Presented);
        s.event(Event::InspectionChanged);
        assert!(!send(&mut s, held, now + INTERVAL * 10));
        assert_eq!(s.wake_at(held.sample, true, || now), None);
    }
    #[test]
    fn stale_completion_preserves_dirty_and_does_not_clear_occlusion() {
        let now = Instant::now();
        let a = stamp(0, 0);
        let b = stamp(1, 0);
        let mut s = FrameScheduler::new();
        s.event(Event::Presented);
        assert!(send(&mut s, a, now));
        s.event(Event::Repaint);
        s.event(Event::VisibilityChanged(true));
        assert!(!s.complete(a, || b));
        assert!(s.needs_paint());
        assert!(!s.can_sample());
        s.event(Event::VisibilityChanged(false));
        assert!(s.needs_paint());
        assert!(s.can_sample());
    }
    #[test]
    fn failure_keeps_existing_deadline_and_refresh_changes_do_not_rebase_it() {
        let now = Instant::now();
        let a = stamp(0, 0);
        let b = stamp(1, 0);
        let mut s = FrameScheduler::new();
        s.event(Event::Retry {
            now,
            interval: INTERVAL,
        });
        assert!(send(&mut s, a, now));
        assert!(s.complete(a, || a));
        s.event(Event::Presented);
        assert_eq!(
            s.request(b.sample, INTERVAL, || now, || Err("closed")),
            Err("closed")
        );
        assert_eq!(s.wake_at(b.sample, true, || now), Some(now + INTERVAL));
        assert!(!send(&mut s, a, now + INTERVAL));
        assert!(
            s.request(
                stamp(0, 1).sample,
                Duration::from_millis(8),
                || now + Duration::from_millis(11),
                || Ok::<_, ()>(())
            )
            .unwrap()
        );
        assert!(s.complete(stamp(0, 1), || stamp(0, 1)));
        s.event(Event::Presented);
        assert_eq!(
            s.wake_at(stamp(0, 2).sample, true, || now),
            Some(now + Duration::from_millis(18))
        );
    }
    fn stamp(revision: u64, millis: u64) -> RequestStamp {
        RequestStamp {
            slide_index: 0,
            sample: PlaybackSample {
                step_index: 0,
                at_nanos: millis * 1_000_000,
                phase: PlaybackPhase::Playing,
                revision,
            },
            palette: GridLinePalette::Orange,
            theme: Theme::Original,
            debug: false,
        }
    }
    fn send(s: &mut FrameScheduler, p: RequestStamp, now: Instant) -> bool {
        s.request(p.sample, INTERVAL, || now, || Ok::<_, ()>(()))
            .unwrap()
    }
    #[test]
    fn moving_completions_are_accepted_but_only_one_distinct_sample_is_in_flight() {
        let now = Instant::now();
        let a = stamp(0, 0);
        let b = stamp(0, 5);
        let mut s = FrameScheduler::new();
        s.event(Event::Presented);
        assert!(send(&mut s, a, now));
        assert!(!send(&mut s, b, now + INTERVAL));
        assert_eq!(s.wake_at(b.sample, true, || now), None);
        assert!(s.complete(a, || b));
        assert!(s.needs_paint());
        s.event(Event::Presented);
        assert!(!send(&mut s, b, now + Duration::from_millis(5)));
        assert!(send(&mut s, b, now + INTERVAL));
        assert!(s.complete(b, || b));
        s.event(Event::Presented);
        assert!(!send(&mut s, b, now + INTERVAL * 10));
        assert_eq!(s.wake_at(b.sample, true, || now), None);
    }
    #[test]
    fn invalidation_while_busy_rejects_stale_frames_and_releases_the_slot() {
        let now = Instant::now();
        let a = stamp(0, 0);
        for (event, b) in [
            (
                Event::SlideChanged,
                RequestStamp {
                    slide_index: 1,
                    ..a
                },
            ),
            (
                Event::AppearanceChanged,
                RequestStamp {
                    palette: GridLinePalette::Copper,
                    ..a
                },
            ),
            (
                Event::AppearanceChanged,
                RequestStamp {
                    theme: Theme::Black,
                    ..a
                },
            ),
            (Event::AppearanceChanged, RequestStamp { debug: true, ..a }),
            (Event::InspectionChanged, stamp(1, 0)),
        ] {
            let mut s = FrameScheduler::new();
            s.event(Event::Presented);
            assert!(send(&mut s, a, now));
            s.event(event);
            assert!(!send(&mut s, b, now));
            assert!(!s.complete(a, || b));
            assert!(!s.needs_paint());
            assert!(send(&mut s, b, now));
            assert!(s.complete(b, || b));
        }
    }
    #[test]
    fn failed_send_commits_nothing_and_success_uses_post_send_time() {
        let now = Instant::now();
        let clock = std::cell::Cell::new(now);
        let a = stamp(0, 0);
        let mut s = FrameScheduler::new();
        s.event(Event::Presented);
        assert_eq!(
            s.request(
                a.sample,
                INTERVAL,
                || clock.get(),
                || {
                    clock.set(now + INTERVAL);
                    Err("closed")
                }
            ),
            Err("closed")
        );
        assert!(s.can_sample());
        assert_eq!(s.wake_at(a.sample, true, || now), Some(now));
        assert!(
            s.request(
                a.sample,
                INTERVAL,
                || clock.get(),
                || {
                    clock.set(now + INTERVAL * 2);
                    Ok::<_, ()>(())
                }
            )
            .unwrap()
        );
        assert!(s.complete(a, || a));
        s.event(Event::Presented);
        assert_eq!(
            s.wake_at(stamp(0, 1).sample, true, || now),
            Some(now + INTERVAL * 3)
        );
        assert!(send(&mut s, stamp(1, 0), now + INTERVAL));
        assert!(s.complete(stamp(1, 0), || stamp(1, 0)));
        s.event(Event::Presented);
        assert_eq!(
            s.wake_at(stamp(1, 1).sample, true, || now),
            Some(now + INTERVAL * 3)
        );
    }
    #[test]
    fn slide_switch_keeps_cadence_while_retry_reanchors_and_repaint_does_not_render() {
        let now = Instant::now();
        let a = stamp(0, 0);
        let b = RequestStamp {
            slide_index: 1,
            ..a
        };
        let mut s = FrameScheduler::new();
        assert!(send(&mut s, a, now));
        assert!(s.complete(a, || a));
        s.event(Event::SlideChanged);
        assert!(!s.needs_paint());
        assert_eq!(s.wake_at(b.sample, true, || now), Some(now + INTERVAL));
        assert!(send(&mut s, b, now));
        assert!(s.complete(b, || b));
        s.event(Event::Presented);
        s.event(Event::Repaint);
        assert!(!send(&mut s, b, now + INTERVAL * 10));
        assert_eq!(s.wake_at(b.sample, false, || now), None);
        s.event(Event::Retry {
            now: now + INTERVAL,
            interval: INTERVAL,
        });
        assert_eq!(s.wake_at(b.sample, true, || now), Some(now + INTERVAL * 2));
        s.event(Event::VisibilityChanged(true));
        assert!(!s.can_sample());
        assert!(s.needs_paint());
        s.event(Event::Presented);
        assert!(s.can_sample());
        assert!(!s.needs_paint());
    }
    #[test]
    fn appearance_round_trip_accepts_old_provenance_without_acknowledging_invalidation() {
        let now = Instant::now();
        let a = stamp(0, 0);
        let mut s = FrameScheduler::new();
        assert!(send(&mut s, a, now));
        s.event(Event::AppearanceChanged);
        s.event(Event::AppearanceChanged);
        assert!(s.complete(a, || a));
        assert!(send(&mut s, a, now));
        s.event(Event::Presented);
        assert!(!s.can_sample());
    }
    #[test]
    fn all_appearance_pairs_check_provenance_not_time_phase_or_step() {
        let a = stamp(7, 0);
        for palette in GridLinePalette::ALL {
            for theme in Theme::ALL {
                for debug in [false, true] {
                    let completed = RequestStamp {
                        palette,
                        theme,
                        debug,
                        ..a
                    };
                    for new_palette in GridLinePalette::ALL {
                        for new_theme in Theme::ALL {
                            for new_debug in [false, true] {
                                let current = RequestStamp {
                                    palette: new_palette,
                                    theme: new_theme,
                                    debug: new_debug,
                                    sample: PlaybackSample {
                                        phase: PlaybackPhase::Held,
                                        step_index: 8,
                                        ..stamp(7, 500).sample
                                    },
                                    ..a
                                };
                                let mut s = FrameScheduler::new();
                                assert!(send(&mut s, completed, Instant::now()));
                                assert_eq!(
                                    s.complete(completed, || current),
                                    palette == new_palette
                                        && theme == new_theme
                                        && debug == new_debug
                                );
                                assert!(s.can_sample());
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn late_wakes_skip_missed_slots_without_cadence_drift() {
        let now = Instant::now();
        let mut deadline = None;
        for (wake, expected) in [(0, 10), (3, 10), (44, 50), (51, 60)] {
            let actual = next_deadline(deadline, now + Duration::from_millis(wake), INTERVAL);
            assert_eq!(actual, now + Duration::from_millis(expected));
            deadline = Some(actual);
        }
    }
}
