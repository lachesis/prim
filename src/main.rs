use std::fs::File;
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
    let mut in_mst = vec![false; n];
    let mut edges = Vec::new();
    in_mst[start] = true;
    while edges.len() < n - 1 {
        let mut best: Option<(usize, usize, u64)> = None;
        for i in 0..n {
            if !in_mst[i] { continue; }
            for j in 0..n {
                if in_mst[j] { continue; }
                let dx = points[i].gx as i64 - points[j].gx as i64;
                let dy = points[i].gy as i64 - points[j].gy as i64;
                let d = (dx * dx + dy * dy) as u64;
                if best.map_or(true, |(_, _, bd)| d < bd) {
                    best = Some((i, j, d));
                }
            }
        }
        if let Some((i, j, _)) = best {
            in_mst[j] = true;
            edges.push((i, j));
        } else {
            break;
        }
    }
    edges
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

    let points = place_points(w, h, pts, args.seed);
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

    // Delays
    let init_delay = Delay::from_saturating_duration(Duration::from_millis(args.start_time as u64 * 1000));
    let per_edge = (edge_count).max(1) as u64;
    let edge_delay = Delay::from_saturating_duration(Duration::from_millis(args.runtime as u64 * 1000 / per_edge));
    let hold_delay = Delay::from_saturating_duration(Duration::from_millis(args.hold_time as u64 * 1000));

    let out = File::create("output.gif").expect("create output.gif failed");
    let mut encoder = GifEncoder::new(out);
    encoder.set_repeat(Repeat::Infinite).expect("set repeat failed");

    for rep in 0..args.repeats {
        let start = rep as usize % points.len();
        let edges = prim_mst(&points, start);

        let total_len: f64 = edges.iter().map(|&(i, j)| {
            let dx = points[i].gx as f64 - points[j].gx as f64;
            let dy = points[i].gy as f64 - points[j].gy as f64;
            (dx * dx + dy * dy).sqrt()
        }).sum();
        println!("start ({},{})  total edge len {:.2}", points[start].gx, points[start].gy, total_len);

        let mut cur = render_base(iw, ih, w, h, cell, margin, r, &points);
        draw_dot(&mut cur, pcx(points[start].gx), pcy(points[start].gy), r, Rgb([0, 0, 255]));
        encode_frame(&mut encoder, &cur, init_delay);

        for &(i, j) in &edges {
            draw_line(&mut cur, pcx(points[i].gx), pcy(points[i].gy), pcx(points[j].gx), pcy(points[j].gy), Rgb([255, 0, 0]));
            encode_frame(&mut encoder, &cur, edge_delay);
        }

        encode_frame(&mut encoder, &cur, hold_delay);
    }

    println!("output.gif  {}x{}  {} pts  {} edges  seed {}  repeats {}", w, h, pts, edge_count, args.seed, args.repeats);
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
    let lo = r - s / 2;
    let hi = r + s / 2;
    for dy in -hi..=hi {
        for dx in -hi..=hi {
            let d = ((dx * dx + dy * dy) as f64).sqrt().round() as i32;
            if d >= lo && d <= hi {
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
