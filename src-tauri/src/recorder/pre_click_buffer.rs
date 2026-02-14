use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BufferedFrameMeta {
    pub captured_at_ms: i64,
}

pub(crate) fn pick_frame_index(
    frames: &VecDeque<BufferedFrameMeta>,
    click_ts_ms: i64,
) -> Option<usize> {
    if frames.is_empty() {
        return None;
    }

    let mut latest_before_click: Option<usize> = None;
    for (idx, frame) in frames.iter().enumerate() {
        if frame.captured_at_ms <= click_ts_ms {
            latest_before_click = Some(idx);
        }
    }

    latest_before_click
}

#[cfg(target_os = "macos")]
mod imp {
    use std::collections::{HashMap, VecDeque};
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use image::RgbaImage;
    use screencapturekit::prelude::{
        PixelFormat, SCContentFilter, SCDisplay, SCShareableContent, SCStream,
        SCStreamConfiguration, SCStreamOutputType,
    };

    use super::{pick_frame_index, BufferedFrameMeta};
    use crate::recorder::window_info::WindowBounds;

    const MAX_RING_FRAMES: usize = 4;
    const TARGET_FPS: u32 = 16;

    #[derive(Debug, Clone)]
    pub struct PreClickCaptureResult {
        pub bounds: WindowBounds,
        pub frame_age_ms: i64,
    }

    #[derive(Clone)]
    pub struct PreClickFrameBuffer {
        inner: Arc<PreClickFrameBufferInner>,
    }

    #[derive(Debug, Clone)]
    struct DisplayTarget {
        display: SCDisplay,
        bounds: WindowBounds,
    }

    #[derive(Debug, Clone)]
    struct BufferedFrame {
        meta: BufferedFrameMeta,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    }

    #[derive(Debug)]
    struct StreamState {
        active_display_id: u32,
        stream: Option<SCStream>,
    }

    struct PreClickFrameBufferInner {
        displays: Vec<DisplayTarget>,
        frames_by_display: Arc<Mutex<HashMap<u32, VecDeque<BufferedFrame>>>>,
        stream_state: Mutex<StreamState>,
    }

    impl Drop for PreClickFrameBufferInner {
        fn drop(&mut self) {
            if let Ok(mut state) = self.stream_state.lock() {
                if let Some(stream) = state.stream.take() {
                    let _ = stream.stop_capture();
                }
            }
        }
    }

    impl PreClickFrameBuffer {
        pub fn start() -> Result<Self, String> {
            let content = SCShareableContent::get()
                .map_err(|e| format!("ScreenCaptureKit shareable content failed: {e}"))?;
            let displays_raw = content.displays();
            if displays_raw.is_empty() {
                return Err("ScreenCaptureKit returned no displays".to_string());
            }

            let displays: Vec<DisplayTarget> =
                displays_raw.into_iter().map(display_to_target).collect();
            let initial_display = displays[0].clone();

            let frames_by_display =
                Arc::new(Mutex::new(HashMap::<u32, VecDeque<BufferedFrame>>::new()));
            let stream = start_stream_for_display(
                &initial_display.display,
                initial_display.display_id(),
                Arc::clone(&frames_by_display),
            )?;

            Ok(Self {
                inner: Arc::new(PreClickFrameBufferInner {
                    displays,
                    frames_by_display,
                    stream_state: Mutex::new(StreamState {
                        active_display_id: initial_display.display_id(),
                        stream: Some(stream),
                    }),
                }),
            })
        }

        pub fn stop(&self) {
            if let Ok(mut state) = self.inner.stream_state.lock() {
                if let Some(stream) = state.stream.take() {
                    let _ = stream.stop_capture();
                }
            }
        }

