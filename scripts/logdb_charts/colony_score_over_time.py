"""colony-score-over-time — within-run arc + across-commits trend.

Two horizontally-concatenated panels:

  Left:  aggregate vs. tick, one line per run, colored by run label.
  Right: final_aggregate vs. commit_time, one point per run, colored by
         archive, with a per-archive mean line over commits.

Filters: ``--archive PATTERN`` (DuckDB LIKE), ``--seed INT``,
``--commit HASH_PREFIX``, ``--smooth N`` (rolling-mean window in samples,
applied to the within-run line). All filters apply to both panels where
they make sense.
"""

from __future__ import annotations

import argparse

import altair as alt  # type: ignore[import-not-found]

TICK_ORIGIN = 1_200_000  # absolute-tick origin (60 × ticks_per_season); see CLAUDE.md "Verification"


def register(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--archive", default=None,
                        help="DuckDB LIKE pattern to filter archives "
                             "(e.g. 'baseline-%%')")
    parser.add_argument("--seed", type=int, default=None,
                        help="restrict to a single seed")
    parser.add_argument("--commit", default=None,
                        help="restrict to a commit_hash_short prefix")
    parser.add_argument("--smooth", type=int, default=0,
                        help="rolling-mean window for the within-run line "
                             "(samples; 0 = off)")
    parser.add_argument("--max-runs", type=int, default=40,
                        help="cap within-run series for legibility (default 40)")


def _filter_clause(args: argparse.Namespace) -> tuple[str, list]:
    parts: list[str] = []
    params: list = []
    if args.archive:
        parts.append("archive LIKE ?")
        params.append(args.archive)
    if args.seed is not None:
        parts.append("seed = ?")
        params.append(args.seed)
    if args.commit:
        parts.append("commit_hash_short LIKE ?")
        params.append(f"{args.commit}%")
    where = (" WHERE " + " AND ".join(parts)) if parts else ""
    return where, params


