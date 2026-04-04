use syn::{spanned::Spanned, Attribute, Error, Lit, LitStr, Meta, NestedMeta};

pub fn visit_attribs(
    attrs: &[Attribute],
    ident: &str,
    mut cb: impl FnMut(&NestedMeta) -> syn::Result<()>,
) -> syn::Result<()> {
    for mv in attrs
        .iter()
        .filter(|attr| attr.path.is_ident(ident))
        .map(|attr| match attr.parse_meta() {
            Ok(Meta::List(meta)) => Ok(meta.nested.into_iter().collect::<Vec<_>>()),
            Ok(other) => Err(Error::new(
                other.span(),
                format!("expected #[{ident}(...)]"),
            )),
            Err(err) => Err(err),
        })
    {
        for mv in mv? {
            cb(&mv)?;
        }
    }

    Ok(())
}

pub fn check_is_disabled(attrs: &[Attribute]) -> bool {
    fn cfg_env_name(key: &str) -> String {
        format!("CARGO_CFG_{}", key.to_uppercase().replace('-', "_"))
    }

    fn check_expr(meta: &NestedMeta, lookup: &impl Fn(&str) -> Option<String>) -> bool {
        match meta {
            NestedMeta::Meta(Meta::NameValue(m)) if m.path.is_ident("feature") => {
                let Ok(feature) = get_lit_str(&m.lit) else {
                    return false;
                };
                let envname = format!(
                    "CARGO_FEATURE_{}",
                    feature.value().to_uppercase().replace('-', "_")
                );
                lookup(&envname).is_some()
            }
            NestedMeta::Meta(Meta::NameValue(m)) => {
                let Some(key) = m.path.get_ident().map(ToString::to_string) else {
                    return false;
                };
                let Ok(expected) = get_lit_str(&m.lit) else {
                    return false;
                };
                let envname = cfg_env_name(&key);
                let Some(actual) = lookup(&envname) else {
                    return false;
                };
                if key == "target_feature" {
                    actual.split(',').any(|f| f == expected.value())
                } else {
                    actual == expected.value()
                }
            }
            NestedMeta::Meta(Meta::Path(path)) => {
                let Some(key) = path.get_ident().map(ToString::to_string) else {
                    return false;
                };
                let envname = cfg_env_name(&key);
                lookup(&envname).is_some()
            }
            NestedMeta::Meta(Meta::List(m)) if m.path.is_ident("not") => {
                let sub = m.nested.first().is_some_and(|n| check_expr(n, lookup));
                !sub
            }
            NestedMeta::Meta(Meta::List(m)) if m.path.is_ident("all") => {
                m.nested.iter().all(|n| check_expr(n, lookup))
            }
            NestedMeta::Meta(Meta::List(m)) if m.path.is_ident("any") => {
                m.nested.iter().any(|n| check_expr(n, lookup))
            }
            // Unknown cfg predicate/function: treat as not enabled.
            _ => false,
        }
    }

    let lookup = |k: &str| std::env::var(k).ok();
    let mut v = true;
    // Invalid cfg syntax: conservatively disable the item.
    if visit_attribs(attrs, "cfg", |m| {
        if v && !check_expr(m, &lookup) {
            v = false;
        }
        Ok(())
    })
    .is_err()
    {
        return true;
    }
    !v
}

pub fn check_is_enabled(attrs: &[Attribute]) -> bool {
    !check_is_disabled(attrs)
}

pub fn get_lit_str(lit: &Lit) -> syn::Result<&LitStr> {
    if let Lit::Str(s) = lit {
        Ok(s)
    } else {
        Err(Error::new(lit.span(), "expected attribute to be a string"))
    }
}

#[cfg(test)]
mod tests {
    use super::check_is_disabled;
    use syn::parse_quote;

    #[test]
    fn no_cfg_attributes_is_not_disabled() {
        let attrs = vec![];
        assert!(!check_is_disabled(&attrs));
    }

    #[test]
    fn unknown_cfg_predicate_is_disabled() {
        let attrs = vec![parse_quote!(#[cfg(made_up_predicate = "x")])];
        assert!(check_is_disabled(&attrs));
    }

    #[test]
    fn unknown_cfg_function_is_disabled() {
        let attrs = vec![parse_quote!(#[cfg(made_up(magic))])];
        assert!(check_is_disabled(&attrs));
    }

    #[test]
    fn not_of_unknown_predicate_is_not_disabled() {
        let attrs = vec![parse_quote!(#[cfg(not(made_up_predicate = "x"))])];
        assert!(!check_is_disabled(&attrs));
    }
}
