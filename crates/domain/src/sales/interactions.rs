//! Drug-interaction checker — Beers criteria + Vademécum CL.
//!
//! Port of Tu Farmacia `apps/web/src/lib/drug-interactions.ts` (≈370 LoC of
//! curated pair rules). Same GROUPS (drug classes that share a profile) +
//! RULES (group-or-drug × group-or-drug, severity, optional exclusions),
//! expanded once into a static pair map.
//!
//! Caveat (mirrored from source): rules are referential, NEVER replace
//! pharmacist judgement. POS surfaces warnings; never blocks dispensing.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critica,
    Mayor,
    Moderada,
}

impl Severity {
    fn rank(self) -> u8 {
        match self {
            Self::Critica => 3,
            Self::Mayor => 2,
            Self::Moderada => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InteractionDetail {
    pub severity: Severity,
    /// Canonical alphabetical pair (UPPERCASE, no accents).
    pub drugs: (String, String),
    pub effect: String,
    pub recommendation: String,
}

struct Rule {
    a: &'static str,
    b: &'static str,
    severity: Severity,
    effect: &'static str,
    recommendation: &'static str,
    exclude: &'static [(&'static str, &'static str)],
}

const GROUPS: &[(&str, &[&str])] = &[
    (
        "AINE",
        &[
            "IBUPROFENO",
            "KETOPROFENO",
            "NAPROXENO",
            "DICLOFENACO",
            "PIROXICAM",
            "MELOXICAM",
            "ACIDO MEFENAMICO",
            "CELECOXIB",
            "ETORICOXIB",
            "NIMESULIDA",
            "KETOROLACO",
            "TENOXICAM",
            "INDOMETACINA",
        ],
    ),
    (
        "ANTICOAGULANTE",
        &[
            "WARFARINA",
            "ACENOCUMAROL",
            "RIVAROXABAN",
            "DABIGATRAN",
            "APIXABAN",
            "EDOXABAN",
        ],
    ),
    (
        "IBP",
        &[
            "OMEPRAZOL",
            "ESOMEPRAZOL",
            "LANSOPRAZOL",
            "PANTOPRAZOL",
            "RABEPRAZOL",
        ],
    ),
    (
        "BENZODIAZEPINA",
        &[
            "ALPRAZOLAM",
            "CLONAZEPAM",
            "DIAZEPAM",
            "LORAZEPAM",
            "BROMAZEPAM",
            "MIDAZOLAM",
        ],
    ),
    (
        "IECA",
        &["ENALAPRIL", "CAPTOPRIL", "LISINOPRIL", "RAMIPRIL"],
    ),
    (
        "ARA2",
        &[
            "LOSARTAN",
            "VALSARTAN",
            "IRBESARTAN",
            "CANDESARTAN",
            "TELMISARTAN",
        ],
    ),
    (
        "ISRS",
        &[
            "SERTRALINA",
            "FLUOXETINA",
            "PAROXETINA",
            "CITALOPRAM",
            "ESCITALOPRAM",
        ],
    ),
    (
        "NITRATO",
        &[
            "ISOSORBIDA",
            "DINITRATO DE ISOSORBIDA",
            "MONONITRATO DE ISOSORBIDA",
            "NITROGLICERINA",
        ],
    ),
    (
        "ESTATINA_3A4",
        &["ATORVASTATINA", "SIMVASTATINA", "LOVASTATINA"],
    ),
    ("MACROLIDO_3A4", &["CLARITROMICINA", "ERITROMICINA"]),
    ("PDE5", &["SILDENAFIL", "TADALAFIL", "VARDENAFIL"]),
    (
        "HIPOGLICEMIANTE",
        &[
            "GLIBENCLAMIDA",
            "GLICLAZIDA",
            "GLIMEPIRIDA",
            "METFORMINA",
            "INSULINA",
        ],
    ),
];

const RULES: &[Rule] = &[
    Rule {
        a: "ANTICOAGULANTE", b: "AINE", severity: Severity::Critica,
        effect: "Riesgo alto de sangrado mayor (gastrointestinal, intracraneal). El AINE inhibe la agregación plaquetaria, irrita la mucosa gástrica y desplaza al anticoagulante de sus proteínas plasmáticas.",
        recommendation: "Evite la combinación. Use paracetamol como analgésico de primera línea (máximo 3 g/día en adulto mayor).",
        exclude: &[],
    },
    Rule {
        a: "WARFARINA", b: "PARACETAMOL", severity: Severity::Moderada,
        effect: "Dosis altas o uso prolongado (>2 g/día por varios días) pueden aumentar el INR.",
        recommendation: "Limite a dosis ocasionales (≤2 g/día). Controle INR si necesita uso regular.",
        exclude: &[],
    },
    Rule {
        a: "WARFARINA", b: "AMIODARONA", severity: Severity::Mayor,
        effect: "Aumento marcado del INR y riesgo de sangrado.",
        recommendation: "Reduzca la dosis de warfarina en 30-50%. Control de INR semanal hasta estabilizar.",
        exclude: &[],
    },
    Rule {
        a: "WARFARINA", b: "CIPROFLOXACINO", severity: Severity::Mayor,
        effect: "Aumento del INR por inhibición del metabolismo hepático.",
        recommendation: "Control de INR a los 3-5 días de iniciar el antibiótico.",
        exclude: &[],
    },
    Rule {
        a: "WARFARINA", b: "METRONIDAZOL", severity: Severity::Mayor,
        effect: "Aumento marcado del INR.",
        recommendation: "Control estrecho de INR durante y hasta 1 semana después del tratamiento.",
        exclude: &[],
    },
    Rule {
        a: "CLOPIDOGREL", b: "IBP", severity: Severity::Mayor,
        effect: "Reducción del efecto antiplaquetario del clopidogrel por inhibición de CYP2C19, aumentando el riesgo de eventos cardiovasculares.",
        recommendation: "Prefiera pantoprazol o famotidina si necesita protección gástrica.",
        exclude: &[("CLOPIDOGREL", "PANTOPRAZOL")],
    },
    Rule {
        a: "LEVOTIROXINA", b: "IBP", severity: Severity::Moderada,
        effect: "Menor absorción de levotiroxina por aumento del pH gástrico.",
        recommendation: "Tome la levotiroxina en ayunas y separe al menos 4 horas del IBP.",
        exclude: &[],
    },
    Rule {
        a: "LEVOTIROXINA", b: "CARBONATO DE CALCIO", severity: Severity::Moderada,
        effect: "El calcio reduce significativamente la absorción de levotiroxina.",
        recommendation: "Separe las tomas al menos 4 horas.",
        exclude: &[],
    },
    Rule {
        a: "LEVOTIROXINA", b: "SULFATO FERROSO", severity: Severity::Moderada,
        effect: "El hierro reduce la absorción de levotiroxina.",
        recommendation: "Separe las tomas al menos 4 horas.",
        exclude: &[],
    },
    Rule {
        a: "ESTATINA_3A4", b: "MACROLIDO_3A4", severity: Severity::Mayor,
        effect: "Riesgo aumentado de miopatía y rabdomiolisis (CYP3A4 ↑ niveles de estatina).",
        recommendation: "Suspenda la estatina durante el tratamiento antibiótico, o use azitromicina.",
        exclude: &[],
    },
    Rule {
        a: "SIMVASTATINA", b: "CLARITROMICINA", severity: Severity::Critica,
        effect: "Riesgo grave de rabdomiolisis (combinación contraindicada).",
        recommendation: "No combinar. Reemplace claritromicina por azitromicina o suspenda simvastatina temporalmente.",
        exclude: &[],
    },
    Rule {
        a: "IECA", b: "ESPIRONOLACTONA", severity: Severity::Mayor,
        effect: "Riesgo de hiperpotasemia (K sanguíneo elevado, arritmias).",
        recommendation: "Control de potasio cada 1-3 meses. Evite suplementos de potasio y sustitutos de sal con K.",
        exclude: &[],
    },
    Rule {
        a: "ARA2", b: "ESPIRONOLACTONA", severity: Severity::Mayor,
        effect: "Hiperpotasemia.",
        recommendation: "Control de potasio. Evite suplementos de potasio.",
        exclude: &[],
    },
    Rule {
        a: "IECA", b: "AINE", severity: Severity::Mayor,
        effect: "Reducción del efecto antihipertensivo y riesgo de daño renal agudo, sobre todo en adulto mayor.",
        recommendation: "Evite AINEs de uso crónico. Use paracetamol.",
        exclude: &[],
    },
    Rule {
        a: "ARA2", b: "AINE", severity: Severity::Mayor,
        effect: "Reducción del efecto antihipertensivo + nefrotoxicidad.",
        recommendation: "Evite AINEs de uso crónico.",
        exclude: &[],
    },
    Rule {
        a: "LITIO", b: "AINE", severity: Severity::Mayor,
        effect: "Aumento de niveles séricos de litio (toxicidad: temblor, confusión, ataxia).",
        recommendation: "Evite AINEs crónicos. Si imprescindible, control de litemia frecuente.",
        exclude: &[],
    },
    Rule {
        a: "LITIO", b: "IECA", severity: Severity::Mayor,
        effect: "Aumento de niveles séricos de litio.",
        recommendation: "Control de litemia al inicio y tras cambios de dosis.",
        exclude: &[],
    },
    Rule {
        a: "ISRS", b: "TRAMADOL", severity: Severity::Mayor,
        effect: "Riesgo de síndrome serotoninérgico (agitación, sudoración, hipertermia, hiperreflexia).",
        recommendation: "Evitar la combinación. Si imprescindible, vigilancia estrecha y dosis baja de tramadol.",
        exclude: &[],
    },
    Rule {
        a: "ISRS", b: "AINE", severity: Severity::Moderada,
        effect: "Aumento del riesgo de sangrado gastrointestinal, especialmente en adulto mayor.",
        recommendation: "Considere protección gástrica (pantoprazol/famotidina) o prefiera paracetamol.",
        exclude: &[],
    },
    Rule {
        a: "DIGOXINA", b: "AMIODARONA", severity: Severity::Mayor,
        effect: "Aumento de niveles de digoxina con toxicidad (náuseas, arritmias, alteraciones visuales).",
        recommendation: "Reduzca la dosis de digoxina ~50%. Control de digoxinemia y ECG.",
        exclude: &[],
    },
    Rule {
        a: "DIGOXINA", b: "FUROSEMIDA", severity: Severity::Moderada,
        effect: "La hipopotasemia inducida por furosemida potencia la toxicidad digital.",
        recommendation: "Controle potasio y magnesio. Suplemente si es necesario.",
        exclude: &[],
    },
    Rule {
        a: "DIGOXINA", b: "CLARITROMICINA", severity: Severity::Mayor,
        effect: "Aumento significativo de niveles de digoxina.",
        recommendation: "Control de digoxinemia. Reducir dosis si es necesario.",
        exclude: &[],
    },
    Rule {
        a: "METOTREXATO", b: "AINE", severity: Severity::Critica,
        effect: "Toxicidad grave del metotrexato (mielosupresión, hepatotoxicidad, mucositis).",
        recommendation: "Evite la combinación. Use paracetamol como analgésico.",
        exclude: &[],
    },
    Rule {
        a: "METOTREXATO", b: "COTRIMOXAZOL", severity: Severity::Critica,
        effect: "Aumento de toxicidad hematológica (efecto antifolato sumado).",
        recommendation: "Evite. Use otro antibiótico.",
        exclude: &[],
    },
    Rule {
        a: "PDE5", b: "NITRATO", severity: Severity::Critica,
        effect: "Hipotensión severa potencialmente mortal.",
        recommendation: "COMBINACIÓN CONTRAINDICADA. No use sildenafil/tadalafil/vardenafil con ningún nitrato.",
        exclude: &[],
    },
    Rule {
        a: "METFORMINA", b: "CONTRASTE YODADO", severity: Severity::Mayor,
        effect: "Riesgo de acidosis láctica si hay insuficiencia renal.",
        recommendation: "Suspenda metformina 48 horas antes y después del contraste. Reanude tras función renal normal.",
        exclude: &[],
    },
    Rule {
        a: "BENZODIAZEPINA", b: "BENZODIAZEPINA", severity: Severity::Mayor,
        effect: "Sedación excesiva, depresión respiratoria, deterioro cognitivo y riesgo alto de caídas (criterios Beers).",
        recommendation: "No combine dos benzodiazepinas en adulto mayor. Considere alternativas no benzodiazepínicas.",
        exclude: &[],
    },
    Rule {
        a: "BENZODIAZEPINA", b: "TRAMADOL", severity: Severity::Mayor,
        effect: "Depresión del sistema nervioso central y respiratoria.",
        recommendation: "Evite en adulto mayor. Si imprescindible, dosis bajas y vigilancia.",
        exclude: &[],
    },
    Rule {
        a: "BENZODIAZEPINA", b: "CODEINA", severity: Severity::Mayor,
        effect: "Depresión respiratoria, especialmente en adulto mayor o EPOC.",
        recommendation: "Evite la combinación.",
        exclude: &[],
    },
    Rule {
        a: "BENZODIAZEPINA", b: "ZOPICLONA", severity: Severity::Mayor,
        effect: "Sedación profunda + caídas.",
        recommendation: "No usar juntos.",
        exclude: &[],
    },
    Rule {
        a: "HIPOGLICEMIANTE", b: "PROPRANOLOL", severity: Severity::Moderada,
        effect: "El betabloqueador puede enmascarar síntomas de hipoglicemia (taquicardia, temblor).",
        recommendation: "Vigile glicemia. Prefiera bisoprolol/atenolol si requiere betabloqueador.",
        exclude: &[],
    },
    Rule {
        a: "AMITRIPTILINA", b: "OXIBUTININA", severity: Severity::Mayor,
        effect: "Carga anticolinérgica sumada: confusión, retención urinaria, constipación, caídas (Beers).",
        recommendation: "Evite combinación en adulto mayor.",
        exclude: &[],
    },
];

fn pair_key(a: &str, b: &str) -> (String, String) {
    if a < b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

fn group_members(name: &str) -> Option<&'static [&'static str]> {
    GROUPS.iter().find(|(g, _)| *g == name).map(|(_, m)| *m)
}

type PairMap = HashMap<(String, String), InteractionDetail>;

fn pair_map() -> &'static PairMap {
    static MAP: OnceLock<PairMap> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = PairMap::new();
        for rule in RULES {
            let single_a = [rule.a];
            let single_b = [rule.b];
            let as_: &[&str] = group_members(rule.a).unwrap_or(&single_a);
            let bs_: &[&str] = group_members(rule.b).unwrap_or(&single_b);
            for &a in as_ {
                for &b in bs_ {
                    if a == b && rule.a != rule.b {
                        // Self pair from cross-group expansion: skip.
                        continue;
                    }
                    if a == b {
                        // BENZODIAZEPINA × BENZODIAZEPINA: skip identical drug pair.
                        continue;
                    }
                    let k = pair_key(a, b);
                    if rule.exclude.iter().any(|(x, y)| pair_key(x, y) == k) {
                        continue;
                    }
                    if let Some(existing) = map.get(&k) {
                        if existing.severity.rank() >= rule.severity.rank() {
                            continue;
                        }
                    }
                    let (a_, b_) = if a < b { (a, b) } else { (b, a) };
                    map.insert(
                        k,
                        InteractionDetail {
                            severity: rule.severity,
                            drugs: (a_.to_string(), b_.to_string()),
                            effect: rule.effect.to_string(),
                            recommendation: rule.recommendation.to_string(),
                        },
                    );
                }
            }
        }
        map
    })
}

