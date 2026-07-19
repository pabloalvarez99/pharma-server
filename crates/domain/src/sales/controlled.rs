//! Controlled-substance lookup (Chile · Decreto 404 + Ley 20.000).
//!
//! Ported literally from Tu Farmacia `apps/web/src/lib/controlled-substances.ts`.
//! Used by [`super::service`] to flag prescription rows as `controlled=true`
//! whenever the dispensed product's `active_ingredient` matches.
//!
//! Reference set covers benzodiazepines, opioids, stimulants, barbiturates and
//! dissociatives currently scheduled by ISP. Update as Decreto 404 evolves.

const CONTROLLED: &[&str] = &[
    // Benzodiazepines
    "clonazepam",
    "alprazolam",
    "lorazepam",
    "diazepam",
    "bromazepam",
    "midazolam",
    "nitrazepam",
    "flunitrazepam",
    "triazolam",
    // Opioids
    "morfina",
    "codeina",
    "codeína",
    "tramadol",
    "oxicodona",
    "hidromorfona",
    "fentanilo",
    "buprenorfina",
    "metadona",
    "meperidina",
    // Stimulants
    "metilfenidato",
    "anfetamina",
    "modafinilo",
    // Barbiturates
    "fenobarbital",
    "butalbital",
    // Dissociatives
    "ketamina",
    "dronabinol",
];

/// Returns true if `active_ingredient` (case-insensitive substring match)
/// contains any controlled substance from Decreto 404 / Ley 20.000.
pub fn is_controlled(active_ingredient: Option<&str>) -> bool {
    let Some(ai) = active_ingredient else {
        return false;
    };
    let lower = ai.to_lowercase();
    CONTROLLED.iter().any(|s| lower.contains(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_controlled() {
        assert!(is_controlled(Some("Clonazepam 2mg")));
        assert!(is_controlled(Some("TRAMADOL/PARACETAMOL")));
        assert!(is_controlled(Some("metilfenidato base")));
    }

    #[test]
    fn ignores_uncontrolled() {
        assert!(!is_controlled(Some("paracetamol")));
        assert!(!is_controlled(Some("ibuprofeno 400mg")));
        assert!(!is_controlled(None));
        assert!(!is_controlled(Some("")));
    }
}
