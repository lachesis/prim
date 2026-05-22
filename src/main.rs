use std::fs::File;
use std::time::{SystemTime, UNIX_EPOCH};
use std::time::Duration;
use image::{RgbImage, Rgb, RgbaImage};
use image::codecs::gif::{GifEncoder, Repeat};
use image::Delay;
use rand::rngs::StdRng;
use rand::RngExt;
use rand::SeedableRng;
use clap::Parser;

const CELL: u32 = 40;
const MAX_DIM: u32 = 800;
const UNIX_J2000: u64 = 946728000; // 2000-01-01 12:00 UTC

#[derive(Parser)]
#[command(name = "prim")]
struct Args {
    #[arg(long, default_value_t = 42)]
    seed: u64,

    #[arg(long, default_value_t = 64)]
    width: u32,

    #[arg(long, default_value_t = 64)]
    height: u32,

    #[arg(long)]
    points: Option<u32>,

    #[arg(long, default_value_t = 15)]
    runtime: u32,

    #[arg(long, default_value_t = 2)]
    start_time: u32,

    #[arg(long, default_value_t = 3)]
    hold_time: u32,

    #[arg(long, default_value_t = 1)]
    repeats: u32,

    #[arg(long, default_value_t = 0)]
    step: u32,

    #[arg(long, default_value_t = String::from("output.gif"))]
    output: String,

    #[arg(long)]
    time: Option<f64>,  // UTC hour (0-24), default = current time
}

struct Pt {
    gx: u32,
    gy: u32,
}

fn place_points(w: u32, h: u32, count: u32, seed: u64) -> Vec<Pt> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut occ = vec![vec![false; h as usize]; w as usize];
    let mut pts = Vec::new();
    let mut placed = 0u32;
    while placed < count {
        let gx = rng.random_range(0..w);
        let gy = rng.random_range(0..h);
        if !occ[gx as usize][gy as usize] {
            occ[gx as usize][gy as usize] = true;
            pts.push(Pt { gx, gy });
            placed += 1;
        }
    }
    pts
}

fn prim_mst(points: &[Pt], start: usize) -> Vec<(usize, usize)> {
    let n = points.len();
    if n < 2 { return vec![]; }

    // closest[i] = (nearest MST-node to i, squared-distance)
    let mut closest = vec![(0, u64::MAX); n];
    let mut in_mst = vec![false; n];
    let mut edges = Vec::new();

    in_mst[start] = true;
    for j in 0..n {
        if j == start { continue; }
        let dx = points[start].gx as i64 - points[j].gx as i64;
        let dy = points[start].gy as i64 - points[j].gy as i64;
        closest[j] = (start, (dx * dx + dy * dy) as u64);
    }

    while edges.len() < n - 1 {
        // Find non-MST node with smallest closest distance
        let best = (0..n)
            .filter(|&j| !in_mst[j])
            .min_by_key(|&j| closest[j].1)
            .filter(|&j| closest[j].1 < u64::MAX);

        if let Some(j) = best {
            in_mst[j] = true;
            edges.push((closest[j].0, j));

            // Update distances for remaining nodes
            for k in 0..n {
                if in_mst[k] { continue; }
                let dx = points[j].gx as i64 - points[k].gx as i64;
                let dy = points[j].gy as i64 - points[k].gy as i64;
                let d = (dx * dx + dy * dy) as u64;
                if d < closest[k].1 {
                    closest[k] = (j, d);
                }
            }
        } else {
            break;
        }
    }
    edges
}