        pub fn capture_for_click(
            &self,
            click_x: i32,
            click_y: i32,
            click_ts_ms: i64,
            output_path: &Path,
        ) -> Result<Option<PreClickCaptureResult>, String> {
            let Some(target) = self.find_display_for_click(click_x, click_y).cloned() else {
                return Ok(None);
            };

            let switched = self.ensure_stream_for_display(target.display_id())?;
            if switched {
                return Ok(None);
            }

            let frame = {
                let frame_map = self
                    .inner
                    .frames_by_display
                    .lock()
                    .map_err(|_| "pre-click frame map lock poisoned".to_string())?;
                let Some(ring) = frame_map.get(&target.display_id()) else {
                    return Ok(None);
                };
                let metas: VecDeque<BufferedFrameMeta> = ring.iter().map(|f| f.meta).collect();
                let Some(idx) = pick_frame_index(&metas, click_ts_ms) else {
                    return Ok(None);
                };
                ring.get(idx).cloned()
            };

            let Some(frame) = frame else {
                return Ok(None);
            };

            let image = RgbaImage::from_raw(frame.width, frame.height, frame.rgba)
                .ok_or_else(|| "pre-click frame conversion failed".to_string())?;
            image
                .save(output_path)
                .map_err(|e| format!("pre-click frame save failed: {e}"))?;

            let frame_age_ms = click_ts_ms.saturating_sub(frame.meta.captured_at_ms);
            Ok(Some(PreClickCaptureResult {
                bounds: target.bounds,
                frame_age_ms,
            }))
        }

        fn find_display_for_click(&self, x: i32, y: i32) -> Option<&DisplayTarget> {
            self.inner
                .displays
                .iter()
                .find(|d| d.bounds.contains(x, y))
                .or_else(|| self.inner.displays.first())
        }

        fn ensure_stream_for_display(&self, display_id: u32) -> Result<bool, String> {
            let mut state = self
                .inner
                .stream_state
                .lock()
                .map_err(|_| "pre-click stream state lock poisoned".to_string())?;

            if state.stream.is_some() && state.active_display_id == display_id {
                return Ok(false);
            }

            if let Some(stream) = state.stream.take() {
                let _ = stream.stop_capture();
            }

            let Some(target) = self
                .inner
                .displays
                .iter()
                .find(|d| d.display_id() == display_id)
            else {
                return Err(format!("pre-click display not found: {display_id}"));
            };

            let stream = start_stream_for_display(
                &target.display,
                target.display_id(),
                Arc::clone(&self.inner.frames_by_display),
            )?;
            state.active_display_id = display_id;
            state.stream = Some(stream);
            Ok(true)
        }
    }

    fn start_stream_for_display(
        display: &SCDisplay,
        display_id: u32,
        frames_by_display: Arc<Mutex<HashMap<u32, VecDeque<BufferedFrame>>>>,
    ) -> Result<SCStream, String> {
        let filter = SCContentFilter::create()
            .with_display(display)
            .with_excluding_windows(&[])
            .build();

        let config = SCStreamConfiguration::new()
            .with_width(display.width())
            .with_height(display.height())
            .with_pixel_format(PixelFormat::BGRA)
            .with_queue_depth(MAX_RING_FRAMES as u32)
            .with_fps(TARGET_FPS)
            .with_shows_cursor(true)
            .with_captures_audio(false);

        let mut stream = SCStream::new(&filter, &config);
        stream.add_output_handler(
            move |sample: screencapturekit::cm::CMSampleBuffer, output_type| {
                if output_type != SCStreamOutputType::Screen {
                    return;
                }
                if sample
                    .frame_status()
                    .map(|status| !status.has_content())
                    .unwrap_or(false)
                {
                    return;
                }

                let Some(pixel_buffer) = sample.image_buffer() else {
                    return;
                };
                let Ok(guard) = pixel_buffer.lock_read_only() else {
                    return;
                };

                let width = guard.width();
                let height = guard.height();
                let bytes_per_row = guard.bytes_per_row();
                if width == 0 || height == 0 || bytes_per_row < width.saturating_mul(4) {
                    return;
                }

                let raw = guard.as_slice();
                if raw.len() < bytes_per_row.saturating_mul(height) {
                    return;
                }

                let mut rgba = vec![0_u8; width.saturating_mul(height).saturating_mul(4)];
                for y in 0..height {
                    let src_row = &raw[y * bytes_per_row..y * bytes_per_row + width * 4];
                    let dst_row = &mut rgba[y * width * 4..(y + 1) * width * 4];
                    for x in 0..width {
                        let si = x * 4;
                        let di = si;
                        // ScreenCaptureKit delivers BGRA for PixelFormat::BGRA.
                        dst_row[di] = src_row[si + 2];
                        dst_row[di + 1] = src_row[si + 1];
                        dst_row[di + 2] = src_row[si];
                        dst_row[di + 3] = src_row[si + 3];
                    }
                }

                let frame = BufferedFrame {
                    meta: BufferedFrameMeta {
                        captured_at_ms: now_ms(),
                    },
                    width: width as u32,
                    height: height as u32,
                    rgba,
                };

                if let Ok(mut map) = frames_by_display.lock() {
                    let ring = map.entry(display_id).or_default();
                    push_frame(ring, frame, MAX_RING_FRAMES);
                }
            },
            SCStreamOutputType::Screen,
        );

        stream
            .start_capture()
            .map_err(|e| format!("ScreenCaptureKit start_capture failed: {e}"))?;

        Ok(stream)
    }

