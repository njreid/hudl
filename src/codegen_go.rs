use crate::ast::Param;
use crate::proto::{ProtoSchema, ProtoType};

pub struct GoOptions {
    pub package_name: String,
    pub pb_import_path: String,
    pub pb_package_name: String,
}

pub fn generate_go_wrapper(
    views: Vec<(String, Vec<Param>)>,
    opts: GoOptions,
) -> String {
    let mut code = String::new();

    // Check which imports are actually needed
    let mut needs_proto = false;
    let mut needs_pb = false;
    let mut needs_fmt = false;

    for (_, params) in &views {
        for p in params {
            let pt = ProtoSchema::parse_type(&p.type_name);
            match pt {
                ProtoType::Message(_) => {
                    needs_proto = true;
                    needs_pb = true;
                    needs_fmt = true;
                }
                ProtoType::Enum(_) => {
                    needs_pb = true;
                }
                _ => {}
            }
        }
    }

    // Package declaration
    code.push_str(&format!("package {}\n\n", opts.package_name));

    // Imports
    code.push_str("import (\n");
    if needs_fmt {
        code.push_str("\t\"fmt\"\n");
    }
    code.push_str("\t\"github.com/njreid/hudl/pkg/hudl\"\n");
    code.push_str("\t\"google.golang.org/protobuf/encoding/protowire\"\n");
    if needs_proto {
        code.push_str("\t\"google.golang.org/protobuf/proto\"\n");
    }
    if needs_pb && !opts.pb_import_path.is_empty() {
        code.push_str(&format!("\t\"{}\"\n", opts.pb_import_path));
    }
    code.push_str(")\n\n");

    // Views struct
    code.push_str("type Views struct {\n");
    code.push_str("\truntime *hudl.Runtime\n");
    code.push_str("}\n\n");

    code.push_str("func NewViews(rt *hudl.Runtime) *Views {\n");
    code.push_str("\treturn &Views{runtime: rt}\n");
    code.push_str("}\n\n");

    // Generate methods for each view
    for (view_name, params) in views {
        generate_view_method(&mut code, &view_name, &params, &opts);
    }

    code
}

fn generate_view_method(code: &mut String, view_name: &str, params: &[Param], opts: &GoOptions) {
    // Function signature
    code.push_str(&format!("func (v *Views) {}(", view_name));
    
    for (i, param) in params.iter().enumerate() {
        if i > 0 {
            code.push_str(", ");
        }
        let go_type = map_hudl_type_to_go(&param.type_name, param.repeated, &opts.pb_package_name);
        code.push_str(&format!("{} {}", param.name, go_type));
    }
    
    code.push_str(") (string, error) {\n");

    // Serialization logic
    if params.is_empty() {
        code.push_str(&format!("\treturn v.runtime.RenderBytes(\"{}\", nil)\n", view_name));
    } else {
        code.push_str("\tvar b []byte\n");
        
        for (i, param) in params.iter().enumerate() {
            let field_num = (i + 1) as u32; // Field numbers are 1-based index of param
            generate_param_serialization(code, param, field_num);
        }

        code.push_str(&format!("\treturn v.runtime.RenderBytes(\"{}\", b)\n", view_name));
    }

    code.push_str("}\n\n");
}

fn map_hudl_type_to_go(type_name: &str, repeated: bool, pb_pkg: &str) -> String {
    let pt = ProtoSchema::parse_type(type_name);
    let base_type = match pt {
        ProtoType::String => "string".to_string(),
        ProtoType::Int32 | ProtoType::Sint32 | ProtoType::Sfixed32 => "int32".to_string(),
        ProtoType::Int64 | ProtoType::Sint64 | ProtoType::Sfixed64 => "int64".to_string(),
        ProtoType::Uint32 | ProtoType::Fixed32 => "uint32".to_string(),
        ProtoType::Uint64 | ProtoType::Fixed64 => "uint64".to_string(),
        ProtoType::Bool => "bool".to_string(),
        ProtoType::Float => "float32".to_string(),
        ProtoType::Double => "float64".to_string(),
        ProtoType::Bytes => "[]byte".to_string(),
        ProtoType::Message(_) | ProtoType::Enum(_) => {
            if !pb_pkg.is_empty() {
                if matches!(pt, ProtoType::Message(_)) {
                    format!("*{}", qualified_go_type(type_name, pb_pkg))
                } else {
                    qualified_go_type(type_name, pb_pkg)
                }
            } else {
                if matches!(pt, ProtoType::Message(_)) {
                    format!("*{}", type_name)
                } else {
                    type_name.to_string()
                }
            }
        },
        _ => "interface{}".to_string(),
    };

    if repeated {
        format!("[]{}", base_type)
    } else {
        base_type
    }
}

fn qualified_go_type(type_name: &str, pb_pkg: &str) -> String {
    if type_name.contains('.') {
        type_name.to_string()
    } else {
        format!("{}.{}", pb_pkg, type_name)
    }
}

fn generate_param_serialization(code: &mut String, param: &Param, field_num: u32) {
    let name = &param.name;
    let proto_type = ProtoSchema::parse_type(&param.type_name);

    if param.repeated {
        code.push_str(&format!("\tfor _, v := range {} {{\n", name));
        generate_single_value_serialization(code, "v", &proto_type, field_num);
        code.push_str("\t}\n");
    } else {
        generate_single_value_serialization(code, name, &proto_type, field_num);
    }
}

