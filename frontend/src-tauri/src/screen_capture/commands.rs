//! Recording the screen alongside the audio a Session already captures.
//!
//! macOS 15 added SCRecordingOutput, which writes a ScreenCaptureKit stream
//! straight to a file. That removes the AVAssetWriter pipeline this would
//! otherwise need, and with it the frame-by-frame bookkeeping that goes wrong.
//!
//! The screen and the audio are two capture paths that start moments apart, so
//! the offset between them is recorded rather than assumed to be zero.

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Runtime};

use super::models::{
    is_offerable_window, CaptureTarget, CaptureTargets, ScreenCaptureError, ScreenRecordingState,
};

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use cidre::{arc, cf, cg, define_obj_type, ns, objc, sc};
    use std::path::Path;

    /// Held for the life of a recording. Dropping the stream stops capture, so
    /// it must outlive the command that started it.
    pub struct ActiveRecording {
        pub stream: arc::R<sc::Stream>,
        /// Held, never read. The stream keeps only a weak reference to its
        /// recording output; dropping this releases it and the file stops
        /// growing mid-recording.
        #[allow(dead_code)]
        pub output: arc::R<sc::RecordingOutput>,
        pub target: CaptureTarget,
        pub path: PathBuf,
        pub started_at_offset_ms: i64,
    }

    // ScreenCaptureKit objects are used from one place at a time, behind the
    // mutex below.
    unsafe impl Send for ActiveRecording {}

    // SCRecordingOutput requires a delegate. cidre's AnyDelegate is a type
    // marker with no constructor, so a real Objective-C class is declared here
    // instead. Every callback is optional, so an empty implementation would
    // do — but the ones that exist are exactly what the UI needs to know, and
    // leaving them unimplemented would mean a failed recording looks like a
    // recording still in progress.
    define_obj_type!(
        RecordingDelegate + sc::recording_output::DelegateImpl,
        usize,
        RECORDING_DELEGATE_CLS
    );

    impl sc::recording_output::Delegate for RecordingDelegate {}

    #[objc::add_methods]
    impl sc::recording_output::DelegateImpl for RecordingDelegate {}

    pub async fn shareable() -> Result<arc::R<sc::ShareableContent>, ScreenCaptureError> {
        // The first call is what triggers the system permission prompt; a
        // refusal surfaces here rather than as an empty list of targets.
        sc::ShareableContent::current()
            .await
            .map_err(|_| ScreenCaptureError::PermissionDenied)
    }

    pub async fn list_targets() -> Result<CaptureTargets, ScreenCaptureError> {
        let content = shareable().await?;

        let displays = content
            .displays()
            .iter()
            .map(|display| CaptureTarget::Display {
                id: display.display_id().0,
                width: display.width() as i64,
                height: display.height() as i64,
            })
            .collect();

        let windows = content
            .windows()
            .iter()
            .filter_map(|window| {
                let title = window.title().map(|t| t.to_string());
                let frame = window.frame();
                if !is_offerable_window(
                    title.as_deref(),
                    window.window_layer() as i64,
                    frame.size.width as i64,
                    frame.size.height as i64,
                ) {
                    return None;
                }
                Some(CaptureTarget::Window {
                    id: window.id(),
                    title: title.unwrap_or_default(),
                    app: window
                        .owning_app()
                        .map(|app| app.app_name().to_string())
                        .unwrap_or_default(),
                })
            })
            .collect();

        Ok(CaptureTargets { displays, windows })
    }

    /// A still of what recording this target would capture.
    ///
    /// A list of window titles asks someone to remember which "Untitled" is
    /// which. A picture does not. Encoded as a PNG data URI so the frontend can
    /// render it directly, with no temporary files to clean up.
    pub async fn thumbnail(target: &CaptureTarget, width: usize) -> Option<String> {
        let content = shareable().await.ok()?;

        let filter = match target {
            CaptureTarget::Display { id, .. } => {
                let displays = content.displays();
                let display = displays.iter().find(|d| d.display_id().0 == *id)?;
                sc::ContentFilter::with_display_excluding_windows(&display, &ns::Array::new())
            }
            CaptureTarget::Window { id, .. } => {
                let windows = content.windows();
                let window = windows.iter().find(|w| w.id() == *id)?;
                sc::ContentFilter::with_desktop_independent_window(&window)
            }
        };

        // Captured small on purpose. A picker showing twenty full-resolution
        // screenshots would stall opening for seconds and move tens of
        // megabytes across the IPC boundary for images drawn at 160 px.
        let mut cfg = sc::StreamCfg::new();
        let size = filter.content_rect();
        let aspect = if size.size.width > 0.0 {
            size.size.height / size.size.width
        } else {
            0.625
        };
        cfg.set_width(width);
        cfg.set_height(((width as f64) * aspect).round().max(1.0) as usize);

        let image = sc::ScreenshotManager::capture_image(&filter, &cfg)
            .await
            .ok()?;

        let mut data = cf::DataMut::with_capacity(0);
        let mut dst =
            cg::ImageDst::with_data(&mut data, cf::String::from_str("public.png").as_ref(), 1)?;
        dst.add_image(&image, None);
        if !dst.finalize() {
            return None;
        }

        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(data.as_slice());
        Some(format!("data:image/png;base64,{encoded}"))
    }

    pub async fn start(
        target: &CaptureTarget,
        destination: &Path,
        started_at_offset_ms: i64,
    ) -> Result<ActiveRecording, ScreenCaptureError> {
        let content = shareable().await?;

        let filter = match target {
            CaptureTarget::Display { id, .. } => {
                let displays = content.displays();
                let display = displays
                    .iter()
                    .find(|display| display.display_id().0 == *id)
                    .ok_or(ScreenCaptureError::TargetUnavailable)?;
                // Excluding nothing: the point of picking a display is to
                // record what is on it.
                sc::ContentFilter::with_display_excluding_windows(&display, &ns::Array::new())
            }
            CaptureTarget::Window { id, .. } => {
                let windows = content.windows();
                let window = windows
                    .iter()
                    .find(|window| window.id() == *id)
                    .ok_or(ScreenCaptureError::TargetUnavailable)?;
                sc::ContentFilter::with_desktop_independent_window(&window)
            }
        };

        let mut cfg = sc::StreamCfg::new();
        cfg.set_captures_audio(false); // Audio has its own path already.

        let mut stream = sc::Stream::new(&filter, &cfg);

        let mut output_cfg = sc::RecordingOutputCfg::new();
        let url = ns::Url::with_fs_path_str(&destination.to_string_lossy(), false);
        output_cfg.set_output_url(&url);

        // SCRecordingOutput requires a delegate object. Every method on the
        // protocol is optional, so a plain NSObject satisfies it: Objective-C
        // only dispatches the callbacks an object actually implements. cidre's
        // AnyDelegate is a type marker with no constructor, so the instance is
        // made through the runtime.
        let delegate = RecordingDelegate::with(0);
        let output = sc::RecordingOutput::with_cfg(&output_cfg, delegate.as_ref());

        stream
            .add_recording_output(&output)
            .map_err(|_| ScreenCaptureError::Unsupported)?;
        stream
            .start()
            .await
            .map_err(|_| ScreenCaptureError::Failed)?;

        Ok(ActiveRecording {
            stream,
            output,
            target: target.clone(),
            path: destination.to_path_buf(),
            started_at_offset_ms,
        })
    }

    /// Takes ownership rather than borrowing: a shared reference held across
    /// an await would require ActiveRecording to be Sync, and these are
    /// ScreenCaptureKit objects driven from one place at a time. Consuming it
    /// also guarantees the stream is dropped here, which releases the capture
    /// even if the stop call itself reports an error.
    pub async fn stop(recording: ActiveRecording) -> Result<(), ScreenCaptureError> {
        let result = recording
            .stream
            .stop()
            .await
            .map_err(|_| ScreenCaptureError::Failed);
        drop(recording);
        result
    }
}

