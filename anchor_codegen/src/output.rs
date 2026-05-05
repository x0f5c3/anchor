use std::collections::BTreeMap;

use crate::static_string::HexName;
use quote::format_ident;
use syn::{parse::Parse, token::Comma, Error, Expr, Ident, LitStr, Type};

#[derive(Debug, Eq, PartialEq)]
pub struct Output {
    pub id: Option<u16>,
    pub format: String,
    pub args: Vec<Arg>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Arg {
    pub type_: Type,
    pub value: Option<Expr>,
}

impl Output {
    pub fn sender_fn_name(&self) -> Ident {
        format_ident!("send_output_{}", HexName(&self.format, false))
    }

    pub fn clear_arg_values(&mut self) {
        for arg in self.args.iter_mut() {
            arg.value = None;
        }
    }
}

lazy_static::lazy_static! {
    static ref TYPE_MAP: BTreeMap<&'static str, &'static str> = BTreeMap::from([
        ("u", "u32"),
        ("i", "i32"),
        ("hu", "u16"),
        ("hi", "i16"),
        ("c", "u8"),
        (".*s", "&[u8]"),
        ("*s", "&str"),
    ]);
    static ref TYPE_KEYS_BY_LEN_DESC: Vec<&'static str> = {
        let mut keys: Vec<&'static str> = TYPE_MAP.keys().copied().collect();
        keys.sort_by_key(|k| core::cmp::Reverse(k.len()));
        keys
    };
}

fn parse_args(mut fmt: &str) -> syn::Result<Vec<Arg>> {
    let mut args = vec![];
    while !fmt.is_empty() {
        let Some(pos) = fmt.find('%') else {
            break;
        };
        fmt = &fmt[pos + 1..];

        // Escaped percent "%%" emits a literal '%' and consumes no argument.
        if let Some(rem) = fmt.strip_prefix('%') {
            fmt = rem;
            continue;
        }

        // Longest-first matching for ambiguous prefixes (*s vs .*s, h*).
        let mut matched = false;
        for &kind in TYPE_KEYS_BY_LEN_DESC.iter() {
            if let Some(rem) = fmt.strip_prefix(kind) {
                let type_ = syn::parse_str(TYPE_MAP.get(kind).expect("missing type map entry"))
                    .expect("invalid Rust type in TYPE_MAP");
                args.push(Arg { type_, value: None });
                fmt = rem;
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        return Err(Error::new(
            proc_macro2::Span::call_site(),
            format!("unknown klipper_output format specifier near '%{}'", fmt),
        ));
    }
    Ok(args)
}

impl Parse for Output {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let format = input.parse::<LitStr>()?.value();
        let mut args = parse_args(&format)?;

        for arg in args.iter_mut() {
            input.parse::<Comma>()?;
            arg.value = Some(input.parse()?);
        }

        if !input.is_empty() {
            Err(input.error("Unexpected extra arguments"))
        } else {
            Ok(Output {
                id: None,
                format,
                args,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Output, TYPE_KEYS_BY_LEN_DESC, TYPE_MAP};
    use syn::parse_str;

    #[test]
    fn parse_output_accepts_known_specifiers() {
        let out = parse_str::<Output>(r#""A=%u B=%.*s C=%c", 1, bytes, 3"#).unwrap();
        assert_eq!(out.args.len(), 3);
    }

    #[test]
    fn parse_output_accepts_escaped_percent() {
        let out = parse_str::<Output>(r#""progress: 100%% done""#).unwrap();
        assert_eq!(out.args.len(), 0);
    }

    #[test]
    fn parse_output_rejects_unknown_specifier() {
        let err = parse_str::<Output>(r#""value=%x""#).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown klipper_output format specifier"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn parse_output_rejects_truncated_specifier() {
        let err = parse_str::<Output>(r#""value=%""#).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown klipper_output format specifier"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn parse_output_accepts_all_registered_specifiers() {
        // If a new key is added to TYPE_MAP, this automatically exercises it.
        let mut fmt = String::new();
        let mut args = Vec::new();
        for (i, &spec) in TYPE_KEYS_BY_LEN_DESC.iter().enumerate() {
            if i > 0 {
                fmt.push(' ');
            }
            fmt.push('%');
            fmt.push_str(spec);
            args.push("0");
        }
        let src = format!(r#""{}", {}"#, fmt, args.join(", "));
        let out = parse_str::<Output>(&src).unwrap();
        assert_eq!(out.args.len(), TYPE_MAP.len());
    }
}
