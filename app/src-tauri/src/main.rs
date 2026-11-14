#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod bunching;
mod range;
mod solver;
mod tree;
mod study;
mod mw;
mod study_core;
use crate::bunching::*;
use crate::range::*;
use crate::solver::*;
use crate::tree::*;
use crate::study::*;
use crate::mw::*;
use crate::study_core::{StudyProgress, StudyState};
use std::sync::Arc;

use postflop_solver::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::sync::Mutex;
use sysinfo::{System, SystemExt};

fn main() {
    tauri::Builder::default()
        .manage(Mutex::new(RangeManager::default()))
        .manage(Mutex::new(default_action_tree()))
        .manage(Mutex::new(None as Option<BunchingData>))
        .manage(Mutex::new(PostFlopGame::default()))
        .manage(Mutex::new(ThreadPoolBuilder::new().build().unwrap()))
        .manage(Mutex::new(StudyState::default()))
        .manage(Arc::new(StudyProgress::default()))
        .manage(std::sync::RwLock::new(multiway::study::State3::default()))
        .manage(Arc::new(multiway::study::Progress3::default()))
        .invoke_handler(tauri::generate_handler![
            os_name,
            memory,
            memory_status,
            release_solve,
            set_num_threads,
            range_num_combos,
            range_clear,
            range_invert,
            range_update,
            range_from_string,
            range_to_string,
            range_get_weights,
            range_raw_data,
            tree_new,
            tree_added_lines,
            tree_removed_lines,
            tree_invalid_terminals,
            tree_actions,
            tree_is_terminal_node,
            tree_is_chance_node,
            tree_back_to_root,
            tree_apply_history,
            tree_play,
            tree_total_bet_amount,
            tree_add_bet_action,
            tree_remove_current_node,
            tree_delete_added_line,
            tree_delete_removed_line,
            bunching_init,
            bunching_clear,
            bunching_progress,
            game_init,
            game_private_cards,
            game_memory_usage,
            game_memory_usage_bunching,
            game_allocate_memory,
            game_set_bunching,
            game_solve_step,
            game_exploitability,
            game_exploitability_unrestricted,
            game_finalize,
            game_apply_history,
            game_total_bet_amount,
            game_lock_strategy,
            game_unlock_strategy,
            game_current_locking_strategy,
            game_clear_locks,
            game_lock_strategy_with_weight,
            game_locking_weight,
            game_actions_after,
            game_possible_cards,
            game_get_results,
            game_get_chance_reports,
            study_progress,
            study_cancel,
            study_memory_estimate,
            study_solve,
            study_summary,
            study_node,
            study_hand,
            study_range_grid,
            mw_progress,
            mw_cancel,
            mw_estimate,
            mw_solve,
            mw_node,
            mw_resolve,
            pushfold_solve,
            pushfold3_solve,
            icm_values,
            cpu_threads
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn os_name() -> String {
    "windows".to_string()
}

#[cfg(target_os = "macos")]
#[tauri::command]
fn os_name() -> String {
    "macos".to_string()
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn os_name() -> String {
    "linux".to_string()
}

#[tauri::command]
fn memory() -> (u64, u64) {
    let mut system = System::new();
    system.refresh_memory();
    (system.available_memory(), system.total_memory())
}

/// (free now, installed, held by the Guided Study solve, held by the 3-Way Pots solve), in bytes.
/// A tab's own held solve is released when it starts its next solve, so it can be counted as free.
#[tauri::command]
fn memory_status(study: tauri::State<Mutex<StudyState>>, mw: tauri::State<std::sync::RwLock<multiway::study::State3>>) -> (u64, u64, u64, u64) {
    let (avail, total) = memory();
    let guided = study.try_lock().map(|s| s.held_bytes()).unwrap_or(0);
    let three = mw.try_read().map(|s| s.held_bytes()).unwrap_or(0);
    (avail, total, guided, three)
}

/// Drops a tab's stored solve to give its memory back ("guided" or "threeway").
#[tauri::command]
fn release_solve(study: tauri::State<Mutex<StudyState>>, mw: tauri::State<std::sync::RwLock<multiway::study::State3>>, which: String) {
    if which == "guided" { study.lock().unwrap().reset(); } else { *mw.write().unwrap() = multiway::study::State3::default(); }
}

#[tauri::command]
fn set_num_threads(pool_state: tauri::State<Mutex<ThreadPool>>, num_threads: usize) {
    *pool_state.lock().unwrap() = ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .unwrap();
}

#[tauri::command]
fn cpu_threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}
