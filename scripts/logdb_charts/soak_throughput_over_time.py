"""soak-throughput-over-time — sim tick throughput across commits.

One point per completed soak: ticks-per-second (``(tick_end - tick_start)
/ duration_secs``) vs. soak time, colored by archive, with a per-archive
mean line and an overall LOESS trend.

Throughput is the wall-clock cost of the sim: a fixed-duration soak
(``just soak`` = 900 s) covers fewer ticks — and therefore fewer
``seasons_survived`` — as per-tick cost grows. A downward trend means the
sim got slower (new per-tick systems outrunning perf passes), even when
the colony itself is healthy. Pairs with ``colony-score-over-time``:
that chart tracks *welfare* drift, this one tracks *cost* drift.

Filters: ``--archive PATTERN`` (DuckDB LIKE), ``--seed INT``,
``--kind STR`` (e.g. 'flat'), ``--min-duration SECS`` (default 600, drops
scenarios / aborted short runs), ``--commit HASH_PREFIX``.

Note: only ``footer_written`` runs are plotted — an aborted soak has a
truncated ``tick_end`` that would read as spuriously low throughput.
"""

from __future__ import annotations

import argparse

import altair as alt  # type: ignore[import-not-found]


def register(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--archive", default=None,
                        help="DuckDB LIKE pattern to filter archives "
                             "(e.g. 'tuned-42-%%')")
    parser.add_argument("--seed", type=int, default=None,
                        help="restrict to a single seed")
    parser.add_argument("--kind", default=None,
                        help="restrict to a run kind (e.g. 'flat')")
    parser.add_argument("--commit", default=None,
                        help="restrict to a commit_hash_short prefix")
    parser.add_argument("--min-duration", type=int, default=600,
                        help="minimum duration_secs to count as a soak "
                             "(default 600; drops scenarios + aborts)")


def _filter_clause(args: argparse.Namespace) -> tuple[str, list]:
    # `r.`-qualified so the clause drops into the joined query below.
    parts = ["r.footer_written = TRUE", "r.duration_secs >= ?"]
    params: list = [args.min_duration]
    if args.archive:
        parts.append("r.archive LIKE ?")
        params.append(args.archive)
    if args.seed is not None:
        parts.append("r.seed = ?")
        params.append(args.seed)
    if args.kind:
        parts.append("r.kind = ?")
        params.append(args.kind)
    if args.commit:
        parts.append("r.commit_hash_short LIKE ?")
        params.append(f"{args.commit}%")
    return " WHERE " + " AND ".join(parts), params


def build(con, args: argparse.Namespace) -> alt.Chart:
    where, params = _filter_clause(args)

    sql = f"""
        SELECT
            r.run_id,
            r.archive,
            r.kind,
            r.seed,
            r.commit_hash_short,
            r.commit_time,
            r.duration_secs,
            (r.tick_end - r.tick_start) AS elapsed_ticks,
            (r.tick_end - r.tick_start) / NULLIF(r.duration_secs, 0)
                AS ticks_per_sec,
            (r.tick_end - r.tick_start) / 20000.0 AS seasons_covered,
            to_timestamp(i.mtime_ns / 1e9) AS soak_time
        FROM runs r
        JOIN ingested_files i
          ON i.file_path = r.events_path AND i.role = 'events'
        {where}
        ORDER BY i.mtime_ns
    """
    df = con.execute(sql, params).fetchdf()

    if df.empty:
        return alt.Chart(df).mark_text(text="no completed soaks match").properties(
            width=720, height=400, title="soak-throughput-over-time"
        )

    base = alt.Chart(df).encode(
        x=alt.X("soak_time:T", title="soak time"),
        color=alt.Color("archive:N", title="archive"),
    )
    points = base.mark_circle(size=70, opacity=0.8).encode(
        y=alt.Y("ticks_per_sec:Q", title="throughput (ticks / sec)"),
        tooltip=[
            alt.Tooltip("archive:N"),
            alt.Tooltip("kind:N"),
            alt.Tooltip("seed:Q"),
            alt.Tooltip("commit_hash_short:N", title="commit"),
            alt.Tooltip("soak_time:T", title="soak time"),
            alt.Tooltip("commit_time:T", title="commit time"),
            alt.Tooltip("ticks_per_sec:Q", title="ticks/sec", format=".1f"),
            alt.Tooltip("elapsed_ticks:Q", format=","),
            alt.Tooltip("seasons_covered:Q", title="seasons covered", format=".2f"),
        ],
    )
    means = (
        alt.Chart(df)
        .mark_line(point=True, strokeDash=[4, 2])
        .encode(
            x=alt.X("soak_time:T"),
            y=alt.Y("mean(ticks_per_sec):Q"),
            color=alt.Color("archive:N"),
        )
    )
    # Overall cost trajectory: LOESS over all runs, archive-agnostic. The
    # trend is non-monotonic (feature work pushes cost up, perf passes pull
    # it down), so LOESS reads better than OLS.
    trend = (
        alt.Chart(df)
        .transform_loess("soak_time", "ticks_per_sec", bandwidth=0.4)
        .mark_line(color="#222", strokeWidth=2.5, opacity=0.85)
        .encode(x=alt.X("soak_time:T"), y=alt.Y("ticks_per_sec:Q"))
    )

    bits: list[str] = []
    if args.archive:
        bits.append(f"archive LIKE {args.archive!r}")
    if args.seed is not None:
        bits.append(f"seed={args.seed}")
    if args.kind:
        bits.append(f"kind={args.kind}")
    if args.commit:
        bits.append(f"commit~{args.commit}")
    bits.append(f"duration>={args.min_duration}s")
    subtitle = " · ".join(bits)

    return (
        (points + means + trend)
        .resolve_scale(color="independent")
        .properties(
            width=720,
            height=420,
            title=alt.TitleParams(
                text="Soak throughput over time",
                subtitle=subtitle,
                anchor="start",
            ),
        )
        .add_params(alt.selection_interval(bind="scales", name="tp_zoom"))
    )
