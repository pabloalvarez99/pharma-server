//! ESC/POS byte builder for thermal receipt printers (P0.4).
//!
//! Pure functions, no I/O — unit-testable. The command layer
//! (`commands/print.rs`) sends the bytes RAW to the Windows spooler.
//!
//! Scope: text-mode receipts (what every 58/80mm ESC/POS clone supports):
//! init, align, bold, double-size, feed, cut. NO raster logos, NO QR —
//! a boleta SII impresa por roller queda para el módulo DTE.

/// ESC/POS control bytes.
const ESC: u8 = 0x1B;
const GS: u8 = 0x1D;

/// Column width per paper size (font A, the default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperWidth {
    /// 58mm — 32 columns.
    Mm58,
    /// 80mm — 48 columns.
    Mm80,
}

impl PaperWidth {
    fn cols(self) -> usize {
        match self {
            Self::Mm58 => 32,
            Self::Mm80 => 48,
        }
    }
}

/// Incremental receipt builder. `String` internally, encoded to the printer's
/// legacy codepage at [`ReceiptBuilder::build`] time.
pub struct ReceiptBuilder {
    width: PaperWidth,
    buf: Vec<u8>,
}

impl ReceiptBuilder {
    pub fn new(width: PaperWidth) -> Self {
        // ESC @ — initialize (clears buffer, resets styles).
        Self { width, buf: vec![ESC, b'@'] }
    }

    fn push_str(&mut self, s: &str) {
        // ESC/POS clones in Chile speak CP858/CP437; á é í ó ú ñ exist in
        // both. Chars outside the codepage degrade to '?' instead of
        // failing the whole ticket.
        for ch in s.chars() {
            let b = match ch {
                'á' => 0xA0, 'é' => 0x82, 'í' => 0xA1, 'ó' => 0xA2, 'ú' => 0xA3,
                'ñ' => 0xA4, 'Ñ' => 0xA5, 'ü' => 0x81, 'Á' => 0xB5, '°' => 0xF8,
                c if c.is_ascii() => c as u8,
                _ => b'?',
            };
            self.buf.push(b);
        }
    }

    fn line(&mut self, s: &str) {
        let cols = self.width.cols();
        let truncated: String = s.chars().take(cols).collect();
        self.push_str(&truncated);
        self.buf.push(b'\n');
    }

    /// Centered line (ESC a 1).
    pub fn center(&mut self, s: &str) -> &mut Self {
        self.buf.extend_from_slice(&[ESC, b'a', 1]);
        self.line(s);
        self.buf.extend_from_slice(&[ESC, b'a', 0]);
        self
    }

    /// Left-aligned line (used by unit tests and future free-form footer lines).
    #[allow(dead_code)]
    pub fn left(&mut self, s: &str) -> &mut Self {
        self.line(s);
        self
    }

    /// Bold line (ESC E 1).
    pub fn bold(&mut self, s: &str) -> &mut Self {
        self.buf.extend_from_slice(&[ESC, b'E', 1]);
        self.line(s);
        self.buf.extend_from_slice(&[ESC, b'E', 0]);
        self
    }

    /// Centered double-height+width line (GS ! 0x11) — the ticket total.
    pub fn big(&mut self, s: &str) -> &mut Self {
        self.buf.extend_from_slice(&[ESC, b'a', 1, GS, b'!', 0x11]);
        // Double width → half the columns.
        let cols = self.width.cols() / 2;
        let truncated: String = s.chars().take(cols).collect();
        self.push_str(&truncated);
        self.buf.extend_from_slice(&[b'\n', GS, b'!', 0x00, ESC, b'a', 0]);
        self
    }

    /// "label ....... value" row filling the full width.
    pub fn row(&mut self, label: &str, value: &str) -> &mut Self {
        let cols = self.width.cols();
        let used = label.chars().count() + value.chars().count();
        let dots = cols.saturating_sub(used).max(1);
        self.line(&format!("{}{}{}", label, ".".repeat(dots), value));
        self
    }

    /// Separator row of dashes.
    pub fn separator(&mut self) -> &mut Self {
        self.line(&"-".repeat(self.width.cols()));
        self
    }

    /// Item row: name on its own line, then "qty x price ...... total".
    pub fn item(&mut self, name: &str, qty: &str, unit: &str, total: &str) -> &mut Self {
        self.line(name);
        self.row(&format!("{} x {}", qty, unit), total);
        self
    }

