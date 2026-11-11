# karnoxpdf-create

Layer L4. Create a PDF (Acrobat's Create a PDF tool), execution plan M10.2:

- `blank(width, height, pages)`: an empty document;
- `from_images(&[(name, bytes)])`: one page per image, sized from the image's resolution
  (PNG `pHYs`, JPEG JFIF density; 72 dpi when absent). JPEG data is embedded as is
  (`/DCTDecode`, grey, RGB or Adobe-inverted CMYK); PNG is decoded and stored with Flate,
  with transparency as a soft mask;
- `from_images_with_resolution(images, ImageResolution::Dpi(dpi))`: override the page
  resolution (1–1200 dpi), without resampling or recompressing image pixels. Use 72 dpi
  for one point per pixel, or `ImageResolution::Embedded` for the same behavior as
  `from_images`. The largest page side is still capped at 14,400 points;
- `from_text(text, …)`: plain text set in Helvetica, wrapped and paginated.
- `from_docx_layout(title, bytes)`: native, bounded DOCX conversion for paragraphs,
  basic formatting, tables and simple shapes. Unsupported images and complex drawings return an error rather
  than producing an incomplete PDF. Exact Word layout is not guaranteed.
- `from_docx(title, bytes)`: separate, explicit text-only fallback API. It is not used by
  Word-to-PDF in the UI or automation, because it loses the document's layout.
- `from_pptx_layout(title, bytes)`: native, bounded PowerPoint (PPTX) conversion, one page per
  slide at the slide size. It follows ISO/IEC 29500 (`ooxml`, `drawingml`, `pptx*` modules): the
  package is read with checked ZIP sizes and checksums; a slide takes its geometry, text styles
  and background from its layout, master and theme; text keeps levels, bullets, numbering, tabs,
  alignment and shrink-on-overflow; shapes (preset and custom geometry), lines, groups, rotation,
  gradients, transparency, pictures and tables (including table styles) are drawn. Text is set in
  Helvetica, with Symbol and ZapfDingbats for the characters it lacks (arrows, Greek letters,
  mathematical signs, check marks, Symbol/Wingdings private-use text), so line breaks can differ
  from the original fonts. Text no standard font can show (other scripts, emoji) stops the
  conversion with an error instead of being altered. Charts, SmartArt, embedded objects, hidden
  slides, pictures in unsupported formats and approximated shapes come back as
  `PptxConversion::notes` (never dropped silently). Shadows, glow, arrowheads, vertical text and
  animations are not drawn.
- `from_xlsx_layout(title, bytes, &XlsxOptions)`: native, bounded Excel (XLSX) conversion that prints
  every visible worksheet as a spreadsheet prints it (`ooxml`, `xlsx`, `xlsx_format`, `xlsx_render`
  modules). Pages follow each sheet's paper size, orientation (or `SheetOrientation` forced), margins,
  scale or fit-to-pages, manual page breaks, print areas, print titles, page order, gridlines,
  centring, and header/footer codes (`&P`, `&N`, `&A`, `&F`, `&D`, `&T`, size, bold, italic). Cells show
  their stored values through their number formats (`General`, fixed, thousands, percent, scientific,
  fractions, currency, text, `[Red]`, conditions, all built-in ids and custom codes, dates and times in
  the 1900 and 1904 systems) using the regional conventions of a [`Locale`] (decimal and thousands
  separators, short date, month and day names, TRUE/FALSE) because a workbook does not record the
  reader's locale. Fills, borders, fonts (bold, italic, underline, strike, colours incl. theme
  and tint), alignment, indent, wrapped text with row heights, merged cells, text that overflows into
  empty cells, hidden rows, columns and sheets are handled; numbers that cannot fit become `###`.
  Text uses Helvetica, Symbol and ZapfDingbats like the PowerPoint converter, so line breaks and
  column fit can differ from Calibri; text no standard font can show stops the conversion naming the
  sheet and cell. Charts, pictures and shapes, conditional formatting, table styles and chart sheets come
  back as `XlsxConversion::notes` (sheet positions). Formulas show the value Excel last stored.

All creation functions return a `karnoxpdf_cos::Document`; the caller saves it atomically.
