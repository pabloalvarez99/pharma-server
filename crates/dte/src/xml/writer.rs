//! Helper común para serializar structs `schema::*` a string XML estable.
//!
//! - Prepende `<?xml version="1.0" encoding="UTF-8"?>` (SII xsd 1.0 admite
//!   UTF-8 declarado; legacy boleta ISO-8859-1 se trata en 9.1.l si SII lo
//!   exige al validar contra sandbox).
//! - Sin pretty-print: el TED firma sobre bytes canónicos, así que el output
//!   no debe variar entre llenadas. Reduce surface de bugs por whitespace.

use crate::DteError;
use serde::Serialize;

const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;

/// Serializa `val` con quick-xml y antepone declaración XML. Sin indentación
/// (output canónico).
pub(crate) fn to_xml_string<T: Serialize>(val: &T) -> Result<String, DteError> {
    let mut buf = String::new();
    let ser = quick_xml::se::Serializer::new(&mut buf);
    val.serialize(ser)
        .map_err(|e| DteError::XmlInvalid(format!("serializar XML: {e}")))?;
    Ok(format!("{XML_DECL}{buf}"))
}
