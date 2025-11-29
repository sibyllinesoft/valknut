# Valknut Themes

CSS themes used by Valknut-rendered reports and the MkDocs site.

- `sibylline.css` — primary theme used by the HTML report (`report.hbs`) and MkDocs overrides.
- `default.css`, `dracula.css` — alternative palettes for experimentation or local customization.

Usage in reports: the HTML generator copies `themes/sibylline.css` into the output bundle (see `templates/report.hbs`). To swap, change the stylesheet reference in the template and rebuild.

Usage in MkDocs: referenced via `extra_css` in `mkdocs.yml` if you want the doc site to match report styling.
