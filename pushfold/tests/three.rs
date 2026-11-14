use pushfold::three::*;
fn spot(stacks: [f64; 3], payouts: Vec<f64>) -> Spot3 { Spot3 { stacks, small_blind: 0.5, ante: 0.0, bb_ante: 0.0, payouts, others: vec![], iterations: 300 } }
#[test]
fn chips_and_prizes() {
    let t = std::time::Instant::now();
    let c = pushfold::three::solve(&spot([10.0, 10.0, 10.0], vec![])).unwrap();
    println!("chips 10/10/10: {:?} value {:?} gain {:?} in {:?}", c.decisions.iter().map(|d| (d.id.clone(), d.pct)).collect::<Vec<_>>(), c.value, c.max_gain, t.elapsed());
    assert!((c.value.iter().sum::<f64>()).abs() < 1e-3, "chips are zero-sum");
    assert!(c.max_gain.iter().all(|&g| g < 0.02), "close to equilibrium: {:?}", c.max_gain);
    let p = pushfold::three::solve(&spot([10.0, 10.0, 10.0], vec![50.0, 30.0, 20.0])).unwrap();
    println!("prizes 10/10/10: {:?} value {:?} gain {:?}", p.decisions.iter().map(|d| (d.id.clone(), d.pct)).collect::<Vec<_>>(), p.value, p.max_gain);
    assert!((p.value.iter().sum::<f64>() - 100.0).abs() < 1e-2);
    // playing for prizes makes calling tighter than playing for chips
    assert!(p.decisions[3].pct < c.decisions[3].pct && p.decisions[1].pct < c.decisions[1].pct);
    let b = pushfold::three::solve(&Spot3 { others: vec![2.0], ..spot([12.0, 8.0, 15.0], vec![50.0, 30.0, 20.0]) }).unwrap();
    println!("bubble with a 2bb stack waiting: {:?} value {:?}", b.decisions.iter().map(|d| (d.id.clone(), d.pct)).collect::<Vec<_>>(), b.value);
}
