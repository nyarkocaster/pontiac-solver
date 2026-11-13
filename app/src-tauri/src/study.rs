// Tauri command wrappers for the guided Study mode (AGPL-3.0-or-later).
use crate::study_core::*;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

#[tauri::command]
pub fn study_progress(prog: tauri::State<Arc<StudyProgress>>) -> (u8, u32, u32, f64) {
    (
        prog.phase.load(Ordering::Relaxed),
        prog.iter.load(Ordering::Relaxed),
        prog.max_iter.load(Ordering::Relaxed),
        prog.expl_milli_pct.load(Ordering::Relaxed) as f64 / 1000.0,
    )
}

#[tauri::command]
pub fn study_cancel(prog: tauri::State<Arc<StudyProgress>>) {
    prog.cancel.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn study_memory_estimate(spec: StudySpec) -> Result<(f64, f64), String> {
    memory_estimate(&spec)
}

#[tauri::command(async)]
pub fn study_solve(
    state: tauri::State<Mutex<StudyState>>,
    prog: tauri::State<Arc<StudyProgress>>,
    spec: StudySpec,
) -> StudySummary {
    let prog: Arc<StudyProgress> = prog.inner().clone();
    // free the previous study before allocating a new one
    state.lock().unwrap().reset();
    let result = solve_inner(&spec, &prog);
    let mut st = state.lock().unwrap();
    match result {
        Ok(out) => {
            let summary = out.summary.clone();
            st.install(out);
            prog.phase.store(4, Ordering::Relaxed);
            summary
        }
        Err(e) => {
            prog.phase.store(5, Ordering::Relaxed);
            StudySummary { error: Some(e), ..Default::default() }
        }
    }
}

#[tauri::command]
pub fn study_summary(state: tauri::State<Mutex<StudyState>>) -> Option<StudySummary> {
    state.lock().unwrap().summary.clone()
}

#[tauri::command]
pub fn study_node(
    state: tauri::State<Mutex<StudyState>>,
    which: String,
    history: Vec<usize>,
) -> Result<NodeView, String> {
    state.lock().unwrap().view(which == "locked", &history)
}

#[tauri::command]
pub fn study_hand(
    state: tauri::State<Mutex<StudyState>>,
    which: String,
    history: Vec<usize>,
    player: usize,
    hand: String,
) -> Result<HandView, String> {
    state.lock().unwrap().hand(which == "locked", &history, player, &hand)
}

#[tauri::command]
pub fn study_range_grid(range: String) -> Result<Vec<f32>, String> {
    range_grid(&range)
}