def build(con, args: argparse.Namespace) -> alt.Chart:
    where, params = _filter_clause(args)

    # ------ within-run panel
    run_filter_sql = f"SELECT run_id FROM runs{where}"
    within_sql = f"""
        SELECT
            cs.run_id,
            cs.tick,
            cs.aggregate,
            cs.welfare,
            cs.living_cats,
            r.archive,
            r.kind,
            r.seed,
            r.rep,
            r.focal,
            r.forced_weather,
            r.commit_hash_short,
            to_timestamp(i.mtime_ns / 1e9) AS soak_time,
            strftime(to_timestamp(i.mtime_ns / 1e9), '%Y-%m-%d %H:%M') AS soak_time_label,
            COALESCE(
                strftime(to_timestamp(i.mtime_ns / 1e9), '%Y-%m-%d %H:%M')
                  || '  ' || r.archive
                  || '/' || CAST(r.seed AS VARCHAR)
                  || COALESCE('-' || CAST(r.rep AS VARCHAR), '')
                  || COALESCE('-' || r.focal, '')
                  || COALESCE('-' || r.forced_weather, ''),
                r.run_id
            ) AS label
        FROM colony_scores cs
        JOIN runs r USING (run_id)
        JOIN ingested_files i
          ON i.file_path = r.events_path AND i.role = 'events'
        WHERE cs.run_id IN ({run_filter_sql})
          AND cs.tick >= {TICK_ORIGIN}
    """
    within_df = con.execute(within_sql, params).fetchdf()

    if within_df.empty:
        return alt.Chart(within_df).mark_text(text="no data").properties(
            width=300, height=200, title="colony-score-over-time"
        )

    if args.max_runs and within_df["label"].nunique() > args.max_runs:
        # Keep the N most-recent runs (by soak_time), not the N largest
        # by row count. Long baselines would otherwise crowd out recent
        # short-soak / scenario / sweep runs that the user is iterating on.
        keep = (
            within_df.groupby("label")["soak_time"].max()
            .sort_values(ascending=False)
            .head(args.max_runs)
            .index.tolist()
        )
        within_df = within_df[within_df["label"].isin(keep)]

    if args.smooth and args.smooth > 1:
        within_df = within_df.sort_values(["label", "tick"]).copy()
        within_df["aggregate"] = (
            within_df.groupby("label")["aggregate"]
            .transform(lambda s: s.rolling(args.smooth, min_periods=1).mean())
        )

    within_chart = (
        alt.Chart(within_df)
        .mark_line(opacity=0.75)
        .encode(
            x=alt.X(
                "tick:Q",
                title="tick",
                scale=alt.Scale(domainMin=TICK_ORIGIN, clamp=True),
            ),
            y=alt.Y("aggregate:Q", title="ColonyScore.aggregate"),
            color=alt.Color(
                "label:N", title="run (soak time + archive)",
                legend=alt.Legend(columns=1, symbolLimit=80),
                sort=alt.SortField(field="soak_time", order="descending"),
            ),
            tooltip=[
                alt.Tooltip("label:N", title="run"),
                alt.Tooltip("soak_time:T", title="soak time"),
                alt.Tooltip("tick:Q", format=","),
                alt.Tooltip("aggregate:Q", format=".1f"),
                alt.Tooltip("welfare:Q", format=".3f"),
                alt.Tooltip("living_cats:Q"),
            ],
        )
        .properties(width=520, height=380, title="Within-run arc")
        .add_params(alt.selection_interval(bind="scales", name="within_zoom"))
    )

    # ------ across-runs panel (x = soak time, not commit time)
    across_sql = f"""
        SELECT
            r.run_id,
            r.archive,
            r.kind,
            r.commit_hash_short,
            r.commit_time,
            to_timestamp(i.mtime_ns / 1e9) AS soak_time,
            f.final_aggregate,
            f.final_welfare,
            f.final_living_cats
        FROM runs r
        JOIN run_footers f USING (run_id)
        JOIN ingested_files i
          ON i.file_path = r.events_path AND i.role = 'events'
        {where}
        ORDER BY i.mtime_ns
    """
    across_df = con.execute(across_sql, params).fetchdf()

    if across_df.empty:
        across_chart = alt.Chart().mark_text(
            text="no footer-complete runs"
        ).properties(width=520, height=380, title="Across-runs trend")
    else:
        base = alt.Chart(across_df).encode(
            x=alt.X("soak_time:T", title="soak time"),
            color=alt.Color("archive:N", title="archive"),
        )
        points = base.mark_circle(size=70, opacity=0.8).encode(
            y=alt.Y("final_aggregate:Q", title="final ColonyScore.aggregate"),
            tooltip=[
                alt.Tooltip("archive:N"),
                alt.Tooltip("kind:N"),
                alt.Tooltip("commit_hash_short:N"),
                alt.Tooltip("soak_time:T", title="soak time"),
                alt.Tooltip("commit_time:T", title="commit time"),
                alt.Tooltip("final_aggregate:Q", format=".1f"),
                alt.Tooltip("final_welfare:Q", format=".3f"),
                alt.Tooltip("final_living_cats:Q"),
            ],
        )
        means = (
            alt.Chart(across_df)
            .mark_line(point=True, strokeDash=[4, 2])
            .encode(
                x=alt.X("soak_time:T"),
                y=alt.Y("mean(final_aggregate):Q"),
                color=alt.Color("archive:N"),
            )
        )
        # Overall trendline: LOESS smoother across all runs, ignoring archive.
        # LOESS handles non-monotonic trajectories better than OLS here, since
        # the score trajectory across commits is non-linear (regressions +
        # recoveries during balance work).
        trend = (
            alt.Chart(across_df)
            .transform_loess(
                on="soak_time", loess="final_aggregate", bandwidth=0.4,
            )
            .mark_line(color="#222", strokeWidth=2.5, opacity=0.85)
            .encode(
                x=alt.X("soak_time:T"),
                y=alt.Y("final_aggregate:Q"),
            )
        )
        across_chart = (
            (points + means + trend)
            .resolve_scale(color="independent")
            .properties(width=520, height=380, title="Across-runs trend (by soak time)")
            .add_params(alt.selection_interval(bind="scales", name="across_zoom"))
        )

    title = "ColonyScore over time"
    bits: list[str] = []
    if args.archive:
        bits.append(f"archive LIKE {args.archive!r}")
    if args.seed is not None:
        bits.append(f"seed={args.seed}")
    if args.commit:
        bits.append(f"commit~{args.commit}")
    if args.smooth and args.smooth > 1:
        bits.append(f"smooth={args.smooth}")
    subtitle = " · ".join(bits) if bits else "all archives, all runs"

    return (
        alt.hconcat(within_chart, across_chart)
        .resolve_scale(color="independent")
        .properties(
            title=alt.TitleParams(text=title, subtitle=subtitle, anchor="start")
        )
    )
