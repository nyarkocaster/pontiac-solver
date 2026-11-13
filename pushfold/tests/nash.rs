use pushfold::*;
#[test]
fn known_ranges() {
    for s in [2.0, 5.0, 8.0, 10.0, 15.0, 20.0] {
        let r = solve(s, 0.5, 0.0).unwrap();
        println!("{:>5} bb  jam {:.1}%  call {:.1}%  sb ev {:+.4}", s, r.jam_pct * 100.0, r.call_pct * 100.0, r.ev_small_blind_bb);
        // nobody can gain by switching a pure choice
        for i in 0..169 {
            if r.jam[i] == 1.0 { assert!(r.jam_edge_bb[i] > -0.02, "{} jam edge {}", r.classes[i], r.jam_edge_bb[i]); }
            if r.jam[i] == 0.0 { assert!(r.jam_edge_bb[i] < 0.02, "{} fold edge {}", r.classes[i], r.jam_edge_bb[i]); }
            if r.call[i] == 1.0 { assert!(r.call_edge_bb[i] > -0.02); }
            if r.call[i] == 0.0 { assert!(r.call_edge_bb[i] < 0.02); }
        }
    }
    let r = solve(10.0, 0.5, 0.0).unwrap();
    assert!((r.jam_pct - 0.58).abs() < 0.03, "jam {}", r.jam_pct);
    assert!((r.call_pct - 0.37).abs() < 0.03, "call {}", r.call_pct);
    let at = |n: &str| r.classes.iter().position(|c| c == n).unwrap();
    assert_eq!(r.jam[at("AA")], 1.0); assert_eq!(r.jam[at("22")], 1.0); assert_eq!(r.jam[at("72o")], 0.0);
    assert_eq!(r.call[at("A2o")], 1.0); assert_eq!(r.call[at("32o")], 0.0);
}