#[cfg(target_os = "macos")]
static ACTIVE: Mutex<Option<mac::ActiveRecording>> = Mutex::new(None);

#[tauri::command]
pub async fn api_list_capture_targets<R: Runtime>(
    _app: AppHandle<R>,
) -> Result<CaptureTargets, ScreenCaptureError> {
    #[cfg(target_os = "macos")]
    {
        mac::list_targets().await
    }
    #[cfg(not(target_os = "macos"))]
    Err(ScreenCaptureError::Unsupported)
}

/// One thumbnail at a time, so the picker can list targets immediately and let
/// the pictures arrive as they are ready.
#[tauri::command]
pub async fn api_capture_target_thumbnail<R: Runtime>(
    _app: AppHandle<R>,
    target: CaptureTarget,
    width: Option<u32>,
) -> Result<Option<String>, ScreenCaptureError> {
    #[cfg(target_os = "macos")]
    {
        // ScreenCaptureKit objects are not Sync, and capture_image borrows the
        // filter across its own await — so the whole capture runs on a thread
        // of its own and only the encoded PNG, a plain String, crosses back.
        // Tauri commands must return Send futures; this is what makes that
        // possible without lying about the objects' thread-safety.
        let width = width.unwrap_or(320) as usize;
        let (tx, rx) = tokio::sync::oneshot::channel();

        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => {
                    let _ = tx.send(None);
                    return;
                }
            };
            let result = runtime.block_on(mac::thumbnail(&target, width));
            let _ = tx.send(result);
        });

        Ok(rx.await.unwrap_or(None))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (target, width);
        Ok(None)
    }
}

