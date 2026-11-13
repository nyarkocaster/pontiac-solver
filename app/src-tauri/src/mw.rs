// Tauri command wrappers for the three-way river solver (AGPL-3.0-or-later).
use multiway::study::*;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};

#[tauri::command]
pub fn mw_progress(prog: tauri::State<Arc<Progress3>>) -> (u8, u32, u32, f64) {
    (
        prog.phase.load(Ordering::Relaxed),
        prog.iter.load(Ordering::Relaxed),
        prog.max_iter.load(Ordering::Relaxed),
        prog.gain_milli_pct.load(Ordering::Relaxed) as f64 / 1000.0,
    )
}

#[tauri::command]
pub fn mw_cancel(prog: tauri::State<Arc<Progress3>>) {
    prog.cancel.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn mw_estimate(spec: Spec3) -> Result<Estimate3, String> {
    estimate(&spec)
}

#[tauri::command(async)]
pub fn mw_solve(state: tauri::State<RwLock<State3>>, prog: tauri::State<Arc<Progress3>>, spec: Spec3) -> Summary3 {
    let prog: Arc<Progress3> = prog.inner().clone();
    *state.write().unwrap() = State3::default();
    match solve3(&spec, &prog) {
        Ok(out) => {
            let summary = out.summary.clone().unwrap_or_default();
            *state.write().unwrap() = out;
            prog.phase.store(4, Ordering::Relaxed);
            summary
        }
        Err(e) => {
            prog.phase.store(5, Ordering::Relaxed);
            Summary3 { error: Some(e), ..Default::default() }
        }
    }
}

#[tauri::command(async)]
pub fn mw_node(state: tauri::State<RwLock<State3>>, which: String, history: Vec<usize>, ev: bool) -> Result<NodeView3, String> {
    state.read().unwrap().view(which == "locked", &history, ev)
}
