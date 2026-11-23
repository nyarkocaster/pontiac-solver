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

/// Sets a played hand against the solve on screen: each of the player's decisions, priced for the exact hand held.
#[tauri::command(async)]
pub fn study_review(state: tauri::State<Mutex<StudyState>>, which: String, hero: usize, hand: String, steps: Vec<ReviewStep>) -> Result<Review, String> {
    state.lock().unwrap().review(which == "locked", hero, &hand, &steps)
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

/// Solve again with hand-made locks, keeping the normal solve already done.
#[tauri::command(async)]
pub fn study_relock(state: tauri::State<Mutex<StudyState>>, prog: tauri::State<Arc<StudyProgress>>, spec: StudySpec, locks: Vec<ManualLock>) -> Result<StudySummary, String> {
    let prog: Arc<StudyProgress> = prog.inner().clone();
    let (base, summary) = state.lock().unwrap().begin_relock()?;
    let (base, out) = relock(base, &spec, &locks, &prog, summary);
    prog.phase.store(if out.is_ok() { 4 } else { 5 }, Ordering::Relaxed);
    Ok(state.lock().unwrap().finish_relock(base, out))
}

#[tauri::command]
pub fn library_entries() -> Vec<LibraryEntry> { library_list() }

#[tauri::command]
pub fn library_remove(id: String) -> Result<(), String> { library_delete(&id) }

#[tauri::command(async)]
pub fn library_save_two(state: tauri::State<Mutex<StudyState>>, name: String, meta: serde_json::Value) -> Result<LibraryEntry, String> {
    state.lock().unwrap().library_save(&name, meta)
}

#[tauri::command(async)]
pub fn library_save_three(state: tauri::State<std::sync::RwLock<multiway::study::State3>>, name: String, meta: serde_json::Value) -> Result<LibraryEntry, String> {
    crate::study_core::library_save_three(Some(&state.read().unwrap()), &name, meta)
}

/// Puts a stored three-player solve on the three-player screen. Returns its summary.
#[tauri::command(async)]
pub fn library_open_three(state: tauri::State<std::sync::RwLock<multiway::study::State3>>, id: String) -> Result<multiway::study::Summary3, String> {
    *state.write().unwrap() = Default::default(); // free the solve on screen before reading the next
    let st = crate::study_core::library_open_three(&id)?;
    let summary = st.summary.clone().unwrap_or_default();
    *state.write().unwrap() = st;
    Ok(summary)
}

#[tauri::command(async)]
pub fn library_open_two(state: tauri::State<Mutex<StudyState>>, id: String) -> Result<LibraryEntry, String> {
    state.lock().unwrap().library_open(&id)
}