#[tauri::command]
pub async fn api_start_screen_recording<R: Runtime>(
    app: AppHandle<R>,
    target: CaptureTarget,
    destination: String,
    started_at_offset_ms: i64,
) -> Result<ScreenRecordingState, ScreenCaptureError> {
    #[cfg(target_os = "macos")]
    {
        {
            let active = ACTIVE.lock().map_err(|_| ScreenCaptureError::Failed)?;
            if active.is_some() {
                return Err(ScreenCaptureError::AlreadyRecording);
            }
        }

        let path = PathBuf::from(destination);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| ScreenCaptureError::Failed)?;
        }

        let recording = mac::start(&target, &path, started_at_offset_ms).await?;
        let state = ScreenRecordingState {
            is_recording: true,
            target: Some(recording.target.clone()),
            output_path: Some(recording.path.to_string_lossy().to_string()),
            started_at_offset_ms: Some(recording.started_at_offset_ms),
        };
        *ACTIVE.lock().map_err(|_| ScreenCaptureError::Failed)? = Some(recording);
        let _ = app.emit("screen-recording-changed", &state);
        Ok(state)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, target, destination, started_at_offset_ms);
        Err(ScreenCaptureError::Unsupported)
    }
}

#[tauri::command]
pub async fn api_stop_screen_recording<R: Runtime>(
    app: AppHandle<R>,
) -> Result<ScreenRecordingState, ScreenCaptureError> {
    #[cfg(target_os = "macos")]
    {
        // Taken out and the lock released before awaiting: holding a
        // MutexGuard across an await makes the whole command non-Send, and a
        // second stop arriving meanwhile should see an empty slot, not block.
        let recording = {
            let mut active = ACTIVE.lock().map_err(|_| ScreenCaptureError::Failed)?;
            active.take().ok_or(ScreenCaptureError::NotRecording)?
        };

        // Described before stopping, because stopping consumes the recording.
        let state = ScreenRecordingState {
            is_recording: false,
            target: Some(recording.target.clone()),
            output_path: Some(recording.path.to_string_lossy().to_string()),
            started_at_offset_ms: Some(recording.started_at_offset_ms),
        };

        let result = mac::stop(recording).await;
        let _ = app.emit("screen-recording-changed", &state);
        result.map(|()| state)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err(ScreenCaptureError::Unsupported)
    }
}

#[tauri::command]
pub async fn api_get_screen_recording_state<R: Runtime>(
    _app: AppHandle<R>,
) -> Result<ScreenRecordingState, ScreenCaptureError> {
    #[cfg(target_os = "macos")]
    {
        let active = ACTIVE.lock().map_err(|_| ScreenCaptureError::Failed)?;
        Ok(match active.as_ref() {
            Some(recording) => ScreenRecordingState {
                is_recording: true,
                target: Some(recording.target.clone()),
                output_path: Some(recording.path.to_string_lossy().to_string()),
                started_at_offset_ms: Some(recording.started_at_offset_ms),
            },
            None => ScreenRecordingState {
                is_recording: false,
                target: None,
                output_path: None,
                started_at_offset_ms: None,
            },
        })
    }
    #[cfg(not(target_os = "macos"))]
    Ok(ScreenRecordingState {
        is_recording: false,
        target: None,
        output_path: None,
        started_at_offset_ms: None,
    })
}
