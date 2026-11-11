//! The system print spooler. On macOS and Linux this is CUPS: printers come from `lpstat`, jobs
//! go to `lp` with the job options (copies, collation, duplex, colour). Other platforms report
//! that printing isn't available yet; the print-ready PDF can still be saved. On Windows, printers come from
//! `Get-CimInstance Win32_Printer`, and jobs go through .NET's `PrintDocument` as pictures of the sheets.

use crate::PrintError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Printer {
    pub name: String,
    pub default: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Duplex {
    #[default]
    Off,
    LongEdge,
    ShortEdge,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    /// `None` = the system default printer.
    pub printer: Option<String>,
    pub copies: u32,
    pub collate: bool,
    pub duplex: Duplex,
    pub grayscale: bool,
    pub title: String,
    /// Windows: print to this file instead of paper, for printers that can ("Microsoft Print to PDF"); for tests and
    /// automation.
    pub to_file: Option<String>,
}

impl Default for Job {
    fn default() -> Self {
        Job { printer: None, copies: 1, collate: true, duplex: Duplex::Off, grayscale: false, title: "KarnoxPDF".into(), to_file: None }
    }
}

/// Parse `lpstat -p -d` output.
pub fn parse_lpstat(out: &str) -> Vec<Printer> {
    let default = out.lines().find_map(|l| l.strip_prefix("system default destination:")).map(|s| s.trim().to_string());
    out.lines()
        .filter_map(|l| l.strip_prefix("printer "))
        .filter_map(|l| l.split_whitespace().next())
        .map(|n| Printer { name: n.to_string(), default: default.as_deref() == Some(n) })
        .collect()
}

/// The `lp` arguments for a job printing `file`.
pub fn lp_args(job: &Job, file: &str) -> Vec<String> {
    let mut a = Vec::new();
    if let Some(p) = &job.printer {
        a.extend(["-d".to_string(), p.clone()]);
    }
    a.extend(["-n".to_string(), job.copies.clamp(1, 999).to_string()]);
    a.extend(["-t".to_string(), job.title.clone()]);
    let mut opt = |o: &str| a.extend(["-o".to_string(), o.to_string()]);
    opt(if job.collate { "collate=true" } else { "collate=false" });
    opt(match job.duplex {
        Duplex::Off => "sides=one-sided",
        Duplex::LongEdge => "sides=two-sided-long-edge",
        Duplex::ShortEdge => "sides=two-sided-short-edge",
    });
    if job.grayscale {
        opt("print-color-mode=monochrome");
    }
    // The sheets are already laid out at their final size.
    opt("fit-to-page=false");
    a.push("--".into());
    a.push(file.to_string());
    a
}

/// `lpstat -p -d`, forced to print untranslated messages so [`parse_lpstat`] can read them.
///
/// `LC_ALL`/`LANG=C` is enough on Linux. macOS CUPS ignores them and follows the user's
/// interface language (`AppleLanguages`) unless `SOFTWARE` is set, in which case it uses `LANG`.
pub fn lpstat_command() -> std::process::Command {
    let mut c = std::process::Command::new("lpstat");
    c.args(["-p", "-d"]).env("LC_ALL", "C").env("LANG", "C").env("SOFTWARE", "KarnoxPDF");
    c
}

/// Parse the `Default|Name` lines the Windows listing script prints.
pub fn parse_windows_printers(out: &str) -> Vec<Printer> {
    let mut printers: Vec<Printer> = Vec::new();
    for line in out.lines() {
        let line = line.trim_start_matches('\u{feff}').trim_end_matches('\r');
        let Some((default, name)) = line.split_once('|') else { continue };
        let name = name.trim();
        if name.is_empty() || printers.iter().any(|p| p.name == name) {
            continue;
        }
        printers.push(Printer { name: name.to_string(), default: default.trim().eq_ignore_ascii_case("true") });
    }
    printers
}

/// A `powershell.exe` that opens no console window and reads no profile.
#[cfg(windows)]
fn powershell() -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut c = std::process::Command::new("powershell.exe");
    c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass"]).creation_flags(CREATE_NO_WINDOW);
    c
}

