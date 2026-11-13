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

/// After a flop solve: solves one turn card exactly (every river card) from the ranges that reach it.
#[tauri::command(async)]
pub fn mw_resolve(state: tauri::State<RwLock<State3>>, prog: tauri::State<Arc<Progress3>>, which: String, history: Vec<usize>, card: u8) -> Summary3 {
    let prog: Arc<Progress3> = prog.inner().clone();
    // free the previous exact solve before starting the next one
    state.write().unwrap().sub = None;
    let out = { let st = state.read().unwrap(); resolve_turn(&st, which == "locked", &history, card, &prog) };
    match out {
        Ok(sub) => {
            let summary = sub.summary.clone();
            state.write().unwrap().sub = Some(sub);
            prog.phase.store(4, Ordering::Relaxed);
            summary
        }
        Err(e) => {
            prog.phase.store(5, Ordering::Relaxed);
            Summary3 { error: Some(e), ..Default::default() }
        }
    }
}

/// Heads-up push/fold equilibrium for one stack depth.
#[tauri::command(async)]
pub fn pushfold_solve(stack: f64, sb: f64, ante: f64) -> Result<pushfold::PushFold, String> {
    pushfold::solve(stack, sb, ante)
}