fn sky_points(w: u32, h: u32, count: u32, lat_deg: f64, lon_deg: f64, time_hour: Option<f64>) -> Vec<Pt> {
    let now_secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let today_start = now_secs - now_secs % 86400;
    let hour = match time_hour {
        Some(h) => h.max(0.0).min(24.0),
        None => (now_secs % 86400) as f64 / 3600.0,
    };
    let unix_ts = today_start + (hour * 3600.0) as u64;

    // Local Sidereal Time
    let d_days = (unix_ts - UNIX_J2000) as f64 / 86400.0;
    let gmst_deg = 280.46061837 + 360.98564736629 * d_days;
    let lst_deg = (gmst_deg + lon_deg).rem_euclid(360.0);
    let lat_rad = lat_deg.to_radians();

    let data = include_bytes!("stars.bin");
    let total = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;

    // Scan all stars brightness-first, collect visible ones
    let mut above: Vec<(f64, f64)> = Vec::new();
    for i in 0..total {
        if above.len() >= count as usize {
            break;
        }

        let off = 4 + i * 12;
        let ra = f32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as f64;
        let dec = f32::from_le_bytes(data[off + 4..off + 8].try_into().unwrap()) as f64;

        let ha_deg = (lst_deg - ra * 15.0).rem_euclid(360.0);
        let ha_rad = ha_deg.to_radians();
        let dec_rad = dec.to_radians();

        let sin_alt = dec_rad.sin() * lat_rad.sin()
            + dec_rad.cos() * lat_rad.cos() * ha_rad.cos();
        if sin_alt <= 0.0 {
            continue;
        }

        let alt_rad = sin_alt.asin();
        let az_rad = (-ha_rad.sin() * dec_rad.cos()).atan2(
            dec_rad.sin() * lat_rad.cos()
                - dec_rad.cos() * lat_rad.sin() * ha_rad.cos(),
        );

        let r = alt_rad.cos();
        let gx_f = w as f64 / 2.0 + (r * az_rad.sin()) * (w.min(h) as f64 * 0.45);
        let gy_f = h as f64 / 2.0 + (-r * az_rad.cos()) * (w.min(h) as f64 * 0.45);

        above.push((gx_f, gy_f));
    }

    // Place on grid with dedup
    let mut occ = vec![vec![false; h as usize]; w as usize];
    let mut pts = Vec::new();
    for &(gx_f, gy_f) in &above {
        let gx = gx_f.round() as i32;
        let gy = gy_f.round() as i32;
        if gx < 0 || gx >= w as i32 || gy < 0 || gy >= h as i32 {
            continue;
        }

        let (fx, fy) = find_free(&mut occ, w, h, gx as u32, gy as u32);
        occ[fx as usize][fy as usize] = true;
        pts.push(Pt { gx: fx, gy: fy });
    }
    pts
}

fn find_free(occ: &[Vec<bool>], w: u32, h: u32, gx: u32, gy: u32) -> (u32, u32) {
    if !occ[gx as usize][gy as usize] {
        return (gx, gy);
    }
    for r in 1..(w.max(h)) {
        for dx in -(r as i32)..=r as i32 {
            for dy in -(r as i32)..=r as i32 {
                if r > 1 && dx.abs() != r as i32 && dy.abs() != r as i32 {
                    continue;
                }
                let nx = gx as i32 + dx;
                let ny = gy as i32 + dy;
                if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32
                    && !occ[nx as usize][ny as usize]
                {
                    return (nx as u32, ny as u32);
                }
            }
        }
    }
    (gx, gy)
}

fn main() {
    let args = Args::parse();
    let w = args.width;
    let h = args.height;
    let pts = args.points.unwrap_or_else(|| (w * h / 10).max(1));

    if pts == 0 {
        eprintln!("no points to place");
        std::process::exit(1);
    }

    let points = if args.seed == 4242 {
        let lat = std::env::var("LAT").ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(39.0);
        let lon = std::env::var("LNG").ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(-84.0);
        let sp = sky_points(w, h, pts, lat, lon, args.time);
        eprintln!("✨ secret sky mode (lat={} lon={}): {} stars", lat, lon, sp.len());
        sp
    } else {
        place_points(w, h, pts, args.seed)
    };
    let edge_count = points.len() - 1;

    // Scale rendering
    let max_grid = w.max(h);
    let cell = if max_grid * CELL > MAX_DIM { (MAX_DIM / max_grid).max(3) } else { CELL };
    let r = (cell as i32 / 3).max(2);
    let margin = (cell / 2).max(4);
    let iw = w * cell + 2 * margin;
    let ih = h * cell + 2 * margin;
    let pcx = |gx: u32| margin + gx * cell + cell / 2;
    let pcy = |gy: u32| margin + gy * cell + cell / 2;

    // Frame stepping
    let frame_step = if args.step == 0 {
        let natural_fps = edge_count as f64 / args.runtime.max(1) as f64;
        if natural_fps <= 30.0 { 1 } else { (natural_fps / 30.0).ceil() as u32 }
    } else {
        args.step
    };
    let frames = (edge_count as u32).div_ceil(frame_step).max(1) as usize;

    // Delays
    let init_delay = Delay::from_saturating_duration(Duration::from_millis(args.start_time as u64 * 1000));
    let edge_delay = Delay::from_saturating_duration(Duration::from_millis(args.runtime as u64 * 1000 / frames as u64));
    let hold_delay = Delay::from_saturating_duration(Duration::from_millis(args.hold_time as u64 * 1000));

    let out = File::create(&args.output).expect("create output.gif failed");
    let mut encoder = GifEncoder::new(out);
    encoder.set_repeat(Repeat::Infinite).expect("set repeat failed");

    // Cache base frame (grid + points, no blue dot)
    let base = render_base(iw, ih, w, h, cell, margin, r, &points);

    for rep in 0..args.repeats {
        let start = rep as usize % points.len();
        let edges = prim_mst(&points, start);

        let total_len: f64 = edges.iter().map(|&(i, j)| {
            let dx = points[i].gx as f64 - points[j].gx as f64;
            let dy = points[i].gy as f64 - points[j].gy as f64;
            (dx * dx + dy * dy).sqrt()
        }).sum();
        println!("start ({},{})  total edge len {:.2}", points[start].gx, points[start].gy, total_len);

        let mut cur = base.clone();
        draw_dot(&mut cur, pcx(points[start].gx), pcy(points[start].gy), r, Rgb([0, 0, 255]));
        encode_frame(&mut encoder, &cur, init_delay);

        for (idx, &(i, j)) in edges.iter().enumerate() {
            draw_line(&mut cur, pcx(points[i].gx), pcy(points[i].gy), pcx(points[j].gx), pcy(points[j].gy), Rgb([255, 0, 0]));
            if idx as u32 % frame_step == 0 {
                encode_frame(&mut encoder, &cur, edge_delay);
            }
        }

        encode_frame(&mut encoder, &cur, hold_delay);
    }

    println!("{}  {}x{}  {} pts  {} edges  seed {}  repeats {}", args.output, w, h, pts, edge_count, args.seed, args.repeats);
}