/// One sheet of a job, rendered to a picture: a PNG and its size on paper in inches.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub png: Vec<u8>,
    pub width_in: f64,
    pub height_in: f64,
}

/// The PowerShell script that prints a folder of sheets (`sheets.txt`: `file|width|height` per
/// line, in inches) through .NET's `PrintDocument`: each sheet is drawn as large as fits on the
/// page it is given, on the paper size that matches it when the printer has one.
#[cfg(windows)]
const PRINT_SCRIPT: &str = r#"param([string]$Dir, [string]$Printer, [int]$Copies, [int]$Collate, [string]$Duplex, [int]$Gray, [string]$Title, [string]$ToFile)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$inv = [Globalization.CultureInfo]::InvariantCulture
$sheets = @()
foreach ($line in Get-Content -LiteralPath (Join-Path $Dir 'sheets.txt') -Encoding UTF8) {
  $p = $line.Split('|')
  if ($p.Count -ge 3) { $sheets += ,@((Join-Path $Dir $p[0]), [double]::Parse($p[1], $inv), [double]::Parse($p[2], $inv)) }
}
if ($sheets.Count -eq 0) { throw 'there are no sheets to print' }
$doc = New-Object System.Drawing.Printing.PrintDocument
$doc.DocumentName = $Title
if ($Printer) { $doc.PrinterSettings.PrinterName = $Printer }
if (-not $doc.PrinterSettings.IsValid) { throw ('the printer ' + $Printer + ' is not available') }
$doc.PrinterSettings.Copies = [int16]$Copies
$doc.PrinterSettings.Collate = ($Collate -ne 0)
if ($doc.PrinterSettings.CanDuplex) { $doc.PrinterSettings.Duplex = [System.Drawing.Printing.Duplex]$Duplex }
$doc.DefaultPageSettings.Color = ($Gray -eq 0)
if ($ToFile) { $doc.PrinterSettings.PrintToFile = $true; $doc.PrinterSettings.PrintFileName = $ToFile }
$doc.OriginAtMargins = $false
$script:index = 0
$doc.add_QueryPageSettings({
  param($s, $e)
  $sheet = $sheets[$script:index]
  $short = [Math]::Min($sheet[1], $sheet[2]); $long = [Math]::Max($sheet[1], $sheet[2])
  $e.PageSettings.Landscape = ($sheet[1] -gt $sheet[2])
  foreach ($size in $doc.PrinterSettings.PaperSizes) {
    if ([Math]::Abs($size.Width / 100.0 - $short) -lt 0.1 -and [Math]::Abs($size.Height / 100.0 - $long) -lt 0.1) { $e.PageSettings.PaperSize = $size; break }
  }
})
$doc.add_PrintPage({
  param($s, $e)
  $sheet = $sheets[$script:index]
  $img = [System.Drawing.Image]::FromFile($sheet[0])
  try {
    $page = $e.PageBounds
    $scale = [Math]::Min($page.Width / $img.Width, $page.Height / $img.Height)
    $w = $img.Width * $scale; $h = $img.Height * $scale
    $x = ($page.Width - $w) / 2 - $e.PageSettings.HardMarginX
    $y = ($page.Height - $h) / 2 - $e.PageSettings.HardMarginY
    $e.Graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $e.Graphics.DrawImage($img, [float]$x, [float]$y, [float]$w, [float]$h)
  } finally { $img.Dispose() }
  $script:index++
  $e.HasMorePages = ($script:index -lt $sheets.Count)
})
$doc.Print()
Write-Output ('{0} sheet(s) to {1}' -f $sheets.Count, $doc.PrinterSettings.PrinterName)
"#;

