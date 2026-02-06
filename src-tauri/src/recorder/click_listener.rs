//! Click listener using macOS CGEventTap for global mouse click monitoring.
//!
//! This module provides a `ClickListener` that captures global mouse clicks
//! using the Core Graphics event tap API and delivers them through a channel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use core_foundation::runloop::{kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoop};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    EventField,
};

use super::click_event::{ClickEvent, MouseButton};

/// A listener for global mouse click events on macOS.
///
/// Uses CGEventTap to passively monitor mouse clicks and delivers
/// them through a channel for processing.
pub struct ClickListener {
    running: Arc<AtomicBool>,
    receiver: Receiver<ClickEvent>,
    run_loop: Arc<std::sync::Mutex<Option<CFRunLoop>>>,
    _handle: JoinHandle<()>,
}

impl ClickListener {
    /// Start listening for mouse clicks.
    ///
    /// Creates a CGEventTap in a background thread and returns immediately.
    /// Returns an error if the event tap cannot be created (usually due to
    /// missing accessibility permissions).
    pub fn start() -> Result<Self, String> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let (tx, rx) = mpsc::channel::<ClickEvent>();
        let (setup_tx, setup_rx) = mpsc::channel::<Result<(), String>>();
        let run_loop_holder: Arc<std::sync::Mutex<Option<CFRunLoop>>> =
            Arc::new(std::sync::Mutex::new(None));
        let run_loop_clone = Arc::clone(&run_loop_holder);

        let handle = thread::spawn(move || {
            Self::run_event_loop(running_clone, tx, setup_tx, run_loop_clone);
        });

        // Wait for the event tap to be set up (with timeout)
        match setup_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self {
                running,
                receiver: rx,
                run_loop: run_loop_holder,
                _handle: handle,
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Timeout waiting for event tap setup".to_string()),
        }
    }

    /// Run the event loop in a background thread.
    fn run_event_loop(
        running: Arc<AtomicBool>,
        tx: Sender<ClickEvent>,
        setup_tx: Sender<Result<(), String>>,
        run_loop_holder: Arc<std::sync::Mutex<Option<CFRunLoop>>>,
    ) {
        // Create event tap for left and right mouse down events
        let events_of_interest = vec![CGEventType::LeftMouseDown, CGEventType::RightMouseDown];

        let tx_clone = tx.clone();
        let tap_result = CGEventTap::new(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            events_of_interest,
            move |_proxy, event_type, event| {
                let location = event.location();
                let button = match event_type {
                    CGEventType::LeftMouseDown => MouseButton::Left,
                    CGEventType::RightMouseDown => MouseButton::Right,
                    _ => return None,
                };

                // Get click count (1 = single, 2 = double, 3 = triple)
                let click_count = event.get_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE);

                let click_event =
                    ClickEvent::new(location.x as i32, location.y as i32, button, click_count);

                // Send event, ignoring errors if receiver is dropped
                let _ = tx_clone.send(click_event);

                // Return None to pass the event through unchanged (passive tap)
                None
            },
        );

        let tap = match tap_result {
            Ok(tap) => tap,
            Err(()) => {
                let _ = setup_tx.send(Err(
                    "Failed to create event tap. Check accessibility permissions.".to_string()
                ));
                return;
            }
        };

        // Create a run loop source from the event tap's mach port
        let loop_source = match tap.mach_port.create_runloop_source(0) {
            Ok(source) => source,
            Err(()) => {
                let _ = setup_tx.send(Err("Failed to create run loop source".to_string()));
                return;
            }
        };

        // Get the current run loop and add the source
        let current_run_loop = CFRunLoop::get_current();

        // Store the run loop reference for stopping later
        {
            let mut holder = run_loop_holder.lock().unwrap();
            *holder = Some(current_run_loop.clone());
        }

        unsafe {
            current_run_loop.add_source(&loop_source, kCFRunLoopCommonModes);
        }

        // Enable the tap
        tap.enable();

        // Signal that setup is complete
        let _ = setup_tx.send(Ok(()));

        // Run the event loop until stopped
        // We use run_in_mode with a timeout to periodically check the running flag
        // Note: kCFRunLoopDefaultMode must be used here, not kCFRunLoopCommonModes
        // (kCFRunLoopCommonModes is a pseudo-mode for adding sources, not for running)
        while running.load(Ordering::SeqCst) {
            let result = unsafe {
                CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, Duration::from_millis(100), true)
            };

            // If the run loop was stopped externally, break
            if result == core_foundation::runloop::CFRunLoopRunResult::Stopped {
                break;
            }
        }

        // Clean up
        unsafe {
            current_run_loop.remove_source(&loop_source, kCFRunLoopCommonModes);
        }
    }

    /// Signal the listener to stop.
    ///
    /// This sets the running flag to false and stops the CFRunLoop,
    /// causing the background thread to exit.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);

        // Stop the run loop if we have a reference to it
        if let Ok(holder) = self.run_loop.lock() {
            if let Some(ref run_loop) = *holder {
                run_loop.stop();
            }
        }
    }

    /// Try to receive a click event without blocking.
    ///
    /// Returns `Some(ClickEvent)` if an event is available, `None` otherwise.
    #[allow(dead_code)]
    pub fn try_recv(&self) -> Option<ClickEvent> {
        match self.receiver.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Receive a click event with timeout.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<ClickEvent> {
        self.receiver.recv_timeout(timeout).ok()
    }

    #[cfg(test)]
    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for ClickListener {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_listener_starts_and_stops() {
        // Note: This test may fail without accessibility permissions
        // In that case, it should return an error rather than panic
        match ClickListener::start() {
            Ok(listener) => {
                // Verify it's running
                assert!(listener.is_running());

                // Stop should not panic
                listener.stop();

                // Give it a moment to process the stop
                std::thread::sleep(Duration::from_millis(200));

                // Verify it's stopped
                assert!(!listener.is_running());
            }
            Err(e) => {
                // This is expected if accessibility permissions are not granted
                println!("Click listener could not start (expected without permissions): {e}");
                assert!(e.contains("accessibility") || e.contains("permission") || e.contains("event tap"));
            }
        }
    }

    #[test]
    fn click_listener_recv_timeout_returns_none_when_empty() {
        if let Ok(listener) = ClickListener::start() {
            let got = listener.recv_timeout(Duration::from_millis(10));
            assert!(got.is_none());
        }
    }

    #[test]
    fn click_listener_running_flag_changes() {
        match ClickListener::start() {
            Ok(listener) => {
                // Initially running
                assert!(listener.is_running());

                // After stop, flag should be false
                listener.stop();
                std::thread::sleep(Duration::from_millis(200));
                assert!(!listener.is_running());
            }
            Err(e) => {
                // This is expected if accessibility permissions are not granted
                println!("Click listener could not start (expected without permissions): {e}");
            }
        }
    }

    #[test]
    fn click_listener_try_recv_returns_none_when_empty() {
        match ClickListener::start() {
            Ok(listener) => {
                // Should return None when no events are available
                assert!(listener.try_recv().is_none());
                listener.stop();
            }
            Err(e) => {
                println!("Click listener could not start (expected without permissions): {e}");
            }
        }
    }
}