fn deaccent_upper(s: &str) -> String {
    s.to_uppercase()
        .replace('Á', "A")
        .replace('É', "E")
        .replace('Í', "I")
        .replace('Ó', "O")
        .replace('Ú', "U")
        .replace('Ñ', "N")
}

fn known_drugs() -> &'static Vec<&'static str> {
    static D: OnceLock<Vec<&'static str>> = OnceLock::new();
    D.get_or_init(|| {
        let group_keys: HashSet<&str> = GROUPS.iter().map(|(g, _)| *g).collect();
        let mut names: HashSet<&'static str> = HashSet::new();
        for (_, members) in GROUPS {
            for &m in *members {
                names.insert(m);
            }
        }
        for r in RULES {
            if !group_keys.contains(r.a) {
                names.insert(r.a);
            }
            if !group_keys.contains(r.b) {
                names.insert(r.b);
            }
            for (x, y) in r.exclude {
                names.insert(x);
                names.insert(y);
            }
        }
        let mut v: Vec<&'static str> = names.into_iter().collect();
        // Longest first so "ACIDO MEFENAMICO" wins over a hypothetical "ACIDO".
        v.sort_by_key(|s| std::cmp::Reverse(s.len()));
        v
    })
}

/// Tokenize an active-ingredient string into ruleset drug names. Matches the
/// uppercased, accent-stripped string against the full set of drug names in
/// GROUPS + RULES (longest-first to handle multi-word names like "ACIDO
/// MEFENAMICO" or "MONONITRATO DE ISOSORBIDA").
fn tokenize(ingredient: &str) -> Vec<&'static str> {
    if ingredient.trim().is_empty() {
        return Vec::new();
    }
    let mut buf = deaccent_upper(ingredient);
    let mut out = Vec::new();
    // Greedy longest-first: when a name matches, blank out its span so a
    // shorter substring of it (e.g. "ISOSORBIDA" inside "MONONITRATO DE
    // ISOSORBIDA") doesn't double-match.
    for &name in known_drugs() {
        while let Some(pos) = buf.find(name) {
            out.push(name);
            let span = " ".repeat(name.len());
            buf.replace_range(pos..pos + name.len(), &span);
        }
    }
    out
}