    fn push_frame(ring: &mut VecDeque<BufferedFrame>, frame: BufferedFrame, max_frames: usize) {
        ring.push_back(frame);
        while ring.len() > max_frames {
            let _ = ring.pop_front();
        }
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    fn display_to_target(display: SCDisplay) -> DisplayTarget {
        let frame = display.frame();
        DisplayTarget {
            bounds: WindowBounds {
                x: frame.x.round() as i32,
                y: frame.y.round() as i32,
                width: display.width(),
                height: display.height(),
            },
            display,
        }
    }

    impl DisplayTarget {
        fn display_id(&self) -> u32 {
            self.display.display_id()
        }
    }

    impl WindowBounds {
        fn contains(&self, x: i32, y: i32) -> bool {
            x >= self.x
                && x < self.x + self.width as i32
                && y >= self.y
                && y < self.y + self.height as i32
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use std::path::Path;

    use crate::recorder::window_info::WindowBounds;

    #[derive(Debug, Clone)]
    pub struct PreClickCaptureResult {
        pub bounds: WindowBounds,
        pub frame_age_ms: i64,
    }

    #[derive(Clone)]
    pub struct PreClickFrameBuffer;

    impl PreClickFrameBuffer {
        pub fn start() -> Result<Self, String> {
            Err("pre-click buffer is only available on macOS".to_string())
        }

        pub fn stop(&self) {}

        pub fn capture_for_click(
            &self,
            _click_x: i32,
            _click_y: i32,
            _click_ts_ms: i64,
            _output_path: &Path,
        ) -> Result<Option<PreClickCaptureResult>, String> {
            Ok(None)
        }
    }
}

pub use imp::PreClickFrameBuffer;

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(ts: &[i64]) -> VecDeque<BufferedFrameMeta> {
        ts.iter()
            .map(|t| BufferedFrameMeta { captured_at_ms: *t })
            .collect()
    }

    #[test]
    fn pick_frame_prefers_latest_before_click() {
        let ring = frames(&[1_000, 1_020, 1_050, 1_080]);
        let idx = pick_frame_index(&ring, 1_055).expect("frame index");
        assert_eq!(idx, 2);
    }

    #[test]
    fn pick_frame_returns_none_if_all_frames_are_after_click() {
        let ring = frames(&[2_000, 2_050, 2_100]);
        let idx = pick_frame_index(&ring, 1_900);
        assert_eq!(idx, None);
    }

    #[test]
    fn pick_frame_returns_none_for_empty_ring() {
        let ring = VecDeque::<BufferedFrameMeta>::new();
        assert_eq!(pick_frame_index(&ring, 42), None);
    }
}
