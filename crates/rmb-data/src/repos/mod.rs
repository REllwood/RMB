//! Repositories — the only place SQL is executed. Parameterized queries only (never string
//! interpolation). Each module owns one aggregate; pure rules live in `rmb-domain`.

pub mod meta;
