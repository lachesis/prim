prim — Prim's algorithm visualizer

Build MST on random grid points, render animated GIF.

Usage:

    prim [OPTIONS]

Options:

    --seed <N>         RNG seed (default: 42)
    --width <N>        Grid width (default: 64)
    --height <N>       Grid height (default: 64)
    --points <N>       Point count (default: 10% of grid cells)
    --runtime <N>      Process phase seconds (default: 15)
    --start-time <N>   Initial condition hold seconds (default: 2)
    --hold-time <N>    Solution hold seconds (default: 3)
    --repeats <N>      Number of runs, each from a different start point (default: 1)
    --step <N>         Frame skip: 0 = auto (cap 30fps), 1 = every edge, N = every Nth edge (default: 0)
    --output <FILE>    Output GIF path (default: output.gif)

Per repeat prints: `start (x,y)  total edge len N.NN`

Output: `output.gif` (infinite loop)

Build: `cargo build --release`


---

Other than this single line, and the contents of user_messages.md, all code and docs were 100% generated with Deepseek v4 Flash via Claude Code
