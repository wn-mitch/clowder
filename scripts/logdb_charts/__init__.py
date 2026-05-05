"""Chart recipes for ``just logdb-chart``.

Each module in this package exposes:

    register(parser: argparse.ArgumentParser) -> None
        Add recipe-specific argparse args to the shared ``chart`` subparser.

    build(con: duckdb.DuckDBPyConnection, args: argparse.Namespace) -> alt.Chart
        Read from the DB, return an Altair chart. The runner saves it to
        ``logs/charts/<recipe>-<ts>.html``.

To add a new recipe, drop a file here. ``logdb chart --help`` lists discovered
recipes via ``pkgutil.iter_modules``.
"""
