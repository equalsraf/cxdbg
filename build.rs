
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use std::env;
use std::fs;
use std::io::{Write, Result};
use std::path::Path;

#[derive(Deserialize, Debug)]
struct ChromeDbgEvent {
    name: String,
    description: Option<String>,
    parameters: Option<Vec<ChromeDbgTypeDecl>>,
    experimental: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct ChromeDbgCommand {
    name: String,
    description: Option<String>,
    parameters: Option<Vec<ChromeDbgTypeDecl>>,
    returns: Option<Vec<ChromeDbgTypeDecl>>,
    experimental: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct ChromeDbgTypeDecl {
    id: Option<String>,
    #[serde(rename = "type")]
    _type: Option<String>,
    optional: Option<bool>,
    #[serde(rename = "$ref")]
    _ref: Option<String>,
    items: Option<Box<ChromeDbgTypeDecl>>,
    #[serde(rename = "enum")]
    _enum: Option<Vec<String>>,
    description: Option<String>,
    name: Option<String>,
    properties: Option<Vec<ChromeDbgTypeDecl>>,
}

impl ChromeDbgTypeDecl {
    fn type_id(&self, absprefix: &str, relprefix: &str) -> Option<String> {
        self.type_id_with_box(None, absprefix, relprefix)
    }
    fn type_id_with_box(&self, parent_type: Option<&str>, absprefix: &str, relprefix: &str) -> Option<String> {
        let mut s = String::new();

        if let Some(ref t) = self._type {
            match t.as_str() {
                "boolean" => s.push_str("bool"),
                "string" => s.push_str("String"),
                "integer" => s.push_str("i64"),
                "number" => s.push_str("f64"),
                "array" => {
                    if let Some(t) = self.items.as_ref().and_then(|t| t.type_id(absprefix, relprefix)) {
                        s = format!("Vec<{}>", t);
                    } else {
                        return None;
                    }
                }
                "any" => s.push_str("JsonValue"),
                "object" => s.push_str("JsonValue"),
                _ => return None,
            }
        } else if let Some(ref r) = self._ref {
            if r.contains(".") {
                // r is an absolute reference
                s = format!("{}{}", absprefix, &r.replace('.', "::"));
            } else {
                // r is a relative reference
                s = format!("{}{}", relprefix, r);
            }
        } else {
            return None;
        }

        if Some(s.as_str()) == parent_type {
            s = format!("Box<{}>", s);
        }

        if self.optional.unwrap_or(false) {
            Some(format!("Option<{}>", s))
        } else {
            Some(s)
        }
    }
}

#[derive(Deserialize)]
struct ChromeDbgDomain {
    domain: String,
    experimental: Option<bool>,
    commands: Vec<ChromeDbgCommand>,
    events: Option<Vec<ChromeDbgEvent>>,
    types: Option<Vec<ChromeDbgTypeDecl>>,
}

impl ChromeDbgDomain {
    fn genrust(&self, w: &mut Write) -> Result<()> {
        writeln!(w, "pub mod {} {{", self.domain)?;

        writeln!(w, "    #[allow(unused_imports)] use serde_json::Value as JsonValue;").unwrap();
        writeln!(w, "    #[allow(unused_imports)] use super::Nothing;").unwrap();
        writeln!(w, "    use super::*;").unwrap();

        if let Some(ref types) = self.types {
            for dtype in types {
                if let Some(ref variants) = dtype._enum {
                    // TODO Sadly serde does not support a fallback variant for enums
                    // see https://github.com/serde-rs/serde/issues/912 for a workaround

                    if let Some(ref d) = dtype.description {
                        writeln!(w, "    /// {}", d.trim())?;
                    }
                    writeln!(w, "        #[derive(Deserialize, Debug, Serialize)]")?;
                    writeln!(w, "        pub enum {} {{", dtype.id.as_ref().expect("Domain type has no id"))?;
                    for var in variants {
                        writeln!(w, r#"            #[serde(rename = "{}")]"#, var)?;
                        writeln!(w, "            _{},", var.replace('-', "_"))?;
                    }
                    writeln!(w, "        }}" )?;
                } else if let Some(ref properties) = dtype.properties {
                    if let Some(ref d) = dtype.description {
                        writeln!(w, "    /// {}", d.trim())?;
                    }
                    let dtype_id = dtype.id.as_ref().expect("Domain type has no id");
                    writeln!(w, "    #[derive(Deserialize, Debug, Serialize)]")?;
                    writeln!(w, "    pub struct {} {{", dtype_id)?;
                    for prop in properties {
                        let name = prop.name.as_ref().expect("Type property has no name");
                        if let Some(t) = prop.type_id_with_box(Some(&dtype_id), "super::", "") {
                        writeln!(w, r#"        #[serde(rename = "{}")]"#, name)?;
                        writeln!(w, "        pub _{}: {},", name, t)?;
                        }
                    }
                    writeln!(w, "    }}" )?;
                } else if let Some(t) = dtype.type_id("super::", "") {
                    writeln!(w, "    pub type {} = {};", dtype.id.as_ref().expect("Domain type has no id"), t)?;
                } else {
                    writeln!(w, "    pub type {} = JsonValue;", dtype.id.as_ref().expect("Domain type has no id"))?;
                }
            }
        }



        let mut cmd_type_info = Vec::new();
        // commands
        for cmd in &self.commands {
            // Create a return type for this command, or use Nothing
            let return_type_name = match cmd.returns.as_ref().map(Vec::as_slice).unwrap_or(&[]) {
                [] => "Nothing".to_string(),
                v => {
                    writeln!(w, "    #[derive(Deserialize, Debug)]")?;
                    writeln!(w, "    pub struct ReturnType_{} {{", cmd.name)?;
                    for r in v {
                        let name = r.name.as_ref().expect("Return type attr has no name");
                        writeln!(w, r#"        #[serde(rename = "{}")]"#, name)?;
                        writeln!(w, "        pub {}: {},",
                                 name,
                                 r.type_id("super::", "").as_ref().expect("Cannot determine return type"))?;
                    }
                    writeln!(w, "    }}")?;
                    format!("ReturnType_{}", cmd.name)
                }
            };

            // Create a request type for this command, this can be private since it is
            // only used later in the generated api functions.
            let request_type = if let Some(ref types) = cmd.parameters {
                writeln!(w, r#"    #[derive(Serialize, Debug)]
    struct Request_{} {{"#, cmd.name)?;

                for ty in types {
                    if let Some(ref s) = ty.description {
                            writeln!(w, "        /// {}", s)?;
                    }

                    let ty_name = ty.name.as_ref().expect("Argument has no name");
                    writeln!(w, r#"        #[serde(rename = "{}")]"#, ty_name)?;
                    if ty.optional.unwrap_or(false) {
                        // Dont serialize optional arguments. By default serde uses null.
                        writeln!(w, r#"        #[serde(skip_serializing_if = "Option::is_none")]"#)?;
                    }
                    writeln!(w, "        _{}: {},",
                             ty_name,
                             ty.type_id("super::", "").expect("Cannot determine type for argument"))?;
                }
                writeln!(w, "    }}")?;
                format!("Request_{}", cmd.name)
            } else {
                "Nothing".to_string()
            };

            cmd_type_info.push((cmd, request_type, return_type_name));
        }

        // a domain trait for the sync api
        writeln!(w, r#"    pub trait {}Api {{"#, &self.domain)?;
        for (cmd, _, return_type_name) in &cmd_type_info {
            if let Some(ref d) = cmd.description {
                writeln!(w, "        /// {}", d.trim())?;
            }
            write!(w, r#"        fn {}(&mut self"#, cmd.name)?;
            if let Some(ref types) = cmd.parameters {
                for ty in types {
                    write!(w, ", _{}: {}",
                           ty.name.as_ref().expect("Argument has no name"),
                           ty.type_id("super::", "").expect("Cannot determine type for argument"))?;
                }
            }
            writeln!(w, r#") -> Result<{}, ClientError>;"#, return_type_name)?;
        }
        writeln!(w, r#"    }}"#)?;

        writeln!(w, r#"    impl {}Api for DebugClient {{"#, &self.domain)?;
        for (cmd, request_type, return_type_name) in &cmd_type_info {
            write!(w, r#"        fn {}(&mut self"#, cmd.name)?;
            if let Some(ref types) = cmd.parameters {
                for ty in types {
                    write!(w, ", _{}: {}",
                           ty.name.as_ref().expect("Argument has no name"),
                           ty.type_id("super::", "").expect("Cannot determine type for argument"))?;
                }
            }

            let fullname = format!("{}.{}", &self.domain, cmd.name);
            writeln!(w, r#") -> Result<{}, ClientError> {{"#, return_type_name)?;
            write!(w, r#"            self.call("{}", {} {{"#, fullname, request_type)?;
            if let Some(ref types) = cmd.parameters {
                for (idx, ty) in types.iter().enumerate() {
                    if idx != 0 {
                        write!(w, ",")?;
                    }
                    write!(w, "_{}", ty.name.as_ref().expect("Argument type is missing a name"))?;
                }
            }
            writeln!(w, r#"}})"#)?;

            writeln!(w, r#"        }}"#)?;
        }
        writeln!(w, r#"    }}"#)?;

        writeln!(w, "}} // {}", self.domain)?;
        Ok(())
    }
}

#[derive(Deserialize)]
struct ChromeDbgProto {
    domains: Vec<ChromeDbgDomain>,
}

impl ChromeDbgProto {
    fn genrust(&self, f: &mut Write) -> Result<()> {
        writeln!(f, r#"
use DebugClient;
use Error as ClientError;
use serde;
use serde_json::Value as JsonValue;

/// A dummy type for commands that return nothing
#[derive(Serialize, Deserialize, Debug)]
pub struct Nothing {{
}}

fn deserialize_unit_enum<'de, D: serde::Deserializer<'de>>(_d: D) -> Result<(), D::Error> {{
    Ok(())
}}
"#)?;

        for domain in &self.domains {
            domain.genrust(f)
                .expect("Error writing src/proto.rs");
            writeln!(f, "pub use self::{}::{}Api;", &domain.domain, &domain.domain)?;
        }

        writeln!(f, "#[derive(Deserialize, Debug)]")?;
        writeln!(f, r#"#[serde(tag = "method", content = "params")]"#)?;
        writeln!(f, "pub enum Event {{")?;
        for domain in &self.domains {
            if let Some(ref events) = domain.events {
                for ev in events {
                    let fullname = format!("{}.{}", &domain.domain, ev.name);


                    if let Some(ref s) = ev.description {
                        writeln!(f, "    /// {}", s)?;
                    }
                    writeln!(f, r#"    #[serde(rename = "{}")]"#, fullname)?;
                    if let Some(ref types) = ev.parameters {
                        writeln!(f, r#"    {}_{} {{"#, &domain.domain, ev.name)?;
                        for ty in types {
                            let name = ty.name.as_ref().expect("Argument has no name");
                            let fixed_name = match name.as_str() {
                                "type" => "_type",
                                _ => name,
                            };
                            writeln!(f, r#"        #[serde(rename = "{}")]
        {}: {},"#,
                                     name,
                                     fixed_name,
                                     ty.type_id("", &format!("{}::", &domain.domain)).expect("Argument has no type"))?;
                        }
                        writeln!(f, r#"    }},"#)?;
                    } else {
                        // when serde decodes these unit variants it does not accept a map, but
                        // that is what chrome hands us
                        writeln!(f, r#"    #[serde(deserialize_with="deserialize_unit_enum")]"#)?;
                        writeln!(f, r#"    {}_{},"#, &domain.domain, ev.name)?;
                    }
                }
            }
        }
        writeln!(f, "}}" )
    }
}

const SOURCE: &'static str = "src/chrome_protocol.json";

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("proto.rs");

    println!("rerun-if-changed={}", SOURCE);

    let mut f = fs::File::open(SOURCE).expect("Failed to open protocol json");
    let p: ChromeDbgProto = serde_json::from_reader(&mut f)
        .expect("Error parsing protocol json");

    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(dest_path)
        .expect("Unable to open src/proto.rs for writing");

    p.genrust(&mut f).unwrap();
}
