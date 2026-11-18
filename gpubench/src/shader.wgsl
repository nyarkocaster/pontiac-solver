struct Params { batch: u32, n_hero: u32, pad0: u32, pad1: u32 }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> pk: array<u32>;     // 52*48: hand id | rank << 16, per card, weakest partner first
@group(0) @binding(2) var<storage, read> cnt: array<u32>;    // 52: partners per card
@group(0) @binding(3) var<storage, read> tab: array<u32>;    // 52*52: hand id | rank << 16
@group(0) @binding(4) var<storage, read> hero: array<u32>;   // n_hero*2: a | b<<8 | s<<16, hand id
@group(0) @binding(5) var<storage, read> lohi: array<u32>;   // n_hero*52: lo | hi<<8
@group(0) @binding(6) var<storage, read> reach: array<f32>;  // batch*2*1326
@group(0) @binding(7) var<storage, read_write> cum: array<f32>; // batch*3*52*49
@group(0) @binding(8) var<storage, read_write> outv: array<f32>; // batch*n_hero*2

@compute @workgroup_size(64)
fn prefix(@builtin(global_invocation_id) gid: vec3<u32>) {
    let id = gid.x;
    if (id >= params.batch * 156u) { return; }
    let b = id / 156u; let rem = id % 156u; let o = rem / 52u; let c = rem % 52u;
    let base = ((b * 3u + o) * 52u + c) * 49u;
    let rb = b * 2652u;
    var acc = 0.0;
    cum[base] = 0.0;
    let n = cnt[c];
    for (var k = 0u; k < n; k = k + 1u) {
        let h = pk[c * 48u + k] & 0xFFFFu;
        var v = 0.0;
        if (o == 0u) { v = reach[rb + h]; } else if (o == 1u) { v = reach[rb + 1326u + h]; } else { v = reach[rb + h] * reach[rb + 1326u + h]; }
        acc = acc + v;
        cum[base + k + 1u] = acc;
    }
}

@compute @workgroup_size(64)
fn values(@builtin(global_invocation_id) gid: vec3<u32>) {
    let id = gid.x;
    if (id >= params.batch * params.n_hero) { return; }
    let b = id / params.n_hero; let i = id % params.n_hero;
    let hw = hero[i * 2u]; let hh = hero[i * 2u + 1u];
    let a = hw & 0xFFu; let bb = (hw >> 8u) & 0xFFu; let s = hw >> 16u;
    let rb = b * 2652u;
    var sa = vec3<f32>(0.0); var sl = vec3<f32>(0.0); var st = vec3<f32>(0.0);
    var oa = vec3<f32>(0.0); var ol = vec3<f32>(0.0); var ot = vec3<f32>(0.0);
    var p0 = 0.0; var p1 = 0.0; var p2 = 0.0; var p3 = 0.0; var p4 = 0.0;
    for (var c = 0u; c < 52u; c = c + 1u) {
        let n = cnt[c];
        if (n == 0u) { continue; }
        let lh = lohi[i * 52u + c]; let lo = lh & 0xFFu; let hi = (lh >> 8u) & 0xFFu;
        let b1 = ((b * 3u) * 52u + c) * 49u; let b2 = ((b * 3u + 1u) * 52u + c) * 49u; let b3 = ((b * 3u + 2u) * 52u + c) * 49u;
        let l = vec3<f32>(cum[b1 + lo], cum[b2 + lo], cum[b3 + lo]);
        let t = vec3<f32>(cum[b1 + hi], cum[b2 + hi], cum[b3 + hi]) - l;
        let al = vec3<f32>(cum[b1 + n], cum[b2 + n], cum[b3 + n]);
        sa = sa + al; sl = sl + l; st = st + t;
        if (c == a || c == bb) { oa = oa + al; ol = ol + l; ot = ot + t; continue; }
        let ta = tab[a * 52u + c]; let tb = tab[bb * 52u + c];
        let ha = ta & 0xFFFFu; let hb = tb & 0xFFFFu; let ka = ta >> 16u; let kb = tb >> 16u;
        let p1a = reach[rb + ha]; let p1b = reach[rb + hb]; let p2a = reach[rb + 1326u + ha]; let p2b = reach[rb + 1326u + hb];
        let la = f32(ka < s); let lb = f32(kb < s); let ea = f32(ka == s); let eb = f32(kb == s);
        let a1 = al.x - p1a - p1b; let a2 = al.y - p2a - p2b;
        let l1 = l.x - p1a * la - p1b * lb; let t1 = t.x - p1a * ea - p1b * eb;
        let l2 = l.y - p2a * la - p2b * lb; let t2 = t.y - p2a * ea - p2b * eb;
        p0 = p0 + a1 * a2; p1 = p1 + l1 * l2; p2 = p2 + t1 * l2; p3 = p3 + l1 * t2; p4 = p4 + t1 * t2;
    }
    let me0 = reach[rb + hh]; let me1 = reach[rb + 1326u + hh];
    let me = vec3<f32>(me0, me1, me0 * me1);
    let xa = sa * 0.5 - oa + me;
    let xl = sl * 0.5 - ol;
    let xt = st * 0.5 - ot + me;
    let g_aa = xa.x * xa.y - p0 + xa.z;
    let g_ll = xl.x * xl.y - p1 + xl.z;
    let g_tl = xt.x * xl.y - p2;
    let g_lt = xl.x * xt.y - p3;
    let g_tt = xt.x * xt.y - p4 + xt.z;
    outv[id * 2u] = g_ll + 0.5 * (g_tl + g_lt) + g_tt / 3.0;
    outv[id * 2u + 1u] = g_aa;
}