/// Check active-ingredient strings from a cart for known interactions.
/// Returns matched pairs sorted by severity descending (`Critica` first).
pub fn check<S: AsRef<str>>(active_ingredients: &[S]) -> Vec<InteractionDetail> {
    let mut drugs: HashSet<&'static str> = HashSet::new();
    for ing in active_ingredients {
        for t in tokenize(ing.as_ref()) {
            drugs.insert(t);
        }
    }
    let arr: Vec<&'static str> = drugs.into_iter().collect();
    let map = pair_map();
    let mut out = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for i in 0..arr.len() {
        for j in (i + 1)..arr.len() {
            let k = pair_key(arr[i], arr[j]);
            if seen.contains(&k) {
                continue;
            }
            if let Some(hit) = map.get(&k) {
                out.push(hit.clone());
                seen.insert(k);
            }
        }
    }
    out.sort_by_key(|d| std::cmp::Reverse(d.severity.rank()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pde5_plus_nitrato_is_critica() {
        let v = check(&["sildenafil 50 mg", "Mononitrato de isosorbida"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].severity, Severity::Critica);
    }

    #[test]
    fn anticoagulante_plus_aine_is_critica() {
        let v = check(&["WARFARINA 5 mg", "ibuprofeno 400"]);
        assert_eq!(v[0].severity, Severity::Critica);
    }

    #[test]
    fn clopidogrel_plus_pantoprazol_is_excluded() {
        // Specific exclude (CLOPIDOGREL+PANTOPRAZOL) overrides group rule.
        let v = check(&["clopidogrel", "pantoprazol"]);
        assert!(v.is_empty());
        // Other IBPs still trigger Mayor.
        let v = check(&["clopidogrel", "omeprazol"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].severity, Severity::Mayor);
    }

    #[test]
    fn simvastatina_plus_claritromicina_overrides_group_rule_to_critica() {
        // ESTATINA_3A4 × MACROLIDO_3A4 = Mayor; specific pair = Critica.
        let v = check(&["simvastatina", "claritromicina"]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].severity, Severity::Critica);
    }

    #[test]
    fn sorted_by_severity_descending() {
        let v = check(&[
            "warfarina",
            "paracetamol", // Moderada with warfarina
            "amiodarona",  // Mayor with warfarina
            "ibuprofeno",  // Critica with warfarina
        ]);
        assert!(v.len() >= 3);
        assert_eq!(v[0].severity, Severity::Critica);
        for w in v.windows(2) {
            assert!(w[0].severity.rank() >= w[1].severity.rank());
        }
    }

    #[test]
    fn empty_or_unknown_returns_empty() {
        let empty: [&str; 0] = [];
        let v: Vec<InteractionDetail> = check(&empty);
        assert!(v.is_empty());
        let v = check(&["", "drug-not-in-ruleset"]);
        assert!(v.is_empty());
    }
}
