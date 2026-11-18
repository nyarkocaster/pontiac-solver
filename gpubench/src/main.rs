//! Experiment: are three-way showdown values faster on a graphics card than on the processor?
//! Builds one river spot, makes a batch of random reaches, and works out every hero hand's
//! showdown share both ways. Prints timings and the largest difference between the two.
use multiway::game::W;
use multiway::study::*;
use rayon::prelude::*;
use std::time::Instant;
use wgpu::util::DeviceExt;

fn say(log: &mut String, line: String) { println!("{line}"); log.push_str(&line); log.push_str("\r\n"); let _ = std::fs::write("gpubench_result.txt", log.as_bytes()); }

fn main() {
    let mut log = String::new();
    let batch: usize = std::env::args().nth(1).and_then(|x| x.parse().ok()).unwrap_or(2048);
    let rounds: usize = std::env::args().nth(2).and_then(|x| x.parse().ok()).unwrap_or(5);
    let w = ["99-22,A9s-A2s,K9s-K2s,Q9s+,J9s+,T8s+,97s+,86s+,75s+,65s,54s,ATo-A8o,KTo+,QTo+,JTo", "TT-22,AJs-A2s,KTs+,QTs+,JTs,T9s,98s,87s,76s,AQo-ATo,KQo,KJo", "AA-22,AKs-A2s,K9s+,Q9s+,J9s+,T8s+,98s,87s,76s,65s,AKo-ATo,KJo+,QJo"];
    let spec = Spec3 { ranges: [w[0].into(), w[1].into(), w[2].into()], board: "Kh9s6d4c2s".into(), pot: 30.0, stack: 100.0, bet_sizes: "66%".into(), raise_sizes: "60%".into(), max_raises: 1, allin_spr: 1.0, threads: 0, turn_cards: 1, river_cards: 1, ..Default::default() };
    let g = build_game(&spec, false).unwrap();
    let sd = &g.streets[0];
    let hero = 2usize; let (o1, o2) = (0usize, 1usize);
    let nh = 1326usize;
    let n_hero = sd.sorted[hero].len();
    say(&mut log, format!("spot: river Kh9s6d4c2s, hero hands {}, opponents' hands {} and {}, batch {} showdowns, {} rounds", n_hero, sd.sorted[o1].len(), sd.sorted[o2].len(), batch, rounds));

    // random reaches: each opponent hand keeps its range weight times a random factor, a third are zero
    let mut seed = 0x2545F4914F6CDD1Du64;
    let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed >> 40) as f32 / (1u64 << 24) as f32 };
    let mut reach = vec![0f32; batch * 2 * nh];
    for b in 0..batch { for (k, p) in [o1, o2].iter().enumerate() { for &h in &g.hands[*p] { let x = rnd(); reach[(b * 2 + k) * nh + h as usize] = if x < 0.33 { 0.0 } else { g.cfg.ranges[*p][h as usize] * x }; } } }

    // processor: every core, the solver's own showdown routine (hero is player 2, so r1 = player 0, r2 = player 1)
    let cpu_once = || -> Vec<f32> {
        let parts: Vec<Vec<f32>> = (0..batch).into_par_iter().map(|b| {
            let r1 = &reach[(b * 2) * nh..(b * 2 + 1) * nh]; let r2 = &reach[(b * 2 + 1) * nh..(b * 2 + 2) * nh];
            let share = g.showdown(0, hero, o1, r1, r2, true, 1.0, 0.0);
            let all = g.showdown(0, hero, o1, r1, r2, true, 0.0, -1.0);
            let mut v = Vec::with_capacity(share.len() * 2);
            for i in 0..share.len() { v.push(share[i]); v.push(all[i]); }
            v
        }).collect();
        parts.concat()
    };
    let cpu_ref = cpu_once();
    let t = Instant::now();
    for _ in 0..rounds { std::hint::black_box(cpu_once()); }
    // the reference does two showdown calls per entry; a solve does one
    let cpu_secs = t.elapsed().as_secs_f64() / rounds as f64 / 2.0;
    say(&mut log, format!("processor ({} threads): {:.1} ms per batch = {:.2} microseconds per showdown", rayon::current_num_threads(), cpu_secs * 1e3, cpu_secs * 1e6 / batch as f64));

    // static tables for this final board
    let cards = &g.deck.cards;
    let rank = |a: usize, b: usize| sd.rank[a * W + b] as u32;
    let mut pk = vec![0u32; 52 * 48]; let mut cnt = vec![0u32; 52]; let mut tab = vec![0u32; 52 * 52];
    for c in 0..52 {
        let mut ps: Vec<(u32, u32)> = (0..52).filter(|&d| d != c && rank(c, d) > 0).map(|d| (rank(c, d), g.deck.idx[c][d] as u32)).collect();
        ps.sort();
        cnt[c] = ps.len() as u32;
        for (k, (r, h)) in ps.iter().enumerate() { pk[c * 48 + k] = h | (r << 16); }
        for d in 0..52 { if d != c && rank(c, d) > 0 { tab[c * 52 + d] = g.deck.idx[c][d] as u32 | (rank(c, d) << 16); } }
    }
    // hero hands in the order of the solver's dense output (position in hands[hero])
    let mut hero_tab = vec![0u32; n_hero * 2]; let mut lohi = vec![0u32; n_hero * 52];
    let mut pos_of = vec![0usize; n_hero];
    for (i, &h) in sd.sorted[hero].iter().enumerate() {
        let (a, b) = cards[h as usize]; let (a, b) = (a as usize, b as usize); let s = rank(a, b);
        hero_tab[i * 2] = a as u32 | ((b as u32) << 8) | (s << 16); hero_tab[i * 2 + 1] = h as u32;
        pos_of[i] = g.hpos[hero][h as usize] as usize;
        for c in 0..52 {
            let n = cnt[c] as usize;
            let lo = (0..n).filter(|&k| (pk[c * 48 + k] >> 16) < s).count() as u32;
            let hi = (0..n).filter(|&k| (pk[c * 48 + k] >> 16) <= s).count() as u32;
            lohi[i * 52 + c] = lo | (hi << 8);
        }
    }

    // graphics card
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::VULKAN | wgpu::Backends::DX12, ..Default::default() });
    for a in instance.enumerate_adapters(wgpu::Backends::all()) { let i = a.get_info(); say(&mut log, format!("found: {} ({:?}, {:?})", i.name, i.device_type, i.backend)); }
    let want = std::env::args().nth(3).unwrap_or_default().to_lowercase();
    let mut adapters = instance.enumerate_adapters(wgpu::Backends::all());
    adapters.sort_by_key(|a| { let i = a.get_info(); let named = !want.is_empty() && i.name.to_lowercase().contains(&want);
        (if named { 0 } else { 1 }, match i.device_type { wgpu::DeviceType::DiscreteGpu => 0, wgpu::DeviceType::IntegratedGpu => 1, _ => 2 }, if i.backend == wgpu::Backend::Vulkan { 0 } else { 1 }) });
    let Some(adapter) = adapters.into_iter().next() else { say(&mut log, "no graphics adapter could be opened".into()); return; };
    let info = adapter.get_info();
    say(&mut log, format!("using: {} ({:?}, {:?}, driver {})", info.name, info.device_type, info.backend, info.driver_info));
    let mut limits = wgpu::Limits::default();
    let al = adapter.limits();
    limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size; limits.max_buffer_size = al.max_buffer_size;
    limits.max_storage_buffers_per_shader_stage = 8;
    let (device, queue) = match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: None, required_features: wgpu::Features::empty(), required_limits: limits }, None)) {
        Ok(x) => x, Err(e) => { say(&mut log, format!("the graphics card refused the request: {e}")); return; } };
    let cum_bytes = (batch * 3 * 52 * 49 * 4) as u64; let out_bytes = (batch * n_hero * 2 * 4) as u64;
    if cum_bytes > al.max_storage_buffer_binding_size as u64 { say(&mut log, format!("batch too large for this card ({} MB needed in one block); try a smaller batch", cum_bytes >> 20)); return; }
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()) });
    let ro = |data: &[u32]| device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(data), usage: wgpu::BufferUsages::STORAGE });
    let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&[batch as u32, n_hero as u32, 0, 0]), usage: wgpu::BufferUsages::UNIFORM });
    let (b_pk, b_cnt, b_tab, b_hero, b_lohi) = (ro(&pk), ro(&cnt), ro(&tab), ro(&hero_tab), ro(&lohi));
    let b_reach = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (reach.len() * 4) as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let b_cum = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: cum_bytes, usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false });
    let b_out = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: out_bytes, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
    let b_read = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: out_bytes, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None }, count: None };
    let rd = wgpu::BufferBindingType::Storage { read_only: true }; let rw = wgpu::BufferBindingType::Storage { read_only: false };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[entry(0, wgpu::BufferBindingType::Uniform), entry(1, rd), entry(2, rd), entry(3, rd), entry(4, rd), entry(5, rd), entry(6, rd), entry(7, rw), entry(8, rw)] });
    let bufs = [&params, &b_pk, &b_cnt, &b_tab, &b_hero, &b_lohi, &b_reach, &b_cum, &b_out];
    let entries: Vec<wgpu::BindGroupEntry> = bufs.iter().enumerate().map(|(i, b)| wgpu::BindGroupEntry { binding: i as u32, resource: b.as_entire_binding() }).collect();
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &layout, entries: &entries });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&layout], push_constant_ranges: &[] });
    let pipe = |entry_point: &str| device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: None, layout: Some(&pl), module: &module, entry_point });
    let (p_prefix, p_values) = (pipe("prefix"), pipe("values"));

    let mut gpu_once = |timing: bool| -> (Vec<f32>, f64, f64) {
        let t = Instant::now();
        queue.write_buffer(&b_reach, 0, bytemuck::cast_slice(&reach));
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_bind_group(0, &bind, &[]);
            pass.set_pipeline(&p_prefix); pass.dispatch_workgroups(((batch * 156 + 63) / 64) as u32, 1, 1);
            pass.set_pipeline(&p_values); pass.dispatch_workgroups(((batch * n_hero + 63) / 64) as u32, 1, 1);
        }
        enc.copy_buffer_to_buffer(&b_out, 0, &b_read, 0, out_bytes);
        queue.submit(Some(enc.finish()));
        let slice = b_read.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::Maintain::Wait);
        let total = t.elapsed().as_secs_f64();
        let t2 = Instant::now();
        let v: Vec<f32> = bytemuck::cast_slice(&slice.get_mapped_range()).to_vec();
        b_read.unmap();
        let _ = timing;
        (v, total, t2.elapsed().as_secs_f64())
    };
    let (first, t_first, _) = gpu_once(false);
    say(&mut log, format!("graphics card, first batch (includes preparing the program): {:.1} ms", t_first * 1e3));
    let mut best = f64::MAX; let mut sum = 0.0;
    for _ in 0..rounds { let (_, t, c) = gpu_once(true); best = best.min(t + c); sum += t + c; }
    let gpu_secs = sum / rounds as f64;
    say(&mut log, format!("graphics card: {:.1} ms per batch on average, {:.1} ms best = {:.2} microseconds per showdown (sending the reaches and reading the answers back included)", gpu_secs * 1e3, best * 1e3, gpu_secs * 1e6 / batch as f64));

    // agreement
    let mut scale = 0f64; for x in &cpu_ref { scale = scale.max(x.abs() as f64); }
    let (mut worst, mut tot, mut n) = (0f64, 0f64, 0usize);
    for b in 0..batch { for i in 0..n_hero {
        for k in 0..2 { let c = cpu_ref[(b * g.hands[hero].len() + pos_of[i]) * 2 + k] as f64; let v = first[(b * n_hero + i) * 2 + k] as f64; let d = (c - v).abs() / scale; worst = worst.max(d); tot += d; n += 1; }
    } }
    say(&mut log, format!("agreement: largest difference {:.2e} of the largest value, average {:.2e} ({} values compared)", worst, tot / n as f64, n));
    say(&mut log, format!("RESULT: graphics card is {:.2}x the processor's speed on this job", cpu_secs / gpu_secs));
}