fn generate_single_value_serialization(code: &mut String, var_name: &str, proto_type: &ProtoType, field_num: u32) {
    match proto_type {
        ProtoType::String => {
            code.push_str(&format!("\tb = protowire.AppendTag(b, {}, protowire.BytesType)\n", field_num));
            code.push_str(&format!("\tb = protowire.AppendString(b, {})\n", var_name));
        }
        ProtoType::Int32 | ProtoType::Int64 | ProtoType::Uint32 | ProtoType::Uint64 | ProtoType::Bool | ProtoType::Enum(_) => {
            code.push_str(&format!("\tb = protowire.AppendTag(b, {}, protowire.VarintType)\n", field_num));
            let cast = match proto_type {
                ProtoType::Bool => {
                    format!("func() uint64 {{ if {} {{ return 1 }}; return 0 }}()", var_name)
                },
                ProtoType::Enum(_) => format!("uint64({})", var_name),
                _ => format!("uint64({})", var_name),
            };
            code.push_str(&format!("\tb = protowire.AppendVarint(b, {})\n", cast));
        }
        ProtoType::Message(_) => {
            code.push_str(&format!("\tbytesVal, err := proto.Marshal({})\n", var_name));
            code.push_str("\tif err != nil {\n\t\treturn \"\", fmt.Errorf(\"failed to marshal param: %w\", err)\n\t}\n");
            code.push_str(&format!("\tb = protowire.AppendTag(b, {}, protowire.BytesType)\n", field_num));
            code.push_str("\tb = protowire.AppendBytes(b, bytesVal)\n");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Param;

    #[test]
    fn test_generate_go_basic() {
        let views = vec![
            ("HomePage".to_string(), vec![
                Param { name: "title".to_string(), type_name: "string".to_string(), repeated: false, default_value: None },
                Param { name: "description".to_string(), type_name: "string".to_string(), repeated: false, default_value: None },
            ]),
            ("StaticPage".to_string(), vec![]),
        ];

        let opts = GoOptions {
            package_name: "views".to_string(),
            pb_import_path: "myapp/pb".to_string(),
            pb_package_name: "pb".to_string(),
        };

        let code = generate_go_wrapper(views, opts);

        assert!(code.contains("package views"));
        assert!(code.contains("type Views struct"));
        assert!(code.contains("func (v *Views) HomePage(title string, description string)"));
        assert!(code.contains("func (v *Views) StaticPage()"));
        assert!(code.contains("protowire.AppendString(b, title)"));
        assert!(code.contains("protowire.AppendString(b, description)"));
        assert!(code.contains("v.runtime.RenderBytes(\"HomePage\", b)"));
        assert!(code.contains("v.runtime.RenderBytes(\"StaticPage\", nil)"));
        // fmt and pb should NOT be imported
        assert!(!code.contains("\"fmt\""));
        assert!(!code.contains("\"myapp/pb\""));
    }

    #[test]
    fn test_generate_go_no_unnecessary_imports_mixed_scalars() {
        let views = vec![
            ("MixedView".to_string(), vec![
                Param { name: "count".to_string(), type_name: "int32".to_string(), repeated: false, default_value: None },
                Param { name: "active".to_string(), type_name: "bool".to_string(), repeated: false, default_value: None },
                Param { name: "tags".to_string(), type_name: "string".to_string(), repeated: true, default_value: None },
            ]),
        ];

        let opts = GoOptions {
            package_name: "views".to_string(),
            pb_import_path: "myapp/pb".to_string(),
            pb_package_name: "pb".to_string(),
        };

        let code = generate_go_wrapper(views, opts);

        assert!(!code.contains("\"fmt\""));
        assert!(!code.contains("\"myapp/pb\""));
        assert!(!code.contains("\"google.golang.org/protobuf/proto\""));
        assert!(code.contains("\"google.golang.org/protobuf/encoding/protowire\""));
    }

    #[test]
    fn test_generate_go_repeated() {
        let views = vec![
            ("ListView".to_string(), vec![
                Param { name: "items".to_string(), type_name: "string".to_string(), repeated: true, default_value: None },
            ]),
        ];

        let opts = GoOptions {
            package_name: "views".to_string(),
            pb_import_path: "".to_string(),
            pb_package_name: "pb".to_string(),
        };

        let code = generate_go_wrapper(views, opts);

        assert!(code.contains("func (v *Views) ListView(items []string)"));
        assert!(code.contains("for _, v := range items"));
        assert!(code.contains("protowire.AppendString(b, v)"));
    }

    #[test]
    fn test_generate_go_types() {
        let views = vec![
            ("TypesView".to_string(), vec![
                Param { name: "count".to_string(), type_name: "int32".to_string(), repeated: false, default_value: None },
                Param { name: "active".to_string(), type_name: "bool".to_string(), repeated: false, default_value: None },
            ]),
        ];

        let opts = GoOptions {
            package_name: "views".to_string(),
            pb_import_path: "".to_string(),
            pb_package_name: "pb".to_string(),
        };

        let code = generate_go_wrapper(views, opts);

        assert!(code.contains("func (v *Views) TypesView(count int32, active bool)"));
        assert!(code.contains("uint64(count)"));
        assert!(code.contains("if active { return 1 }; return 0"));
    }

    #[test]
    fn test_generate_go_message() {
        let views = vec![
            ("UserPage".to_string(), vec![
                Param { name: "user".to_string(), type_name: "User".to_string(), repeated: false, default_value: None },
            ]),
        ];

        let opts = GoOptions {
            package_name: "views".to_string(),
            pb_import_path: "myapp/pb".to_string(),
            pb_package_name: "pb".to_string(),
        };

        let code = generate_go_wrapper(views, opts);

        assert!(code.contains("\"fmt\""));
        assert!(code.contains("\"google.golang.org/protobuf/proto\""));
        assert!(code.contains("\"myapp/pb\""));
        assert!(code.contains("func (v *Views) UserPage(user *pb.User)"));
        assert!(code.contains("proto.Marshal(user)"));
        assert!(code.contains("fmt.Errorf"));
    }
}