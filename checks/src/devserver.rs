#[path = "../../app/src-tauri/src/study_core.rs"]
mod study_core;
use study_core::*;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use serde_json::{json, Value};
use multiway::study as mw;

fn main() {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:8765").unwrap());
    let state = Arc::new(Mutex::new(StudyState::default()));
    let prog = Arc::new(StudyProgress::default());
    let mstate = Arc::new(std::sync::RwLock::new(mw::State3::default()));
    let mprog = Arc::new(mw::Progress3::default());
    println!("dev server on 8765");
    for req in server.incoming_requests() {
        let state = state.clone(); let prog = prog.clone(); let (ms, mp) = (mstate.clone(), mprog.clone());
        std::thread::spawn(move || handle(req, state, prog, ms, mp));
    }
}

fn handle(mut req: tiny_http::Request, state: Arc<Mutex<StudyState>>, prog: Arc<StudyProgress>, mstate: Arc<std::sync::RwLock<mw::State3>>, mprog: Arc<mw::Progress3>) {
    let cors = tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap();
    if req.method() == &tiny_http::Method::Options {
        let h2 = tiny_http::Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"*"[..]).unwrap();
        let _ = req.respond(tiny_http::Response::empty(200).with_header(cors).with_header(h2));
        return;
    }
    let cmd = req.url().trim_start_matches('/').to_string();
    let mut body = String::new();
    let _ = req.as_reader().read_to_string(&mut body);
    let args: Value = serde_json::from_str(&body).unwrap_or(json!({}));
    let result: Result<Value, String> = (|| match cmd.as_str() {
        "mw_progress" => Ok(json!([mprog.phase.load(Ordering::Relaxed), mprog.iter.load(Ordering::Relaxed), mprog.max_iter.load(Ordering::Relaxed), mprog.gain_milli_pct.load(Ordering::Relaxed) as f64 / 1000.0])),
        "mw_cancel" => { mprog.cancel.store(true, Ordering::Relaxed); Ok(Value::Null) }
        "mw_estimate" => {
            let spec: mw::Spec3 = serde_json::from_value(args["spec"].clone()).map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(mw::estimate(&spec)?).unwrap())
        }
        "mw_solve" => {
            let spec: mw::Spec3 = serde_json::from_value(args["spec"].clone()).map_err(|e| e.to_string())?;
            *mstate.write().unwrap() = mw::State3::default();
            match mw::solve3(&spec, &mprog) {
                Ok(out) => { let s = out.summary.clone().unwrap_or_default(); *mstate.write().unwrap() = out; mprog.phase.store(4, Ordering::Relaxed); Ok(serde_json::to_value(s).unwrap()) }
                Err(e) => { mprog.phase.store(5, Ordering::Relaxed); Ok(serde_json::to_value(mw::Summary3 { error: Some(e), ..Default::default() }).unwrap()) }
            }
        }
        "mw_node" => {
            let which = args["which"].as_str().unwrap_or("base").to_string();
            let hist: Vec<usize> = serde_json::from_value(args["history"].clone()).unwrap_or_default();
            let ev = args["ev"].as_bool().unwrap_or(true);
            Ok(serde_json::to_value(mstate.read().unwrap().view(which == "locked", &hist, ev)?).unwrap())
        }
        "cpu_threads" => Ok(json!(2)),
        "memory" => Ok(json!([6u64<<30, 8u64<<30])),
        "study_progress" => Ok(json!([prog.phase.load(Ordering::Relaxed), prog.iter.load(Ordering::Relaxed), prog.max_iter.load(Ordering::Relaxed), prog.expl_milli_pct.load(Ordering::Relaxed) as f64 / 1000.0])),
        "study_cancel" => { prog.cancel.store(true, Ordering::Relaxed); Ok(Value::Null) }
        "study_memory_estimate" => {
            let spec: StudySpec = serde_json::from_value(args["spec"].clone()).map_err(|e| e.to_string())?;
            Ok(json!(memory_estimate(&spec)?))
        }
        "study_memory_detail" => {
            let spec: StudySpec = serde_json::from_value(args["spec"].clone()).map_err(|e| e.to_string())?;
            let g = build_game(&spec)?; let (u, c) = g.memory_usage();
            Ok(json!([u as f64/1048576.0, c as f64/1048576.0]))
        }
        "study_solve" => {
            let spec: StudySpec = serde_json::from_value(args["spec"].clone()).map_err(|e| e.to_string())?;
            state.lock().unwrap().reset();
            match solve_inner(&spec, &prog) {
                Ok(out) => { let s = out.summary.clone(); state.lock().unwrap().install(out); prog.phase.store(4, Ordering::Relaxed); Ok(serde_json::to_value(s).unwrap()) }
                Err(e) => { prog.phase.store(5, Ordering::Relaxed); Ok(json!({"error": e, "base_ev":[0,0],"base_expl_pct":0,"base_secs":0,"locked":false,"locked_ev":[0,0],"locked_expl_pct":0,"locked_secs":0,"locked_nodes":0,"memory_mb":0})) }
            }
        }
        "study_hand" => {
            let which = args["which"].as_str().unwrap_or("base").to_string();
            let hist: Vec<usize> = serde_json::from_value(args["history"].clone()).unwrap_or_default();
            let player = args["player"].as_u64().unwrap_or(0) as usize;
            let hand = args["hand"].as_str().unwrap_or("").to_string();
            let v = state.lock().unwrap().hand(which == "locked", &hist, player, &hand)?;
            Ok(serde_json::to_value(v).unwrap())
        }
        "study_range_grid" => Ok(json!(range_grid(args["range"].as_str().unwrap_or(""))?)),
        "study_node" => {
            let which = args["which"].as_str().unwrap_or("base").to_string();
            let hist: Vec<usize> = serde_json::from_value(args["history"].clone()).unwrap_or_default();
            let v = state.lock().unwrap().view(which == "locked", &hist)?;
            Ok(serde_json::to_value(v).unwrap())
        }
        _ => Err(format!("unknown command {cmd}")),
    })();
    let (code, text) = match result { Ok(v) => (200, v.to_string()), Err(e) => (500, json!(e).to_string()) };
    let ct = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    let _ = req.respond(tiny_http::Response::from_string(text).with_status_code(code).with_header(cors).with_header(ct));
}
