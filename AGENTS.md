# Code conventions

- Write code documentation, Rustdoc, docstrings and comments in English.
- Prefer ASCII in source files, including identifiers and punctuation.
- Use Unicode escapes when non-ASCII string values are needed for UI content,
  asset names or Unicode test coverage. Preserve their runtime values and meaning.

# Documentation placement

- Keep code review reports, fix notes and their supporting evidence in `debug/`,
  not in the formal `doc/` directory. Store review data files in `debug/data/`.
