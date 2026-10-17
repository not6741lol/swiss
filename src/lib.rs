//! swiss — one small toolkit, three small jobs.
//!
//! This is the library half. The `swiss` command-line binary and the `swiss-gui`
//! windowed binary are both thin front-ends over the functions in here, so the two
//! can never drift apart.
//!
//! Each module splits in two:
//!
//! * a **data API** — plain functions that do the work and return what happened
//!   (`unlock::who_holds`, `pdf::merge`, `brand::favicons`, ...). No printing.
//! * a **`run`** — the command-line front-end, which calls the data API and prints.
//!
//! The CLI keeps its tiny dependency graph: nothing in here pulls in a GUI toolkit.

pub mod brand;
pub mod pdf;
pub mod unlock;
