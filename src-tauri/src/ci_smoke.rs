//! A CI-only application entry point: real assets, WebView and IPC, no core startup.
//! Production builds do not compile this module or accept its command-line switch.
use std::path::PathBuf;
use tauri::{Manager, State, WebviewWindow};

struct SmokeState {
    token: String,
    output: PathBuf,
}

#[tauri::command]
fn ci_smoke_ready(
    window: WebviewWindow,
    state: State<'_, SmokeState>,
    token: String,
    error: Option<String>,
) -> Result<(), String> {
    if window.label() != "main" || token != state.token {
        return Err("unexpected smoke-test caller".into());
    }
    let (report, exit_code) = match error {
        Some(error) => (serde_json::json!({"token": token, "error": error}), 1),
        None => (
            serde_json::json!({"token": token, "webview": true, "assets": true, "ipc": true}),
            0,
        ),
    };
    std::fs::write(&state.output, report.to_string()).map_err(|error| error.to_string())?;
    window.app_handle().exit(exit_code);
    Ok(())
}

pub fn run() {
    let context = crate::app_context();
    assert_eq!(context.config().identifier, "com.zeytun.app.ci");
    let token = std::env::var("ZEYTUN_CI_SMOKE_TOKEN").expect("smoke token is required");
    let output = std::env::var_os("ZEYTUN_CI_SMOKE_OUTPUT").expect("smoke output is required");
    let token_json = serde_json::to_string(&token).expect("token must serialize");
    // Import the embedded modulepreload graph before accepting the IPC handshake.
    // This catches absent/changing chunk paths as well as WebView startup failures.
    let script = format!(
        r#"(async () => {{
            if (!document.body || !document.styleSheets.length) throw new Error('missing frontend');
            await Promise.all(Array.from(document.querySelectorAll('link[rel="modulepreload"]'), link => import(link.href)));
            await window.__TAURI_INTERNALS__.invoke('ci_smoke_ready', {{ token: {token_json} }});
        }})().catch(error => window.__TAURI_INTERNALS__.invoke('ci_smoke_ready', {{
            token: {token_json}, error: String(error)
        }}));"#
    );
    tauri::Builder::default()
        .manage(SmokeState {
            token,
            output: output.into(),
        })
        .invoke_handler(tauri::generate_handler![ci_smoke_ready])
        .on_page_load(move |webview, payload| {
            if webview.label() == "main"
                && payload.event() == tauri::webview::PageLoadEvent::Finished
            {
                if let Err(error) = webview.eval(&script) {
                    eprintln!("could not run smoke handshake: {error}");
                    webview.app_handle().exit(1);
                }
            }
        })
        .run(context)
        .expect("could not run isolated CI application");
}