/// Print sheets already rendered to pictures. Windows only (elsewhere [`submit`] sends the PDF).
pub fn submit_sheets(sheets: &[Sheet], job: &Job) -> Result<String, PrintError> {
    #[cfg(windows)]
    {
        if sheets.is_empty() {
            return Err(PrintError::NoPages);
        }
        let dir = std::env::temp_dir().join(format!(
            "karnoxpdf-print-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&dir).map_err(|e| PrintError::Spool(e.to_string()))?;
        let result = run_print_script(&dir, sheets, job);
        let _ = std::fs::remove_dir_all(&dir);
        result
    }
    #[cfg(not(windows))]
    {
        let _ = (sheets, job);
        Err(PrintError::Spool("printing pictures of the sheets is only used on Windows".into()))
    }
}

#[cfg(windows)]
fn run_print_script(dir: &std::path::Path, sheets: &[Sheet], job: &Job) -> Result<String, PrintError> {
    let spool = |e: std::io::Error| PrintError::Spool(e.to_string());
    let mut manifest = String::new();
    for (i, sheet) in sheets.iter().enumerate() {
        let name = format!("sheet-{}.png", i + 1);
        std::fs::write(dir.join(&name), &sheet.png).map_err(spool)?;
        manifest.push_str(&format!("{name}|{:.4}|{:.4}\n", sheet.width_in, sheet.height_in));
    }
    std::fs::write(dir.join("sheets.txt"), manifest).map_err(spool)?;
    let script = dir.join("print.ps1");
    std::fs::write(&script, PRINT_SCRIPT).map_err(spool)?;
    let mut command = powershell();
    command.arg("-File").arg(&script);
    command.arg("-Dir").arg(dir);
    if let Some(printer) = &job.printer {
        command.arg("-Printer").arg(printer);
    }
    command
        .arg("-Copies")
        .arg(job.copies.clamp(1, 999).to_string())
        .arg("-Collate")
        .arg(if job.collate { "1" } else { "0" })
        .arg("-Duplex")
        .arg(match job.duplex {
            Duplex::Off => "Simplex",
            Duplex::LongEdge => "Vertical",
            Duplex::ShortEdge => "Horizontal",
        })
        .arg("-Gray")
        .arg(if job.grayscale { "1" } else { "0" })
        .arg("-Title")
        .arg(&job.title);
    if let Some(file) = &job.to_file {
        command.arg("-ToFile").arg(file);
    }
    let out = command.output().map_err(|e| PrintError::Spool(format!("Windows PowerShell is not available: {e}")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(PrintError::Spool(if err.is_empty() { "the print job was refused".into() } else { err }))
    }
}

/// The printers the system knows (empty when there are none or no spooler).
pub fn printers() -> Vec<Printer> {
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    {
        match lpstat_command().output() {
            Ok(o) => parse_lpstat(&String::from_utf8_lossy(&o.stdout)),
            Err(_) => Vec::new(),
        }
    }
    #[cfg(windows)]
    {
        // `Default|Name` per printer, as UTF-8 so names with accents survive.
        let listing =
            "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Get-CimInstance Win32_Printer | ForEach-Object { '{0}|{1}' -f $_.Default, $_.Name }";
        match powershell().args(["-Command", listing]).output() {
            Ok(o) => parse_windows_printers(&String::from_utf8_lossy(&o.stdout)),
            Err(_) => Vec::new(),
        }
    }
    #[cfg(not(any(windows, all(unix, not(target_arch = "wasm32")))))]
    {
        Vec::new()
    }
}

/// Send a print-ready PDF to the spooler. Returns the spooler's message (the job id).
pub fn submit(pdf: &[u8], job: &Job) -> Result<String, PrintError> {
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    {
        let dir = std::env::temp_dir().join(format!("karnoxpdf-print-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| PrintError::Spool(e.to_string()))?;
        let file = dir.join(format!("job-{}.pdf", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())));
        std::fs::write(&file, pdf).map_err(|e| PrintError::Spool(e.to_string()))?;
        let out = std::process::Command::new("lp").args(lp_args(job, &file.to_string_lossy())).output();
        let _ = std::fs::remove_file(&file);
        let out = out.map_err(|e| PrintError::Spool(format!("the print spooler is not available: {e}")))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(PrintError::Spool(if err.is_empty() { "the print job was refused".into() } else { err }))
        }
    }
    #[cfg(not(all(unix, not(target_arch = "wasm32"))))]
    {
        let _ = (pdf, job);
        Err(PrintError::Spool("printing to a printer isn't available on this platform yet; save the print-ready PDF instead".into()))
    }
}
