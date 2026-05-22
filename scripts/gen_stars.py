"""Rebuild star catalog binary for prim's secret sky mode.

Usage:
    python3 scripts/gen_stars.py               # mag < 8, all dec
    python3 scripts/gen_stars.py --mag 6.5     # custom mag limit
    python3 scripts/gen_stars.py --lat 39 --lon -84  # filter visible dec
"""

import argparse, csv, gzip, struct, urllib.request, os, sys

HYG_URL = "https://codeberg.org/astronexus/hyg/media/branch/main/data/hyg/CURRENT/hyg_v42.csv.gz"
OUT = os.path.join(os.path.dirname(__file__), "..", "src", "stars.bin")

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mag", type=float, default=8.0, help="brightest magnitude limit (default 8)")
    ap.add_argument("--lat", type=float, help="observer latitude (deg N, positive). If set, filters visible dec range")
    ap.add_argument("--lon", type=float, help="observer longitude (deg E, positive). Only used for note")
    args = ap.parse_args()

    # Download catalog
    path = "/tmp/hyg_v42.csv.gz"
    if not os.path.exists(path):
        print(f"Downloading {HYG_URL} ...", file=sys.stderr)
        urllib.request.urlretrieve(HYG_URL, path)

    # Parse
    stars = []
    with gzip.open(path, "rt", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            mag_s = row["mag"].strip()
            dec_s = row["dec"].strip()
            ra_s = row["ra"].strip()
            if not mag_s or not dec_s or not ra_s:
                continue
            mag = float(mag_s)
            dec = float(dec_s)
            ra = float(ra_s)
            if mag < -10:  # skip Sun
                continue
            if mag >= args.mag:
                continue
            if args.lat is not None and dec < args.lat - 90.0:
                # Never rises at this latitude
                continue
            stars.append((ra, dec, mag))

    # Sort brightest first
    stars.sort(key=lambda x: x[2])

    # Write binary: u32 count, then f32 triplets (ra_hours, dec_deg, mag)
    out_path = os.path.abspath(OUT)
    data = struct.pack("<I", len(stars))
    for ra, dec, mag in stars:
        data += struct.pack("<fff", ra, dec, mag)

    with open(out_path, "wb") as f:
        f.write(data)

    loc = f"lat={args.lat} lon={args.lon}" if args.lat else "global"
    print(f"{len(stars)} stars (mag<{args.mag}, {loc}) → {out_path} ({len(data)} bytes)", file=sys.stderr)

if __name__ == "__main__":
    main()
