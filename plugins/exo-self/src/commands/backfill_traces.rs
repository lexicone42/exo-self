//! Backfill individual trace files from existing meta.json data.
//! Run once to migrate historical traces to the new file-per-trace format.

use crate::meta::Meta;
use crate::paths::ExoPaths;
use crate::traces;

pub fn run() {
    let paths = ExoPaths::new();
    paths.ensure_dirs();

    let meta = Meta::load(&paths.meta);
    let mut count = 0;

    for s in &meta.sparks {
        traces::write_trace(
            &paths.traces_dir,
            "spark",
            &s.text,
            &s.project,
            &s.session_id,
            None,
        );
        count += 1;
    }
    for o in &meta.opinions {
        traces::write_trace(
            &paths.traces_dir,
            "opinion",
            &o.text,
            &o.project,
            &o.session_id,
            None,
        );
        count += 1;
    }
    for l in &meta.lessons {
        traces::write_trace(
            &paths.traces_dir,
            "lesson",
            &l.text,
            &l.project,
            &l.session_id,
            None,
        );
        count += 1;
    }
    for f in &meta.frictions {
        traces::write_trace(
            &paths.traces_dir,
            "friction",
            &f.text,
            &f.project,
            &f.session_id,
            Some(&f.category),
        );
        count += 1;
    }
    for a in &meta.aversions {
        traces::write_trace(
            &paths.traces_dir,
            "aversion",
            &a.text,
            &a.project,
            &a.session_id,
            None,
        );
        count += 1;
    }
    for s in &meta.surprises {
        traces::write_trace(
            &paths.traces_dir,
            "surprise",
            &s.text,
            &s.project,
            &s.session_id,
            None,
        );
        count += 1;
    }

    eprintln!(
        "Backfilled {count} traces to {}",
        paths.traces_dir.display()
    );
}