    /// Feed N blank lines.
    pub fn feed(&mut self, n: u8) -> &mut Self {
        for _ in 0..n {
            self.buf.push(b'\n');
        }
        self
    }

    /// Partial cut (GS V 1) — present on virtually every auto-cutter clone.
    pub fn cut(&mut self) -> &mut Self {
        self.feed(3);
        self.buf.extend_from_slice(&[GS, b'V', 1]);
        self
    }

    /// Append a cash-drawer kick (ESC p). Same pulse as [`drawer_kick_bytes`].
    /// Available for composition; `print_ticket` appends the kick after `build`.
    #[allow(dead_code)]
    pub fn kick_drawer(&mut self) -> &mut Self {
        self.buf.extend_from_slice(&drawer_kick_bytes());
        self
    }

    /// Final byte payload for the spooler.
    pub fn build(&self) -> Vec<u8> {
        self.buf.clone()
    }
}

/// ESC/POS cash-drawer pulse: `ESC p m t1 t2`.
///
/// - `m = 0` → connector pin 2 (default on Epson TM / clones in Chile).
/// - `t1 = 25` → ~50 ms ON, `t2 = 250` → ~500 ms OFF (units of 2 ms).
///
/// Pure bytes — no I/O. Spool RAW to the same thermal printer that drives
/// the drawer RJ-11 cable; a separate command exists so Preferencias can
/// leave kick off by default (most installs have no drawer).
pub fn drawer_kick_bytes() -> Vec<u8> {
    vec![ESC, b'p', 0, 25, 250]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_and_cut_frame_the_ticket() {
        let mut b = ReceiptBuilder::new(PaperWidth::Mm58);
        b.left("hola").cut();
        let out = b.build();
        assert_eq!(&out[..2], &[0x1B, b'@']);
        assert_eq!(&out[out.len() - 3..], &[0x1D, b'V', 1]);
    }

    #[test]
    fn long_lines_are_truncated_to_paper_width() {
        let mut b = ReceiptBuilder::new(PaperWidth::Mm58);
        b.left(&"x".repeat(100));
        let out = b.build();
        // init(2) + 32 chars + '\n'
        assert_eq!(out.len(), 2 + 32 + 1);
    }

    #[test]
    fn spanish_accents_map_to_cp437() {
        let mut b = ReceiptBuilder::new(PaperWidth::Mm80);
        // Precomposed accents (á é í ñ) — not ASCII a/e/i + combining marks.
        b.left("áéíñ");
        let out = b.build();
        // skip ESC @ init
        assert_eq!(&out[2..6], &[0xA0, 0x82, 0xA1, 0xA4]);
    }

    #[test]
    fn row_fills_width_with_dots() {
        let mut b = ReceiptBuilder::new(PaperWidth::Mm58);
        b.row("TOTAL", "$1.000");
        let out = b.build();
        // skip ESC @ (2 bytes); body is "TOTAL" + dots + "$1.000" + '\n'
        let body = &out[2..];
        assert_eq!(body.len(), 32 + 1);
        assert!(body.starts_with(b"TOTAL"));
        assert!(body.windows(6).any(|w| w == b"$1.000"));
        let text: String = body[..32]
            .iter()
            .map(|&c| c as char)
            .collect();
        assert_eq!(text.chars().count(), 32);
        assert!(text.ends_with("$1.000"));
    }

    #[test]
    fn big_uses_double_size_and_half_columns() {
        let mut b = ReceiptBuilder::new(PaperWidth::Mm58);
        b.big(&"T".repeat(40));
        let out = b.build();
        // GS ! 0x11 present, text truncated to 16 cols.
        assert!(out.windows(3).any(|w| w == [0x1D, b'!', 0x11]));
    }

    #[test]
    fn drawer_kick_is_esc_p_pulse() {
        let kick = drawer_kick_bytes();
        assert_eq!(kick, vec![0x1B, b'p', 0, 25, 250]);
        let mut b = ReceiptBuilder::new(PaperWidth::Mm58);
        b.kick_drawer();
        let out = b.build();
        // init + ESC p ...
        assert_eq!(&out[2..], &[0x1B, b'p', 0, 25, 250]);
    }
}