fn encode_frame(encoder: &mut GifEncoder<File>, img: &RgbImage, delay: Delay) {
    let raw = img.clone().into_raw();
    let rgba: Vec<u8> = raw.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    let frame = RgbaImage::from_raw(img.width(), img.height(), rgba).unwrap();
    let gf = image::Frame::from_parts(frame, 0, 0, delay);
    encoder.encode_frame(gf).expect("encode frame failed");
}

fn render_base(iw: u32, ih: u32, w: u32, h: u32, cell: u32, margin: u32, r: i32, points: &[Pt]) -> RgbImage {
    let mut img = RgbImage::new(iw, ih);
    img.fill(255);

    let grid_col = Rgb([220, 220, 220]);
    for i in 0..=w {
        let gx = margin + i * cell;
        if gx >= iw { break; }
        for yp in margin..(margin + h * cell) {
            if yp >= ih { break; }
            img.put_pixel(gx, yp, grid_col);
        }
    }
    for j in 0..=h {
        let gy = margin + j * cell;
        if gy >= ih { break; }
        for xp in margin..(margin + w * cell) {
            if xp >= iw { break; }
            img.put_pixel(xp, gy, grid_col);
        }
    }

    let pt_col = Rgb([0, 0, 0]);
    for pt in points {
        let cx = margin + pt.gx * cell + cell / 2;
        let cy = margin + pt.gy * cell + cell / 2;
        draw_circle(&mut img, cx, cy, r, 2, pt_col);
    }

    img
}

fn draw_circle(img: &mut RgbImage, cx: u32, cy: u32, r: i32, s: i32, col: Rgb<u8>) {
    let cx = cx as i32;
    let cy = cy as i32;
    let lo = (r - s / 2).max(0);
    let hi = r + s / 2;
    let lo_sq = lo * lo;
    let hi_sq = hi * hi;
    for dy in -hi..=hi {
        for dx in -hi..=hi {
            let d_sq = dx * dx + dy * dy;
            if d_sq >= lo_sq && d_sq <= hi_sq {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && py >= 0 && px < img.width() as i32 && py < img.height() as i32 {
                    img.put_pixel(px as u32, py as u32, col);
                }
            }
        }
    }
}

fn draw_dot(img: &mut RgbImage, cx: u32, cy: u32, r: i32, col: Rgb<u8>) {
    let cx = cx as i32;
    let cy = cy as i32;
    let rsq = r * r;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= rsq {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && py >= 0 && px < img.width() as i32 && py < img.height() as i32 {
                    img.put_pixel(px as u32, py as u32, col);
                }
            }
        }
    }
}

fn draw_line(img: &mut RgbImage, x1: u32, y1: u32, x2: u32, y2: u32, col: Rgb<u8>) {
    let mut x1 = x1 as i32;
    let mut y1 = y1 as i32;
    let x2 = x2 as i32;
    let y2 = y2 as i32;
    let dx = (x2 - x1).abs();
    let dy = -(y2 - y1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        for ox in -1..=1 {
            for oy in -1..=1 {
                let px = x1 + ox;
                let py = y1 + oy;
                if px >= 0 && py >= 0 && px < img.width() as i32 && py < img.height() as i32 {
                    img.put_pixel(px as u32, py as u32, col);
                }
            }
        }

        if x1 == x2 && y1 == y2 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x1 += sx; }
        if e2 <= dx { err += dx; y1 += sy; }
    }
}
