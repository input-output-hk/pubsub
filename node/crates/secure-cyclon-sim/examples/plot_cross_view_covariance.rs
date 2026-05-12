//! Render the cross-view covariance findings as a single SVG plot.
//!
//! Writes the SVG to stdout. Usage from the workspace root:
//!
//! ```text
//! cargo run --release --example plot_cross_view_covariance \
//!     -p secure-cyclon-sim > ../docs/cyclon-cross-view-covariance.svg
//! ```
//!
//! Empirical points are hard-coded from the most recent experiment run
//! (`docs/cyclon-cross-view-covariance.csv`). Re-running the underlying
//! experiment with different parameters requires updating `DATA` below.

use std::fmt::Write as _;

const C: f64 = 10.0;
const DATA: &[(f64, f64)] = &[
    (50.0, -3.6054),
    (100.0, -5.8235),
    (200.0, -7.3768),
    (500.0, -8.4530),
];

const W: f64 = 720.0;
const H: f64 = 460.0;
const ML: f64 = 80.0;
const MR: f64 = 30.0;
const MT: f64 = 56.0;
const MB: f64 = 72.0;

const X_MIN_LOG: f64 = 1.55; // ≈ log10(35)
const X_MAX_LOG: f64 = 2.85; // ≈ log10(708)
const Y_MIN: f64 = -12.0;
const Y_MAX: f64 = 1.5;

fn pw() -> f64 {
    W - ML - MR
}
fn ph() -> f64 {
    H - MT - MB
}

fn x_to_px(n: f64) -> f64 {
    let frac = (n.log10() - X_MIN_LOG) / (X_MAX_LOG - X_MIN_LOG);
    ML + frac * pw()
}

fn y_to_px(y: f64) -> f64 {
    let frac = (Y_MAX - y) / (Y_MAX - Y_MIN);
    MT + frac * ph()
}

fn bound_at(n: f64) -> f64 {
    -C * (1.0 - C / n)
}

