use aegoris_core::parse::{PositionedLayout, PositionedRun};
use anyhow::{bail, Context, Result};
use pdf_extract::{MediaBox, OutputDev, OutputError, Transform};

/// Extract positioned text from a PDF.
///
/// This is the I/O half of PDF profile support: it walks the PDF's content
/// streams and emits one [`PositionedRun`] per word. The pure normalization
/// from positions to a `Profile` lives in `aegoris-core`, so the layout can be
/// unit-tested without a PDF.
pub fn extract_layout(bytes: &[u8]) -> Result<PositionedLayout> {
    let document = pdf_extract::Document::load_mem(bytes).context("reading PDF document")?;
    if document.is_encrypted() {
        bail!("PDF is encrypted; cannot extract the profile");
    }

    let mut output = LayoutOutput::default();
    pdf_extract::output_doc(&document, &mut output).context("extracting text from PDF")?;

    if output.runs.is_empty() {
        bail!("PDF has no extractable text (it may be a scan); OCR is not supported");
    }
    Ok(PositionedLayout { runs: output.runs })
}

#[derive(Default)]
struct LayoutOutput {
    runs: Vec<PositionedRun>,
    page: usize,
    page_height: f64,
    in_word: bool,
    positioned: bool,
    word: String,
    word_x: f64,
    word_end: f64,
    word_y: f64,
    word_size: f64,
}

impl LayoutOutput {
    fn flush_word(&mut self) {
        if self.in_word {
            let text = self.word.trim();
            if !text.is_empty() {
                self.runs.push(PositionedRun {
                    page: self.page,
                    x: self.word_x,
                    y: self.word_y,
                    width: (self.word_end - self.word_x).max(0.0),
                    size: self.word_size,
                    text: text.to_string(),
                });
            }
            self.word.clear();
        }
        self.in_word = false;
        self.positioned = false;
    }
}

impl OutputDev for LayoutOutput {
    fn begin_page(
        &mut self,
        page_num: u32,
        media_box: &MediaBox,
        _art_box: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        self.page = page_num.saturating_sub(1) as usize;
        self.page_height = media_box.ury - media_box.lly;
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), OutputError> {
        self.flush_word();
        Ok(())
    }

    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        _spacing: f64,
        font_size: f64,
        char: &str,
    ) -> Result<(), OutputError> {
        if !self.in_word {
            self.in_word = true;
            self.word.clear();
            self.positioned = false;
        }
        let scale = (trm.m11 * trm.m11 + trm.m12 * trm.m12).sqrt();
        if !self.positioned {
            self.word_x = trm.m31;
            self.word_y = self.page_height - trm.m32;
            self.word_size = font_size;
            self.positioned = true;
        }
        self.word_end = trm.m31 + width * font_size * scale;
        self.word.push_str(char);
        Ok(())
    }

    fn begin_word(&mut self) -> Result<(), OutputError> {
        self.flush_word();
        self.in_word = true;
        Ok(())
    }

    fn end_word(&mut self) -> Result<(), OutputError> {
        self.flush_word();
        Ok(())
    }

    fn end_line(&mut self) -> Result<(), OutputError> {
        self.flush_word();
        Ok(())
    }
}
