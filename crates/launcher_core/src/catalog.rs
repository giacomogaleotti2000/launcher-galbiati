//! Le app che il launcher sa installare. Per aggiungerne una basta un elemento in [`catalog`].

/// Da dove arrivano i rilasci di un'app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceSpec {
  /// Release di un repository GitHub (privato): tag `<tag_prefix><versione>`.
  GitHub { owner: String, repo: String, tag_prefix: String },
  /// Cartella condivisa con una sottocartella per versione e `manifest.json` (non ancora usata).
  NetworkShare { root: String },
}

/// Un'app del catalogo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppSpec {
  pub id: String,
  pub name: String,
  pub description: String,
  pub source: SourceSpec,
}

/// Catalogo delle app.
pub fn catalog() -> Vec<AppSpec> {
  vec![AppSpec {
    id: "heidenhain-siemens-converter".to_owned(),
    name: "HEIDENHAIN-SIEMENS CONVERTER".to_owned(),
    description: "Conversione Heidenhain Klartext (.H) -> Siemens Sinumerik (.MPF)".to_owned(),
    source: SourceSpec::GitHub {
      owner: "giacomogaleotti2000".to_owned(),
      repo: "HEIDEN-SIEMENS".to_owned(),
      tag_prefix: "converter-v".to_owned(),
    },
  }]
}