fn main() {
    let mut s = String::new();
    let font = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif";

    let _ = writeln!(
        s,
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {W} {H}' font-family=\"{font}\" font-size='13'>"
    );
    s.push_str("<rect width='100%' height='100%' fill='white'/>");

    // Title
    let _ = write!(
        s,
        "<text x='{cx}' y='28' text-anchor='middle' font-size='17' font-weight='600'>Cyclon cross-view covariance vs network size</text>",
        cx = W / 2.0
    );

    let x0 = ML;
    let y0 = MT;
    let x1 = ML + pw();
    let y1 = MT + ph();

    // Plot frame
    let _ = write!(
        s,
        "<rect x='{x0}' y='{y0}' width='{w}' height='{h}' fill='none' stroke='#333' stroke-width='1.5'/>",
        w = pw(),
        h = ph()
    );

    // Y axis ticks and gridlines
    for &y in &[0.0_f64, -2.0, -4.0, -6.0, -8.0, -10.0, -12.0] {
        let yp = y_to_px(y);
        let _ = write!(
            s,
            "<line x1='{}' y1='{yp}' x2='{x0}' y2='{yp}' stroke='#333' stroke-width='1'/>",
            x0 - 5.0
        );
        let _ = write!(
            s,
            "<text x='{}' y='{}' text-anchor='end'>{}</text>",
            x0 - 10.0,
            yp + 4.0,
            y as i32
        );
        if y > Y_MIN && y < Y_MAX {
            let _ = write!(
                s,
                "<line x1='{x0}' y1='{yp}' x2='{x1}' y2='{yp}' stroke='#eee' stroke-width='1'/>"
            );
        }
    }
    let _ = write!(
        s,
        "<text transform='rotate(-90)' x='-{cy}' y='22' text-anchor='middle' font-size='14'>Cov · N²</text>",
        cy = MT + ph() / 2.0
    );

    // X axis ticks (log scale)
    for &n in &[50.0_f64, 100.0, 200.0, 500.0] {
        let xp = x_to_px(n);
        let _ = write!(
            s,
            "<line x1='{xp}' y1='{y1}' x2='{xp}' y2='{}' stroke='#333' stroke-width='1'/>",
            y1 + 5.0
        );
        let _ = write!(
            s,
            "<text x='{xp}' y='{}' text-anchor='middle'>{}</text>",
            y1 + 22.0,
            n as i32
        );
    }
    let _ = write!(
        s,
        "<text x='{cx}' y='{ty}' text-anchor='middle' font-size='14'>network size N (log scale)</text>",
        cx = ML + pw() / 2.0,
        ty = H - 22.0
    );

    // Reference: independence (Cov = 0)
    let y_indep = y_to_px(0.0);
    let _ = write!(
        s,
        "<line x1='{x0}' y1='{y_indep}' x2='{x1}' y2='{y_indep}' stroke='#999' stroke-width='1' stroke-dasharray='3 3'/>"
    );
    let _ = write!(
        s,
        "<text x='{}' y='{}' font-size='11' fill='#666' text-anchor='end'>Independence (Cov = 0)</text>",
        x1 - 8.0,
        y_indep - 6.0
    );

    // Reference: conservation limit −c
    let y_lim = y_to_px(-C);
    let _ = write!(
        s,
        "<line x1='{x0}' y1='{y_lim}' x2='{x1}' y2='{y_lim}' stroke='#999' stroke-width='1' stroke-dasharray='3 3'/>"
    );
    let _ = write!(
        s,
        "<text x='{}' y='{}' font-size='11' fill='#666' text-anchor='end'>Conservation limit (−c = −10)</text>",
        x1 - 8.0,
        y_lim - 6.0
    );

    // Conservation curve: bound · N² = −c(1 − c/N), sampled densely
    let mut curve = String::from("M ");
    for i in 0..=160 {
        let frac = i as f64 / 160.0;
        let log_n = X_MIN_LOG + frac * (X_MAX_LOG - X_MIN_LOG);
        let n = 10f64.powf(log_n);
        let xp = x_to_px(n);
        let yp = y_to_px(bound_at(n));
        if i > 0 {
            curve.push_str(" L ");
        }
        let _ = write!(curve, "{xp:.2},{yp:.2}");
    }
    let _ = write!(
        s,
        "<path d='{curve}' fill='none' stroke='#c0392b' stroke-width='2' stroke-dasharray='6 4'/>"
    );

    // Empirical polyline + markers + labels
    let mut emp = String::from("M ");
    for (i, &(n, y)) in DATA.iter().enumerate() {
        let xp = x_to_px(n);
        let yp = y_to_px(y);
        if i > 0 {
            emp.push_str(" L ");
        }
        let _ = write!(emp, "{xp:.2},{yp:.2}");
    }
    let _ = write!(
        s,
        "<path d='{emp}' fill='none' stroke='#1f6fb4' stroke-width='2.5'/>"
    );
    for &(n, y) in DATA {
        let xp = x_to_px(n);
        let yp = y_to_px(y);
        let _ = write!(
            s,
            "<circle cx='{xp}' cy='{yp}' r='5.5' fill='#1f6fb4' stroke='white' stroke-width='2'/>"
        );
        let _ = write!(
            s,
            "<text x='{}' y='{}' font-size='11' fill='#1f6fb4' font-weight='600'>{:.2}</text>",
            xp + 9.0,
            yp - 7.0,
            y
        );
    }

    // Legend (inside plot, top-left)
    let lg_x = ML + 18.0;
    let lg_y = MT + 26.0;
    let _ = write!(
        s,
        "<rect x='{}' y='{}' width='290' height='56' fill='white' fill-opacity='0.92' stroke='#ddd' stroke-width='1' rx='4'/>",
        lg_x - 8.0,
        lg_y - 18.0
    );
    let _ = write!(
        s,
        "<line x1='{}' y1='{}' x2='{}' y2='{}' stroke='#1f6fb4' stroke-width='2.5'/>",
        lg_x,
        lg_y,
        lg_x + 28.0,
        lg_y
    );
    let _ = write!(
        s,
        "<circle cx='{}' cy='{}' r='5' fill='#1f6fb4' stroke='white' stroke-width='2'/>",
        lg_x + 14.0,
        lg_y
    );
    let _ = write!(
        s,
        "<text x='{}' y='{}' font-size='12'>empirical Cov · N²</text>",
        lg_x + 38.0,
        lg_y + 4.0
    );

    let lg_y2 = lg_y + 22.0;
    let _ = write!(
        s,
        "<line x1='{}' y1='{}' x2='{}' y2='{}' stroke='#c0392b' stroke-width='2' stroke-dasharray='6 4'/>",
        lg_x,
        lg_y2,
        lg_x + 28.0,
        lg_y2
    );
    let _ = write!(
        s,
        "<text x='{}' y='{}' font-size='12'>conservation reference: −c · (1 − c/N)</text>",
        lg_x + 38.0,
        lg_y2 + 4.0
    );

    s.push_str("</svg>\n");

    print!("{s}");
}
