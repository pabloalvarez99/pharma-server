//! Thermal ticket printing (P0.4): build ESC/POS bytes from a POS receipt and
//! spool them RAW to a named Windows printer.
//!
//! The receipt payload mirrors the webview's `Receipt` wire type (money as
//! STRING, nullable tenders). The printer name is per-MACHINE (the printer is
//! plugged into one PC), so the webview passes it from localStorage — it is
//! NOT a tenant setting. When no printer is configured the webview keeps its
//! `window.print()` fallback; this command is never a hard dependency of a
//! sale.
//!
//! Barcode readers need NO code: keyboard-wedge scanners type into the
//! focused input and end with Enter — the POS picker already handles that.

use serde::Deserialize;
use tauri::State;

use crate::escpos::{drawer_kick_bytes, PaperWidth, ReceiptBuilder};
use crate::state::SessionState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptItemInput {
    name: String,
    qty: f64,
    unit_price: String,
    line_total: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptInput {
    tenant_name: String,
    folio_or_number: String,
    datetime: String,
    items: Vec<ReceiptItemInput>,
    discount: String,
    total: String,
    payment_method: String,
    cash_amount: Option<String>,
    card_amount: Option<String>,
    change: Option<String>,
    cashier: Option<String>,
    footer_note: String,
}

/// Build the ESC/POS byte stream for a receipt. Pure — spooling is separate
/// so the layout stays unit-testable on any OS.
pub fn build_ticket_bytes(r: &ReceiptInput, width: PaperWidth) -> Vec<u8> {
    let mut b = ReceiptBuilder::new(width);
    b.bold(&r.tenant_name)
        .center(&format!("Boleta N° {}", r.folio_or_number))
        .center(&r.datetime)
        .separator();
    for it in &r.items {
        let qty = if it.qty.fract() == 0.0 {
            format!("{}", it.qty as i64)
        } else {
            format!("{}", it.qty)
        };
        b.item(&it.name, &qty, &it.unit_price, &it.line_total);
    }
    b.separator();
    if r.discount != "0" && r.discount != "0.00" {
        b.row("Descuento", &format!("-{}", r.discount));
    }
    b.big(&format!("TOTAL {}", r.total));
    let tender = match r.payment_method.as_str() {
        "pos_cash" => "Efectivo",
        "pos_card" => "Tarjeta",
        "pos_mixed" => "Mixto",
        other => other,
    };
    b.row("Pago", tender);
    if let Some(cash) = &r.cash_amount {
        b.row("Recibido", cash);
    }
    if let Some(card) = &r.card_amount {
        b.row("Tarjeta", card);
    }
    if let Some(change) = &r.change {
        b.row("Vuelto", change);
    }
    if let Some(cashier) = &r.cashier {
        b.row("Atendió", cashier);
    }
    b.separator();
    if !r.footer_note.is_empty() {
        b.center(&r.footer_note);
    }
    b.cut();
    b.build()
}

/// Spool RAW bytes to a Windows printer via winspool (OpenPrinter →
/// StartDocPrinter(RAW) → WritePrinter → EndDocPrinter).
///
/// Uses `PRINTER_HANDLE` (windows 0.61+), not the older `HANDLE` alias —
/// winspool APIs no longer accept `HANDLE` for printer jobs.
#[cfg(windows)]
fn spool_raw(printer: &str, bytes: &[u8]) -> Result<(), String> {
    use std::ffi::c_void;
    use windows::core::PWSTR;
    use windows::Win32::Graphics::Printing::{
        ClosePrinter, EndDocPrinter, EndPagePrinter, OpenPrinterW, StartDocPrinterW,
        StartPagePrinter, WritePrinter, DOC_INFO_1W, PRINTER_HANDLE,
    };

    let mut name_utf16: Vec<u16> = printer.encode_utf16().chain(std::iter::once(0)).collect();
    let mut handle = PRINTER_HANDLE::default();
    // SAFETY: name_utf16 is null-terminated and lives for the OpenPrinter call;
    // handle is only used while open below.
    unsafe { OpenPrinterW(PWSTR(name_utf16.as_mut_ptr()), &mut handle, None) }
        .map_err(|e| format!("No se pudo abrir la impresora '{printer}': {e}"))?;

    let result = (|| {
        let mut doc_name: Vec<u16> = "RutBusiness ticket\0".encode_utf16().collect();
        let mut raw: Vec<u16> = "RAW\0".encode_utf16().collect();
        let doc = DOC_INFO_1W {
            pDocName: PWSTR(doc_name.as_mut_ptr()),
            pOutputFile: PWSTR::null(),
            pDatatype: PWSTR(raw.as_mut_ptr()),
        };
        // SAFETY: doc name/datatype buffers outlive StartDocPrinterW.
        let job = unsafe { StartDocPrinterW(handle, 1, &doc) };
        if job == 0 {
            return Err("StartDocPrinter falló".to_string());
        }
        unsafe {
            if !StartPagePrinter(handle).as_bool() {
                let _ = EndDocPrinter(handle);
                return Err("StartPagePrinter falló".to_string());
            }
            let mut written = 0u32;
            let ok = WritePrinter(
                handle,
                bytes.as_ptr() as *const c_void,
                bytes.len() as u32,
                &mut written,
            );
            if !ok.as_bool() {
                let _ = EndPagePrinter(handle);
                let _ = EndDocPrinter(handle);
                return Err("WritePrinter falló".to_string());
            }
            let _ = EndPagePrinter(handle);
            let _ = EndDocPrinter(handle);
        }
        Ok(())
    })();
    // SAFETY: handle was opened above; always close even if the job failed.
    unsafe {
        let _ = ClosePrinter(handle);
    }
    result
}

/// Print a POS receipt on the configured thermal printer.
/// `printer` = Windows printer name (localStorage `rb.thermalPrinter`).
/// `width58` = true for 58mm paper (32 cols), false for 80mm (48 cols).
/// `open_drawer` = when true, append ESC p kick after the cut (localStorage
/// `rb.openDrawer`, default off — most PCs have no cash drawer).
#[tauri::command]
pub fn print_ticket(
    printer: String,
    width58: bool,
    receipt: ReceiptInput,
    open_drawer: Option<bool>,
    _state: State<'_, SessionState>,
) -> Result<(), String> {
    let width = if width58 { PaperWidth::Mm58 } else { PaperWidth::Mm80 };
    let mut bytes = build_ticket_bytes(&receipt, width);
    if open_drawer.unwrap_or(false) {
        bytes.extend_from_slice(&drawer_kick_bytes());
    }
    #[cfg(windows)]
    {
        spool_raw(&printer, &bytes)
    }
    #[cfg(not(windows))]
    {
        let _ = (printer, bytes);
        Err("Impresión térmica solo disponible en Windows por ahora".to_string())
    }
}

/// Open the cash drawer via the thermal printer's kick pulse (ESC p).
/// Same printer name as tickets (`rb.thermalPrinter`). No-op surface in the
/// webview unless Preferencias has a printer configured.
#[tauri::command]
pub fn open_cash_drawer(
    printer: String,
    _state: State<'_, SessionState>,
) -> Result<(), String> {
    let bytes = drawer_kick_bytes();
    #[cfg(windows)]
    {
        spool_raw(&printer, &bytes)
    }
    #[cfg(not(windows))]
    {
        let _ = (printer, bytes);
        Err("Cajón de dinero solo disponible en Windows por ahora".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ReceiptInput {
        ReceiptInput {
            tenant_name: "Botillería El Rey".into(),
            folio_or_number: "152".into(),
            datetime: "18-07-2026 13:45".into(),
            items: vec![
                ReceiptItemInput {
                    name: "Pisco Capel 35° 700cc".into(),
                    qty: 1.0,
                    unit_price: "$8.990".into(),
                    line_total: "$8.990".into(),
                },
                ReceiptItemInput {
                    name: "Bebida 1.5L".into(),
                    qty: 2.0,
                    unit_price: "$2.000".into(),
                    line_total: "$4.000".into(),
                },
            ],
            discount: "0".into(),
            total: "$12.990".into(),
            payment_method: "pos_cash".into(),
            cash_amount: Some("$15.000".into()),
            card_amount: None,
            change: Some("$2.010".into()),
            cashier: Some("admin".into()),
            footer_note: "Gracias por su compra".into(),
        }
    }

    #[test]
    fn ticket_contains_header_items_and_total() {
        let bytes = build_ticket_bytes(&sample(), PaperWidth::Mm58);
        let text: String = bytes
            .iter()
            .map(|&b| if b.is_ascii_graphic() || b == b' ' || b == b'\n' { b as char } else { '.' })
            .collect();
        assert!(text.contains("Boleta N"));
        assert!(text.contains("Pisco Capel 35"));
        assert!(text.contains("2 x $2.000"));
        assert!(text.contains("TOTAL $12.990"));
        assert!(text.contains("Vuelto"));
    }

    #[test]
    fn discount_row_only_when_applies() {
        let mut r = sample();
        r.discount = "$1.000".into();
        let bytes = build_ticket_bytes(&r, PaperWidth::Mm80);
        assert!(bytes.windows(9).any(|w| w == b"Descuento"));
        let bytes = build_ticket_bytes(&sample(), PaperWidth::Mm80);
        assert!(!bytes.windows(9).any(|w| w == b"Descuento"));
    }

    #[test]
    fn open_drawer_appends_esc_p_after_ticket() {
        let base = build_ticket_bytes(&sample(), PaperWidth::Mm58);
        let mut with_kick = base.clone();
        with_kick.extend_from_slice(&drawer_kick_bytes());
        assert!(with_kick.len() > base.len());
        assert_eq!(
            &with_kick[with_kick.len() - 5..],
            &[0x1B, b'p', 0, 25, 250]
        );
    }
}
