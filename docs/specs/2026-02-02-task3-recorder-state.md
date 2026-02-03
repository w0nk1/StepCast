# Task 3: Recorder State Machine

Goal: define minimal recorder session state machine API.

Requirements:
- SessionState enum with Idle/Recording/Paused/Stopped.
- RecorderState with start/pause/resume/stop methods and tests for flow + idle pause error.

Non-goals:
- Integrating with UI, persistence, or capture.
