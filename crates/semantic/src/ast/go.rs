use crate::ast::go_nodes::anon_unions::Block_IfStatement;
use crate::ast::go_nodes::anon_unions::DefaultCase_ExpressionCase;
use crate::ast::go_nodes::anon_unions::DefaultCase_TypeCase;
use crate::ast::go_nodes::anon_unions::Expression_ForClause_RangeClause;
use crate::ast::go_nodes::anon_unions::Expression_LiteralValue;
use crate::ast::go_nodes::anon_unions::Expression_Type_VariadicArgument;
use crate::ast::go_nodes::anon_unions::KeyedElement_LiteralElement;
use crate::ast::go_nodes::anon_unions::NegatedType_QualifiedType_TypeIdentifier;
use crate::ast::go_nodes::anon_unions::NotEq_Mod_And_AndAnd_AndBitXor_Mul_Add_Sub_Div_Lt_LtLt_LtEq_EqEq_Gt_GtEq_GtGt_BitXor_Or_OrOr as BinaryOp;
use crate::ast::go_nodes::anon_unions::Not_And_Mul_Add_Sub_LtSub_BitXor as UnaryOp;
use crate::ast::go_nodes::anon_unions::ParameterDeclaration_VariadicParameterDeclaration as ParamDecl;
use crate::ast::go_nodes::anon_unions::Statement_FunctionDeclaration_ImportDeclaration_MethodDeclaration_PackageClause as TopLevel;
use crate::ast::go_nodes::*;
use crate::ast::source_info::SourceInfo;
use crate::il;
use crate::il::{StatementValue, ValueExtra};

use anyhow::anyhow;
use anyhow::Result;
use ast_grep_core::tree_sitter::StrDoc;
use ast_grep_core::AstGrep;
use ast_grep_language::SupportLang;
use type_sitter::HasChild;
use type_sitter::{HasChildren, Node};

use super::go_nodes::anon_unions::Comma_Identifier;
use super::go_nodes::anon_unions::Comma_Type;
use super::go_nodes::anon_unions::SimpleType_ParameterList;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TMP_VAR_COUNTER: AtomicUsize = AtomicUsize::new(0);

use std::sync::LazyLock;
use std::vec;

static BUILTIN_TYPES: LazyLock<HashMap<&'static str, il::Type>> = LazyLock::new(|| {
  [
    ("int", il::Type::Int),
    ("uint", il::Type::UInt),
    ("int8", il::Type::Byte),
    ("uint8", il::Type::UByte),
    ("int16", il::Type::Short),
    ("uint16", il::Type::UShort),
    ("int32", il::Type::Int),
    ("uint32", il::Type::UInt),
    ("int64", il::Type::Long),
    ("uint64", il::Type::ULong),
    ("float32", il::Type::Float),
    ("float64", il::Type::Double),
    ("string", il::Type::String),
    ("bool", il::Type::Bool),
    ("nil", il::Type::Null),
    (
      "map",
      il::Type::Named {
        name: "map".to_string(),
        generics: vec![il::Type::Any, il::Type::Any],
      },
    ),
    (
      "chan",
      il::Type::Named {
        name: "chan".to_string(),
        generics: vec![il::Type::Any],
      },
    ),
    ("any", il::Type::Any),
  ]
  .into_iter()
  .collect()
});

pub struct GoConverter<'src> {
  pub program: il::Program,
  pub source_info: SourceInfo<'src>,
}

impl<'src> GoConverter<'src> {
  pub fn new(path: PathBuf) -> Self {
    Self {
      program: il::Program::new(path),
      source_info: SourceInfo::new(),
    }
  }

  pub fn get_tmp_var(&mut self, t: Option<il::Type>) -> (il::Value, String) {
    let counter = TMP_VAR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = format!("%tmp_{}", counter);
    (
      il::Value {
        kind: il::ValueKind::Ident(name.clone()),
        extra: ValueExtra::new(t, None),
        tag: None,
      },
      name,
    )
  }

  pub fn convert(&mut self, root: &'src AstGrep<StrDoc<SupportLang>>) -> () {
    eprintln!("Converting source file");
    let stmts = {
      let node = SourceFile::try_from_raw(root.root().get_inner_node()).unwrap();
      node
        .children(&mut node.walk())
        .map(|child| child.unwrap())
        .collect::<Vec<_>>()
    };
    match self.convert_toplevels(&root, &stmts) {
      Ok(stmts) => self.program.add_statements(stmts),
      Err(e) => eprintln!("Error converting top-level statements: {:?}", e),
    }
  }

  fn convert_parameter_declaration(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    param: &ParamDecl<'src>,
  ) -> Result<Vec<il::Value>> {
    match param {
      ParamDecl::ParameterDeclaration(pd) => {
        let names = pd
          .names(&mut pd.walk())
          .map(|name| Ok(name.utf8_text(root.source().as_bytes())?.to_string()))
          .collect::<Result<Vec<_>>>()?;
        let t = self.convert_type(root, &pd.r#type().unwrap());
        if names.is_empty() {
          Ok(vec![il::Value {
            kind: il::ValueKind::Ident("_".to_string()),
            extra: ValueExtra::new(Some(t.clone()), None),
            tag: None,
          }])
        } else {
          Ok(
            names
              .into_iter()
              .map(|name| {
                if BUILTIN_TYPES.contains_key(name.as_str()) {
                  il::Value {
                    kind: il::ValueKind::Ident("_".to_string()),
                    extra: ValueExtra::new(Some(BUILTIN_TYPES[name.as_str()].clone()), None),
                    tag: None,
                  }
                } else {
                  il::Value {
                    kind: il::ValueKind::Ident(name.clone()),
                    extra: ValueExtra::new(Some(t.clone()), None),
                    tag: None,
                  }
                }
              })
              .collect::<Vec<_>>(),
          )
        }
      }
      ParamDecl::VariadicParameterDeclaration(vd) => {
        let name = vd
          .name()
          .ok_or(anyhow!("Variadic parameter declaration has no name"))?
          .utf8_text(root.source().as_bytes())?
          .to_string();
        let t = self.convert_type(root, &vd.r#type().unwrap());
        Ok(vec![il::Value {
          kind: il::ValueKind::Ident(name),
          extra: ValueExtra::new(Some(il::Type::Array(Box::new(t.clone()), None)), None),
          tag: None,
        }])
      }
    }
  }

  fn convert_parameter_list(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    param_list: &ParameterList<'src>,
  ) -> Result<Vec<il::Value>> {
    param_list
      .children(&mut param_list.walk())
      .map(|plr| {
        plr
          .map_err(|e| anyhow!("Error converting parameter list: {:?}", e))
          .and_then(|pl| self.convert_parameter_declaration(root, &pl))
      })
      .collect::<Result<Vec<Vec<il::Value>>>>()
      .map(|inner| inner.into_iter().flatten().collect::<Vec<_>>())
  }

  fn convert_toplevels(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    tops: &Vec<TopLevel<'src>>,
  ) -> Result<Vec<il::Statement>> {
    // Convert each top-level declaration
    let results: Vec<Vec<il::Statement>> = tops
      .iter()
      .map(|top| -> Result<Vec<il::Statement>> {
        match top {
          TopLevel::Statement(st) => self.convert_statement(root, st),
          TopLevel::PackageClause(pc) => {
            let name = pc
              .package_identifier()
              .map_err(|e| anyhow!("Error converting package name: {:?}", e))?
              .utf8_text(root.source().as_bytes())?
              .to_string();
            Ok(vec![il::Statement::Define(il::DefineKind::Package(name))])
          }
          TopLevel::MethodDeclaration(md) => {
            let receiver_list = md
              .receiver()
              .map_err(|e| anyhow!("Error converting method receiver: {:?}", e))?;
            let receiver_params = self.convert_parameter_list(root, &receiver_list)?;
            let method_param_list = md
              .parameters()
              .map_err(|e| anyhow!("Error converting method parameters: {:?}", e))?;
            let method_params = self.convert_parameter_list(root, &method_param_list)?;
            let params = [receiver_params, method_params]
              .concat()
              .into_iter()
              .flat_map(|v| match v {
                il::Value {
                  kind: il::ValueKind::Ident(name),
                  extra:
                    ValueExtra {
                      type_declared: Some(t),
                      ..
                    },
                  tag: _,
                } => Some((t, Some(name))),
                _ => {
                  eprintln!("Expected Var value in method params, got: {:?}", v);
                  None
                }
              })
              .collect::<Vec<_>>();
            let method_stmts = md
              .body()
              .transpose()
              .map_err(|e| anyhow!("Error converting method body: {:?}", e))?
              .map(|x| self.convert_block(root, &x))
              .transpose()?
              .unwrap_or(vec![]);
            let method_sig = il::MethodSig {
              name: md
                .name()
                .map_err(|e| anyhow!("Error converting method name: {:?}", e))?
                .utf8_text(root.source().as_bytes())?
                .to_string(),
              params,
            };
            let ret_type = md
              .result()
              .transpose()
              .map_err(|e| anyhow!("Error converting method return type: {:?}", e))?
              .map(|x| self.convert_simple_type_parameter_list(root, &x))
              .transpose()?
              .unwrap_or(vec![]);
            let method_decl = il::DefineKind::Method {
              sig: method_sig,
              body: method_stmts,
              ret_type: ret_type,
            };
            Ok(vec![il::Statement::Define(method_decl)])
          }
          TopLevel::FunctionDeclaration(fd) => {
            let function_param_list = fd
              .parameters()
              .map_err(|e| anyhow!("Error converting function parameters: {:?}", e))?;
            let function_params = self.convert_parameter_list(root, &function_param_list)?;
            let params = function_params
              .into_iter()
              .flat_map(|v| match v {
                il::Value {
                  kind: il::ValueKind::Ident(name),
                  extra:
                    ValueExtra {
                      type_declared: Some(t),
                      ..
                    },
                  tag: _,
                } => Some((t, Some(name))),
                _ => {
                  eprintln!("Expected Var value in function params, got: {:?}", v);
                  None
                }
              })
              .collect::<Vec<_>>();
            let function_stmts = fd
              .body()
              .transpose()
              .map_err(|e| anyhow!("Error converting function body: {:?}", e))?
              .map(|x| self.convert_block(root, &x))
              .transpose()?
              .unwrap_or(vec![]);
            let function_sig = il::MethodSig {
              name: fd
                .name()
                .map_err(|e| anyhow!("Error converting function name: {:?}", e))?
                .utf8_text(root.source().as_bytes())?
                .to_string(),
              params,
            };
            let ret_type = fd
              .result()
              .transpose()
              .map_err(|e| anyhow!("Error converting function return type: {:?}", e))?
              .map(|x| self.convert_simple_type_parameter_list(root, &x))
              .transpose()?
              .unwrap_or(vec![]);
            let method_decl = il::DefineKind::Method {
              sig: function_sig,
              body: function_stmts,
              ret_type: ret_type,
            };
            Ok(vec![il::Statement::Define(method_decl)])
          }
          _ => {
            eprintln!("convert_toplevels: Unhandled node type: {:?}", top);
            Ok(vec![])
          }
        }
      })
      .collect::<Result<Vec<Vec<il::Statement>>>>()?;
    Ok(results.into_iter().flatten().collect())
  }

  fn convert_block(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    block: &Block<'src>,
  ) -> Result<Vec<il::Statement>> {
    let stmts = block
      .statement_list()
      .transpose()
      .map_err(|e| anyhow!("Error converting statement list: {:?}", e))?;
    match stmts {
      None => Ok(vec![]),
      Some(stmts) => Ok(
        stmts
          .statements(&mut block.walk())
          .map(|child| {
            child
              .map_err(|e| anyhow!("Error converting block statement: {:?}", e))
              .and_then(|c| self.convert_statement(root, &c))
          })
          .collect::<Result<Vec<Vec<il::Statement>>>>()?
          .into_iter()
          .flatten()
          .collect::<Vec<_>>(),
      ),
    }
  }

  fn convert_type(&mut self, root: &AstGrep<StrDoc<SupportLang>>, t: &Type<'src>) -> il::Type {
    match t {
      Type::SimpleType(st) => self.convert_simple_type(root, st),
      _ => {
        eprintln!("convert_type: Unhandled type: {:?}", t);
        il::Type::Any
      }
    }
  }

  fn convert_type_elem(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    te: &TypeElem<'src>,
  ) -> il::Type {
    te.types(&mut te.walk())
      .map(|t| self.convert_type(root, &t.unwrap()))
      .next()
      .unwrap_or(il::Type::Any)
  }

  fn convert_simple_type_parameter_list(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    pl: &SimpleType_ParameterList<'src>,
  ) -> Result<Vec<il::Type>> {
    match pl {
      SimpleType_ParameterList::SimpleType(st) => Ok(vec![self.convert_simple_type(root, &st)]),
      SimpleType_ParameterList::ParameterList(pl) => {
        self.convert_parameter_list(root, &pl).map(|vs| {
          vs.into_iter()
            .map(|v| match v {
              il::Value {
                kind: il::ValueKind::Ident(_name),
                extra:
                  ValueExtra {
                    type_declared: Some(t),
                    ..
                  },
                tag: _,
              } => t,
              _ => {
                eprintln!("Expected Ident value in function return type, got: {:?}", v);
                il::Type::Any
              }
            })
            .collect::<Vec<_>>()
        })
      }
    }
  }

  fn convert_simple_type(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    st: &SimpleType<'src>,
  ) -> il::Type {
    match st {
      SimpleType::TypeIdentifier(id) => {
        let name = id.utf8_text(root.source().as_bytes()).unwrap().to_string();
        if let Some(t) = BUILTIN_TYPES.get(name.as_str()) {
          t.clone()
        } else {
          il::Type::Named {
            name,
            generics: vec![],
          }
        }
      }
      SimpleType::GenericType(gt) => {
        let t = gt
          .r#type()
          .map(|t| match t {
            NegatedType_QualifiedType_TypeIdentifier::NegatedType(nt) => {
              self.convert_simple_type(root, &SimpleType::NegatedType(nt))
            }
            NegatedType_QualifiedType_TypeIdentifier::QualifiedType(qt) => {
              self.convert_simple_type(root, &SimpleType::QualifiedType(qt))
            }
            NegatedType_QualifiedType_TypeIdentifier::TypeIdentifier(id) => {
              self.convert_simple_type(root, &SimpleType::TypeIdentifier(id))
            }
          })
          .unwrap_or(il::Type::Any);
        let t_args = gt
          .type_arguments()
          .map(|args| {
            args
              .type_elems(&mut gt.walk())
              .map(|t| self.convert_type_elem(root, &t.unwrap()))
              .collect::<Vec<_>>()
          })
          .unwrap_or(vec![]);
        match t {
          il::Type::Named { name, generics } => il::Type::Named {
            name,
            generics: [generics, t_args].concat(),
          },
          _ => {
            eprintln!("convert_simple_type: Unhandled generic type: {:?}", gt);
            t
          }
        }
      }
      SimpleType::ChannelType(ct) => {
        let value_type = ct
          .value()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        let chan_name = match ct.utf8_text(root.source().as_bytes()) {
          Ok(name) if name.starts_with("chan<-") => "chan<-".to_string(),
          Ok(name) if name.starts_with("<-chan") => "<-chan".to_string(),
          Ok(_) => "chan".to_string(),
          Err(e) => {
            eprintln!("Error converting channel type: {:?}", e);
            "chan".to_string()
          }
        };
        il::Type::Named {
          name: chan_name,
          generics: vec![value_type],
        }
      }
      SimpleType::MapType(mt) => {
        let key_type = mt
          .key()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        let value_type = mt
          .value()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        il::Type::Named {
          name: "map".to_string(),
          generics: vec![key_type, value_type],
        }
      }
      SimpleType::SliceType(st) => {
        let elem_type = st
          .element()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        il::Type::Slice(Box::new(elem_type))
      }
      SimpleType::ArrayType(at) => {
        let elem_type = at
          .element()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        let (length_exp, length_stmts) = at
          .length()
          .map_err(|e| anyhow!("Error converting array length: {:?}", e))
          .and_then(|exp| self.convert_expression(root, &exp))
          .unwrap_or((
            il::Value {
              kind: il::ValueKind::IntLit(0),
              extra: ValueExtra::new(None, None),
              tag: None,
            },
            vec![],
          ));
        let length = match length_exp {
          il::Value {
            kind: il::ValueKind::IntLit(len),
            extra: ValueExtra { .. },
            tag: _,
          } => Some(len as usize),
          _ => {
            eprintln!(
              "Expected integer literal for array length, got: {:?}",
              length_exp
            );
            None
          }
        };
        if !length_stmts.is_empty() {
          eprintln!(
            "Array length conversion produced statements: {:?}",
            length_stmts
          );
        };
        il::Type::Array(Box::new(elem_type), length)
      }
      SimpleType::PointerType(pt) => {
        let elem_type = pt
          .r#type()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);
        il::Type::Pointer(Box::new(elem_type))
      }
      SimpleType::FunctionType(ft) => {
        let params = ft
          .parameters()
          .map_err(|e| anyhow!("Error converting function parameters: {:?}", e))
          .and_then(|pl| self.convert_parameter_list(root, &pl))
          .map(|vs| {
            vs.into_iter()
              .map(|v| match v {
              il::Value {
                  kind: il::ValueKind::Ident(_name),
                  extra:
                    ValueExtra {
                      type_declared: Some(t),
                      ..
                    },
                  tag: _,
                } => t,
                _ => {
                  eprintln!("Expected Ident value in function params, got: {:?}", v);
                  il::Type::Any
                }
              })
              .collect::<Vec<_>>()
          })
          .unwrap_or(vec![]);
        let ret_type = ft
          .result()
          .map(|ret| {
            {
              ret
                .map_err(|e| anyhow!("Error converting return type: {:?}", e))
                .and_then(|pl| self.convert_simple_type_parameter_list(root, &pl))
            }
            .unwrap_or(vec![])
          })
          .unwrap_or(vec![il::Type::Void]);
        il::Type::Function {
          params,
          ret: ret_type,
        }
      }
      SimpleType::NegatedType(nt) =>
      // Not supported yet.
      {
        nt.r#type()
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any)
      }
      SimpleType::QualifiedType(qt) => {
        let name = qt
          .name()
          .map_err(|e| anyhow!("Error converting qualified type name: {:?}", e))
          .and_then(|id| {
            id.utf8_text(root.source().as_bytes())
              .map_err(|e| anyhow!("Error converting qualified type name text: {:?}", e))
              .map(|s| s.to_string())
          });
        let package = qt
          .package()
          .map_err(|e| anyhow!("Error converting qualified type package: {:?}", e))
          .and_then(|pkg| {
            pkg
              .utf8_text(root.source().as_bytes())
              .map_err(|e| anyhow!("Error converting qualified type package text: {:?}", e))
              .map(|s| s.to_string())
          });
        match (name, package) {
          (Ok(name), Ok(package)) => il::Type::Named {
            name: format!("{}.{}", package, name),
            generics: vec![],
          },
          _ => {
            eprintln!("convert_simple_type: Unhandled qualified type: {:?}", qt);
            il::Type::Any
          }
        }
      }
      _ => {
        eprintln!("convert_simple_type: Unhandled type: {:?}", st);
        il::Type::Any
      }
    }
  }

  fn convert_statement(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    st: &Statement<'src>,
  ) -> Result<Vec<il::Statement>> {
    match st {
      Statement::SimpleStatement(sst) => self.convert_simple_statement(root, sst),
      Statement::ConstDeclaration(cd) => {
        cd.const_specs(&mut cd.walk())
          .map(|spec| {
            let spec = spec.map_err(|e| anyhow!("Error converting const spec: {:?}", e))?;
            let names = spec
              .names(&mut cd.walk())
              .filter_map(|name| match name {
                Ok(Comma_Identifier::Identifier(id)) => Some(Ok(
                  id.utf8_text(root.source().as_bytes()).unwrap().to_string(),
                )),
                Ok(Comma_Identifier::Comma(_)) => None,
                Err(e) => Some(Err(anyhow!("Error converting const name: {:?}", e))),
              })
              .collect::<Result<Vec<String>>>()?;
            let type_ = spec
              .r#type()
              .transpose()
              .map_err(|e| anyhow!("Error converting const type: {:?}", e))?
              .map(|t| self.convert_type(root, &t))
              .unwrap_or(il::Type::Any);
            if let Some(Ok(value_list)) = spec.value() {
              let values = value_list
                .expressions(&mut cd.walk())
                .map(|expr| self.convert_expression(root, &expr.unwrap()))
                .collect::<Result<Vec<_>>>()?;
              let (values, stmts): (Vec<il::Value>, Vec<Vec<il::Statement>>) =
                values.into_iter().unzip();
              let tag = self.source_info.register(*spec.raw());
              // Create assignments for each variable-value pair
              Ok(
                [
                  stmts.into_iter().flatten().collect(),
                  names
                    .into_iter()
                    .zip(values)
                    .map(|(name, value)| il::Statement::Assign {
                      left: il::Value {
                        kind: il::ValueKind::Ident(name),
                        extra: ValueExtra::new(Some(type_.clone()), None),
                        tag: None,
                      },
                      right: value,
                      tag: Some(tag),
                    })
                    .collect::<Vec<_>>(),
                ]
                .concat(),
              )
            } else {
              // Handle const declarations without initializers
              Err(anyhow!("Const declaration has no value"))
            }
          })
          .collect::<Result<Vec<Vec<_>>>>()
          .map(|inner| inner.into_iter().flatten().collect())
      }
      Statement::VarDeclaration(vd) => {
        let spec = vd.child().unwrap().as_var_spec().unwrap();
        let names = spec
          .names(&mut vd.walk())
          .map(|name| {
            name
              .utf8_text(root.source().as_bytes())
              .unwrap()
              .to_string()
          })
          .collect::<Vec<_>>();

        let type_ = spec
          .r#type()
          .and_then(|t| t.ok())
          .map(|t| self.convert_type(root, &t))
          .unwrap_or(il::Type::Any);

        let tag = self.source_info.register(*spec.raw());

        if let Some(Ok(value_list)) = spec.value() {
          let values = value_list
            .expressions(&mut vd.walk())
            .map(|expr| self.convert_expression(root, &expr.unwrap()))
            .collect::<Result<Vec<_>>>()?;
          let (values, stmts): (Vec<il::Value>, Vec<Vec<il::Statement>>) =
            values.into_iter().unzip();
          // Create assignments for each variable-value pair
          Ok(
            [
              stmts.into_iter().flatten().collect(),
              names
                .into_iter()
                .zip(values)
                .map(|(name, value)| il::Statement::Assign {
                  left: il::Value {
                    kind: il::ValueKind::Ident(name),
                    extra: ValueExtra::new(Some(type_.clone()), None),
                    tag: None,
                  },
                  right: value,
                  tag: Some(tag),
                })
                .collect::<Vec<_>>(),
            ]
            .concat(),
          )
        } else {
          // Handle variable declarations without initializers
          Ok(
            names
              .iter()
              .map(|name| il::Statement::Assign {
                left: il::Value {
                  kind: il::ValueKind::Ident(name.clone()),
                  extra: ValueExtra::new(Some(type_.clone()), None),
                  tag: None,
                },
                right: il::Value {
                  kind: il::ValueKind::NullLit,
                  extra: ValueExtra::new(None, None),
                  tag: None,
                },
                tag: Some(tag),
              })
              .collect::<Vec<_>>(),
          )
        }
      }
      Statement::IfStatement(is) => {
        let (cond, cond_stmts) = self.convert_expression(root, &is.condition().unwrap())?;
        let body = is.consequence().unwrap();
        let else_body = is.alternative();
        let then_stmts = self.convert_block(root, &body)?;
        let else_stmts = if let Some(Ok(else_body)) = else_body {
          match else_body {
            Block_IfStatement::Block(block) => self.convert_block(root, &block)?,
            Block_IfStatement::IfStatement(if_stmt) => {
              self.convert_statement(root, &Statement::IfStatement(if_stmt))?
            }
          }
        } else {
          vec![]
        };
        let src_node = is.raw();
        let tag = self.source_info.register(*src_node);
        Ok(
          [
            cond_stmts,
            vec![il::Statement::If {
              condition: cond,
              then_stmts,
              else_stmts,
              tag: Some(tag),
            }],
          ]
          .concat(),
        )
      }
      Statement::ForStatement(fs) => {
        let body = fs
          .body()
          .map_err(|e| anyhow!("Error converting for body: {:?}", e))?;
        let stmts = self.convert_block(root, &body)?;
        let tag = self.source_info.register(*fs.raw());
        match fs.other() {
          None => {
            // Simple for loop
            Ok(vec![il::Statement::For {
              init: vec![],
              condition: StatementValue {
                statements: vec![],
                result: Some(il::Value {
                  kind: il::ValueKind::IntLit(1),
                  extra: ValueExtra::new(None, None),
                  tag: None,
                }),
              },
              update: vec![],
              body: stmts,
              tag: Some(tag),
            }])
          }
          Some(Ok(Expression_ForClause_RangeClause::Expression(for_clause))) => {
            // For loop with condition
            let (cond, cond_stmts) = self.convert_expression(root, &for_clause)?;
            Ok(vec![il::Statement::For {
              init: vec![],
              condition: StatementValue {
                statements: cond_stmts,
                result: Some(cond),
              },
              update: vec![],
              body: stmts,
              tag: Some(tag),
            }])
          }
          Some(Ok(Expression_ForClause_RangeClause::ForClause(for_clause))) => {
            let init_stmts = for_clause
              .initializer()
              .map(|init| {
                init
                  .map_err(|e| anyhow!("Error converting for initializer: {:?}", e))
                  .and_then(|init| self.convert_simple_statement(root, &init))
              })
              .transpose()?
              .unwrap_or(vec![]);
            let (cond, cond_stmts) = for_clause
              .condition()
              .map(|cond| {
                cond
                  .map_err(|e| anyhow!("Error converting for condition: {:?}", e))
                  .and_then(|cond| self.convert_expression(root, &cond))
              })
              .transpose()?
              .unwrap_or((
                il::Value {
                  kind: il::ValueKind::IntLit(1),
                  extra: ValueExtra::new(None, None),
                  tag: None,
                },
                vec![],
              ));
            let update_stmts = for_clause
              .update()
              .map(|update| {
                update
                  .map_err(|e| anyhow!("Error converting for update: {:?}", e))
                  .and_then(|update| self.convert_simple_statement(root, &update))
              })
              .transpose()?
              .unwrap_or(vec![]);
            Ok(vec![il::Statement::For {
              init: init_stmts,
              condition: StatementValue {
                statements: cond_stmts,
                result: Some(cond),
              },
              update: update_stmts,
              body: stmts,
              tag: Some(tag),
            }])
          }
          _ => Ok(vec![]), // TODO: RangeClause handling
        }
      }
      Statement::TypeSwitchStatement(tss) => {
        let value = tss
          .value()
          .map_err(|e| anyhow!("Error converting type switch value: {:?}", e))?;
        let (expr, expr_stmts) = self.convert_expression(root, &value)?;
        let mut cases = Vec::new();
        let others = tss
          .others(&mut tss.walk())
          .map(|x| x.map_err(|e| anyhow!("Error converting type switch case: {:?}", e)))
          .collect::<Result<Vec<_>>>()?;
        let (err, default, cases) = others.into_iter().fold(
          (None, None, &mut cases),
          |(err, acc_default, acc_stmts), child| match child {
            DefaultCase_TypeCase::DefaultCase(dc) => {
              let stmts = dc.statement_list().transpose().map_err(|e| {
                anyhow!(
                  "Error converting type switch default case statement list: {:?}",
                  e
                )
              });
              let stmts = stmts.and_then(|sl| match sl {
                None => Ok(vec![]),
                Some(sl) => sl
                  .statements(&mut sl.walk())
                  .map(|child| {
                    child
                      .map_err(|e| {
                        anyhow!(
                          "Error converting type switch default case statement: {:?}",
                          e
                        )
                      })
                      .and_then(|c| self.convert_statement(root, &c))
                  })
                  .collect::<Result<Vec<Vec<il::Statement>>>>()
                  .map(|inner| inner.into_iter().flatten().collect::<Vec<_>>()),
              });
              match stmts {
                Ok(stmts) => (err, Some(stmts), acc_stmts),
                Err(e) => (Some(e), acc_default, acc_stmts),
              }
            }
            DefaultCase_TypeCase::TypeCase(tc) => {
              let stmts = tc
                .statement_list()
                .transpose()
                .map_err(|e| anyhow!("Error converting type switch case statement list: {:?}", e));
              let stmts = stmts.and_then(|sl| match sl {
                None => Ok(vec![]),
                Some(sl) => sl
                  .statements(&mut sl.walk())
                  .map(|child| {
                    child
                      .map_err(|e| anyhow!("Error converting type switch case statement: {:?}", e))
                      .and_then(|c| self.convert_statement(root, &c))
                  })
                  .collect::<Result<Vec<Vec<il::Statement>>>>()
                  .map(|inner| inner.into_iter().flatten().collect::<Vec<_>>()),
              });
              let types = tc
                .types(&mut tc.walk())
                .filter_map(|t| match t {
                  Ok(Comma_Type::Type(t)) => Some(Ok(self.convert_type(root, &t))),
                  Ok(Comma_Type::Comma(_)) => None,
                  Err(e) => Some(Err(anyhow!("Error converting type switch case: {:?}", e))),
                })
                .collect::<Result<Vec<_>>>();
              match (stmts, types) {
                (Ok(stmts), Ok(types)) => {
                  let mut result = Vec::new();
                  let len = types.len();
                  for (i, t) in types.into_iter().enumerate() {
                    if i < len - 1 {
                      result.push((
                        StatementValue {
                          statements: vec![],
                          result: Some(il::Value {
                            kind: il::ValueKind::TypeLit(t),
                            extra: ValueExtra::new(None, None),
                            tag: None,
                          }),
                        },
                        vec![],
                      ));
                    } else {
                      let mut stmts = stmts.clone();
                      stmts.push(il::Statement::Break);
                      result.push((
                        StatementValue {
                          statements: vec![],
                          result: Some(il::Value {
                            kind: il::ValueKind::TypeLit(t),
                            extra: ValueExtra::new(None, None),
                            tag: None,
                          }),
                        },
                        stmts,
                      ));
                    }
                  }
                  acc_stmts.extend(result);
                  (err, acc_default, acc_stmts)
                }
                (Err(e), _) => (Some(e), acc_default, acc_stmts),
                (_, Err(e)) => (Some(e), acc_default, acc_stmts),
              }
            }
          },
        );
        let tag = self.source_info.register(*tss.raw());
        match err {
          None => Ok(
            [vec![il::Statement::Switch {
              value: StatementValue {
                statements: expr_stmts,
                result: Some(expr),
              },
              cases: cases.clone(),
              default,
              tag: Some(tag),
            }]]
            .concat(),
          ),
          Some(e) => Err(e),
        }
      }
      Statement::ExpressionSwitchStatement(ess) => {
        let initializer = ess
          .initializer()
          .transpose()
          .map_err(|e| anyhow!("Error converting expression switch initializer: {:?}", e))?;
        let init_stmts = initializer
          .map(|init| self.convert_simple_statement(root, &init))
          .transpose()?;
        let value = ess
          .value()
          .transpose()
          .map_err(|e| anyhow!("Error converting expression switch value: {:?}", e))?;
        let expr = value
          .map(|v| self.convert_expression(root, &v))
          .transpose()?;
        let others = ess
          .others(&mut ess.walk())
          .map(|x| x.map_err(|e| anyhow!("Error converting expression switch case: {:?}", e)))
          .collect::<Result<Vec<_>>>()?;
        let (err, default, cases) = others.into_iter().fold(
          (None, None, Vec::new()),
          |(err, default, mut cases), case| match case {
            DefaultCase_ExpressionCase::ExpressionCase(ec) => {
              let stmts = ec.statement_list().transpose().map_err(|e| {
                anyhow!(
                  "Error converting expression switch case statement list: {:?}",
                  e
                )
              });
              let stmts = stmts.and_then(|sl| match sl {
                None => Ok(vec![]),
                Some(sl) => sl
                  .statements(&mut sl.walk())
                  .map(|child| {
                    child
                      .map_err(|e| {
                        anyhow!("Error converting expression switch case statement: {:?}", e)
                      })
                      .and_then(|c| self.convert_statement(root, &c))
                  })
                  .collect::<Result<Vec<Vec<il::Statement>>>>()
                  .map(|inner| inner.into_iter().flatten().collect::<Vec<_>>()),
              });
              let expr = ec
                .value()
                .map_err(|e| anyhow!("Error converting expression switch case: {:?}", e))
                .and_then(|es| {
                  es.expressions(&mut ec.walk())
                    .map(|e| {
                      e.map_err(|e| anyhow!("Error converting expression switch case: {:?}", e))
                        .and_then(|e| self.convert_expression(root, &e))
                    })
                    .collect::<Result<Vec<_>>>()
                });
              match (stmts, expr) {
                (Ok(stmts), Ok(expr)) => {
                  for (e, e_stmts) in expr {
                    cases.push((
                      StatementValue {
                        statements: e_stmts,
                        result: Some(e),
                      },
                      stmts.clone(),
                    ));
                  }
                  (err, default, cases)
                }
                (Err(e), _) => (Some(e), default, cases),
                (_, Err(e)) => (Some(e), default, cases),
              }
            }
            DefaultCase_ExpressionCase::DefaultCase(dc) => {
              let stmts = dc.statement_list().transpose().map_err(|e| {
                anyhow!(
                  "Error converting expression switch default case statement list: {:?}",
                  e
                )
              });
              let stmts = stmts.and_then(|sl| match sl {
                None => Ok(vec![]),
                Some(sl) => sl
                  .statements(&mut sl.walk())
                  .map(|child| {
                    child
                      .map_err(|e| {
                        anyhow!(
                          "Error converting expression switch default case statement: {:?}",
                          e
                        )
                      })
                      .and_then(|c| self.convert_statement(root, &c))
                  })
                  .collect::<Result<Vec<Vec<il::Statement>>>>()
                  .map(|inner| inner.into_iter().flatten().collect::<Vec<_>>()),
              });
              match stmts {
                Ok(stmts) => (err, Some(stmts), cases),
                Err(e) => (Some(e), default, cases),
              }
            }
          },
        );
        let tag = self.source_info.register(*ess.raw());
        match err {
          None => Ok(
            [vec![il::Statement::Switch {
              value: StatementValue {
                statements: init_stmts
                  .unwrap_or(vec![])
                  .into_iter()
                  .chain(
                    expr
                      .clone()
                      .map(|(_, expr_stmts)| expr_stmts)
                      .unwrap_or(vec![])
                      .into_iter(),
                  )
                  .collect(),
                result: expr.map(|(expr, _)| expr),
              },
              cases: cases.clone(),
              default,
              tag: Some(tag),
            }]]
            .concat(),
          ),
          Some(e) => Err(e),
        }
      }
      Statement::ReturnStatement(rs) => {
        let values = rs
          .expression_list()
          .transpose()
          .map_err(|e| anyhow!("Error converting return statement: {:?}", e))?
          .map(|x| {
            x.expressions(&mut x.walk())
              .map(|expr| self.convert_expression(root, &expr.unwrap()))
              .collect::<Result<Vec<_>>>()
          })
          .unwrap_or(Ok(vec![]))?;
        let (values, stmts): (Vec<il::Value>, Vec<Vec<il::Statement>>) = values.into_iter().unzip();
        let tag = self.source_info.register(*rs.raw());
        Ok(
          [
            stmts.into_iter().flatten().collect(),
            vec![il::Statement::Return {
              // TODO: Handle multiple return values
              value: values.first().cloned(),
              tag: Some(tag),
            }],
          ]
          .concat(),
        )
      }
      Statement::BreakStatement(_) => Ok(vec![il::Statement::Break]),
      Statement::ContinueStatement(_) => Ok(vec![il::Statement::Continue]),
      _ => {
        eprintln!("convert_statement: Unhandled node type: {:?}", st);
        Ok(vec![])
      }
    }
  }

  fn convert_simple_statement(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    sst: &SimpleStatement<'src>,
  ) -> Result<Vec<il::Statement>> {
    match sst {
      SimpleStatement::ShortVarDeclaration(s) => {
        let mut left_values = Vec::new();
        let mut left_stmts = Vec::new();
        if let Ok(left_list) = s.left() {
          for expr in left_list.expressions(&mut s.walk()) {
            let (value, stmts) = self.convert_expression(root, &expr.unwrap())?;
            left_values.push(value);
            left_stmts.extend(stmts);
          }
        }
        let mut right_values = Vec::new();
        let mut right_stmts = Vec::new();
        if let Ok(right_list) = s.right() {
          for expr in right_list.expressions(&mut s.walk()) {
            let (value, stmts) = self.convert_expression(root, &expr.unwrap())?;
            right_values.push(value);
            right_stmts.extend(stmts);
          }
        }
        let tag = self.source_info.register(*s.raw());
        // Create assignments for each variable-value pair
        Ok(
          [
            left_stmts,
            right_stmts,
            left_values
              .into_iter()
              .zip(right_values)
              .map(|(left, right)| il::Statement::Assign {
                left,
                right,
                tag: Some(tag),
              })
              .collect::<Vec<_>>(),
          ]
          .concat(),
        )
      }
      SimpleStatement::ExpressionStatement(es) => {
        let (expr, stmts) = self.convert_expression(root, &es.expression().unwrap())?;
        let tag = self.source_info.register(*es.raw());
        Ok(
          [
            stmts,
            vec![il::Statement::Assign {
              left: il::Value {
                kind: il::ValueKind::Ident("_".to_string()),
                extra: ValueExtra::new(None, None),
                tag: None,
              },
              right: expr,
              tag: Some(tag),
            }],
          ]
          .concat(),
        )
      }
      SimpleStatement::IncStatement(is) => {
        let tag = self.source_info.register(*is.raw());
        is.expression()
          .map_err(|e| anyhow!("Error converting increment statement: {:?}", e))
          .and_then(|expr| self.convert_expression(root, &expr))
          .map(|(value, stmts)| {
            [
              stmts,
              vec![il::Statement::Assign {
                left: value.clone(),
                right: il::Value {
                  kind: il::ValueKind::Exp(il::Expr::BinOp {
                    left: Box::new(value),
                    right: Box::new(il::Value {
                      kind: il::ValueKind::IntLit(1),
                      extra: ValueExtra::new(None, None),
                      tag: None,
                    }),
                    op: il::BinaryOp::Add,
                  }),
                  extra: ValueExtra::new(None, None),
                  tag: None,
                },
                tag: Some(tag),
              }],
            ]
            .concat()
          })
      }
      SimpleStatement::DecStatement(ds) => {
        let tag = self.source_info.register(*ds.raw());
        ds.expression()
          .map_err(|e| anyhow!("Error converting decrement statement: {:?}", e))
          .and_then(|expr| self.convert_expression(root, &expr))
          .map(|(value, stmts)| {
            [
              stmts,
              vec![il::Statement::Assign {
                left: value.clone(),
                right: il::Value {
                  kind: il::ValueKind::Exp(il::Expr::BinOp {
                    left: Box::new(value),
                    right: Box::new(il::Value {
                      kind: il::ValueKind::IntLit(1),
                      extra: ValueExtra::new(None, None),
                      tag: None,
                    }),
                    op: il::BinaryOp::Sub,
                  }),
                  extra: ValueExtra::new(None, None),
                  tag: None,
                },
                tag: Some(tag),
              }],
            ]
            .concat()
          })
      }
      SimpleStatement::AssignmentStatement(asg) => {
        let mut left_values = Vec::new();
        let mut left_stmts = Vec::new();
        if let Ok(left_list) = asg.left() {
          for expr in left_list.expressions(&mut asg.walk()) {
            let (value, stmts) = self.convert_expression(root, &expr.unwrap())?;
            left_values.push(value);
            left_stmts.extend(stmts);
          }
        }
        let mut right_values = Vec::new();
        let mut right_stmts = Vec::new();
        if let Ok(right_list) = asg.right() {
          for expr in right_list.expressions(&mut asg.walk()) {
            let (value, stmts) = self.convert_expression(root, &expr.unwrap())?;
            right_values.push(value);
            right_stmts.extend(stmts);
          }
        }
        let tag = self.source_info.register(*asg.raw());
        // Create assignments for each variable-value pair
        Ok(
          [
            left_stmts,
            right_stmts,
            left_values
              .into_iter()
              .zip(right_values)
              .map(|(left, right)| il::Statement::Assign {
                left,
                right,
                tag: Some(tag),
              })
              .collect::<Vec<_>>(),
          ]
          .concat(),
        )
      }
      _ => {
        eprintln!("convert_simple_statement: Unhandled node type: {:?}", sst);
        Ok(vec![])
      }
    }
  }

  fn convert_binary_operator(&mut self, op: &BinaryOp) -> il::BinaryOp {
    match op {
      BinaryOp::NotEq(_) => il::BinaryOp::Neq,
      BinaryOp::Mod(_) => il::BinaryOp::Mod,
      BinaryOp::And(_) => il::BinaryOp::And,
      BinaryOp::AndAnd(_) => il::BinaryOp::AndAnd,
      BinaryOp::BitXor(_) => il::BinaryOp::Xor,
      BinaryOp::Mul(_) => il::BinaryOp::Mul,
      BinaryOp::Add(_) => il::BinaryOp::Add,
      BinaryOp::Sub(_) => il::BinaryOp::Sub,
      BinaryOp::Div(_) => il::BinaryOp::Div,
      BinaryOp::Lt(_) => il::BinaryOp::Lt,
      BinaryOp::LtEq(_) => il::BinaryOp::Lte,
      BinaryOp::EqEq(_) => il::BinaryOp::Eq,
      BinaryOp::Gt(_) => il::BinaryOp::Gt,
      BinaryOp::GtEq(_) => il::BinaryOp::Gte,
      BinaryOp::GtGt(_) => il::BinaryOp::Shr,
      BinaryOp::LtLt(_) => il::BinaryOp::Shl,
      BinaryOp::Or(_) => il::BinaryOp::Or,
      BinaryOp::OrOr(_) => il::BinaryOp::OrOr,
      BinaryOp::AndBitXor(_) => il::BinaryOp::Xor,
    }
  }

  fn convert_unary_operator(&mut self, op: &UnaryOp) -> il::UnaryOp {
    match op {
      UnaryOp::Not(_) => il::UnaryOp::Not,
      UnaryOp::And(_) => il::UnaryOp::And,
      UnaryOp::Mul(_) => il::UnaryOp::Mul,
      UnaryOp::Add(_) => il::UnaryOp::Add,
      UnaryOp::Sub(_) => il::UnaryOp::Sub,
      UnaryOp::LtSub(_) => il::UnaryOp::LtSub,
      UnaryOp::BitXor(_) => il::UnaryOp::BitXor,
    }
  }

  fn convert_literal_element(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    node: &LiteralElement<'src>,
  ) -> Result<(il::Value, Vec<il::Statement>)> {
    node
      .child()
      .map_err(|e| anyhow!("Error converting literal element child: {:?}", e))
      .and_then(|child| match child {
        Expression_LiteralValue::Expression(e) => self.convert_expression(root, &e),
        Expression_LiteralValue::LiteralValue(le) => self.convert_literal_value(root, &le),
      })
  }

  fn convert_literal_value(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    node: &LiteralValue<'src>,
  ) -> Result<(il::Value, Vec<il::Statement>)> {
    let elements = node
      .children(&mut node.walk())
      .map(|child| {
        child
          .map_err(|e| anyhow!("Error converting literal element child: {:?}", e))
          .and_then(|e| match e {
            KeyedElement_LiteralElement::KeyedElement(ke) => {
              let (key, key_stmts) = ke
                .key()
                .map_err(|e| anyhow!("Error converting keyed element key: {:?}", e))
                .and_then(|k| self.convert_literal_element(root, &k))?;
              let (value, value_stmts) = ke
                .value()
                .map_err(|e| anyhow!("Error converting keyed element value: {:?}", e))
                .and_then(|v| self.convert_literal_element(root, &v))?;
              Ok((
                (Some(key), value),
                key_stmts.into_iter().chain(value_stmts).collect(),
              ))
            }
            KeyedElement_LiteralElement::LiteralElement(le) => {
              let (value, stmts) = self.convert_literal_element(root, &le)?;
              Ok(((None, value), stmts))
            }
          })
      })
      .collect::<Result<Vec<_>>>()?;
    let (kvs, stmts): (Vec<(Option<il::Value>, il::Value)>, Vec<Vec<il::Statement>>) =
      elements.into_iter().unzip();
    let tag = self.source_info.register(*node.raw());
    Ok((
      il::Value {
        kind: il::ValueKind::Exp(il::Expr::Composite { elements: kvs }),
        extra: ValueExtra::new(None, None),
        tag: Some(tag),
      },
      stmts.into_iter().flatten().collect(),
    ))
  }

  fn convert_expression(
    &mut self,
    root: &AstGrep<StrDoc<SupportLang>>,
    node: &Expression<'src>,
  ) -> Result<(il::Value, Vec<il::Statement>)> {
    match node {
      Expression::Identifier(ident) => {
        let name = ident.utf8_text(root.source().as_bytes())?;
        let tag = self.source_info.register(*ident.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::Ident(name.to_string()),
            extra: ValueExtra::new(None, None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::IntLiteral(lit) => {
        let value = lit.utf8_text(root.source().as_bytes())?.parse()?;
        let tag = self.source_info.register(*lit.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::IntLit(value),
            extra: ValueExtra::new(Some(il::Type::Int), None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::FloatLiteral(lit) => {
        let value = lit.utf8_text(root.source().as_bytes())?.parse()?;
        let tag = self.source_info.register(*lit.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::FloatLit(value),
            extra: ValueExtra::new(Some(il::Type::Float), None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::InterpretedStringLiteral(lit) => {
        let text = lit
          .utf8_text(root.source().as_bytes())?
          .trim_matches('"')
          .to_string();
        let tag = self.source_info.register(*lit.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::StringLit(text),
            extra: ValueExtra::new(None, None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::RawStringLiteral(lit) => {
        let text = lit
          .utf8_text(root.source().as_bytes())?
          .trim_matches('`')
          .to_string();
        let tag = self.source_info.register(*lit.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::StringLit(text),
            extra: ValueExtra::new(None, None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::Nil(n) => {
        let tag = self.source_info.register(*n.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::NullLit,
            extra: ValueExtra::new(None, None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::BinaryExpression(be) => {
        let (left, left_stmts) = self.convert_expression(root, &be.left().unwrap())?;
        let (right, right_stmts) = self.convert_expression(root, &be.right().unwrap())?;
        let bin_op = self.convert_binary_operator(&be.operator().unwrap());
        let tag = self.source_info.register(*be.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::Exp(il::Expr::BinOp {
              left: Box::new(left),
              right: Box::new(right),
              op: bin_op,
            }),
            extra: ValueExtra::new(None, None),
            tag: Some(tag),
          },
          [left_stmts, right_stmts].concat(),
        ))
      }
      Expression::UnaryExpression(ue) => {
        let op = self.convert_unary_operator(
          &ue
            .operator()
            .map_err(|e| anyhow!("Error converting unary operator: {:?}", e))?,
        );
        let (value, stmts) = self.convert_expression(
          root,
          &ue
            .operand()
            .map_err(|e| anyhow!("Error converting unary operand: {:?}", e))?,
        )?;
        if op == il::UnaryOp::Mul {
          return Ok((
            il::Value {
              kind: il::ValueKind::Exp(il::Expr::Deref {
                value: Box::new(value),
              }),
              extra: ValueExtra::new(None, None),
              tag: None,
            },
            stmts,
          ));
        }
        Ok((
          il::Value {
            kind: il::ValueKind::Exp(il::Expr::UnOp {
              value: Box::new(value),
              op,
            }),
            extra: ValueExtra::new(None, None),
            tag: None,
          },
          stmts,
        ))
      }
      Expression::True(_) => {
        let tag = self.source_info.register(*node.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::IntLit(1),
            extra: ValueExtra::new(Some(il::Type::Bool), None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::False(_) => {
        let tag = self.source_info.register(*node.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::IntLit(0),
            extra: ValueExtra::new(Some(il::Type::Bool), None),
            tag: Some(tag),
          },
          vec![],
        ))
      }
      Expression::SelectorExpression(se) => {
        let (base, base_stmts) = self.convert_expression(root, &se.operand().unwrap())?;
        let field = se
          .field()
          .map_err(|e| anyhow!("Error converting field: {:?}", e))?
          .utf8_text(root.source().as_bytes())?
          .to_string();
        Ok((
          il::Value {
            kind: il::ValueKind::Exp(il::Expr::DotAccess {
              base: Box::new(base),
              field,
            }),
            extra: ValueExtra::new(None, None),
            tag: None,
          },
          base_stmts,
        ))
      }
      Expression::CallExpression(ce) => {
        let function = ce
          .function()
          .map_err(|e| anyhow!("Error converting call expression: {:?}", e))?;
        let (func_value, func_stmts) = self.convert_expression(root, &function)?;
        let args = ce
          .arguments()
          .map_err(|e| anyhow!("Error converting call expression: {:?}", e))?
          .children(&mut ce.walk())
          .map(|e| match e {
            Ok(Expression_Type_VariadicArgument::Expression(e)) => {
              self.convert_expression(root, &e)
            }
            Ok(Expression_Type_VariadicArgument::VariadicArgument(e)) => {
              let expr = e
                .expression()
                .map_err(|e| anyhow!("Error converting call expression: {:?}", e))?;
              self.convert_expression(root, &expr)
            }
            Ok(Expression_Type_VariadicArgument::Type(t)) => {
              let t = self.convert_type(root, &t);
              Ok((
                il::Value {
                  kind: il::ValueKind::TypeLit(t),
                  extra: ValueExtra::new(None, None),
                  tag: None,
                },
                vec![],
              ))
            }
            Err(e) => Err(anyhow!(
              "Error converting call expression argument: {:?}",
              e
            )),
          })
          .collect::<Result<Vec<_>>>()?;
        let (args_values, args_stmts): (Vec<il::Value>, Vec<Vec<il::Statement>>) =
          args.into_iter().unzip();
        let (tmp_var, _) = self.get_tmp_var(None);
        let src_node = ce.raw();
        let tag = self.source_info.register(*src_node);
        let invoke_stmt = il::Statement::Invoke {
          kind: il::InvokeKind::Static,
          callee: func_value,
          args: args_values,
          ret_type: il::Type::Any, // TODO: Determine the actual return type
          ret_loc: Some(tmp_var.clone()),
          tag: Some(tag),
        };
        Ok((
          tmp_var,
          [
            func_stmts,
            args_stmts.into_iter().flatten().collect(),
            vec![invoke_stmt],
          ]
          .concat(),
        ))
      }
      Expression::FuncLiteral(fl) => {
        let parameter_list = fl
          .parameters()
          .map_err(|e| anyhow!("Error converting function parameters: {:?}", e))?;
        let params = self.convert_parameter_list(root, &parameter_list)?;
        let params = params
          .iter()
          .map(|p| match &p {
            il::Value {
              kind: il::ValueKind::Ident(name),
              extra:
                ValueExtra {
                  type_declared: Some(t),
                  ..
                },
              ..
            } => Ok((t.clone(), Some(name.clone()))),
            _ => Err(anyhow!("Unsupported parameter type")),
          })
          .collect::<Result<Vec<(il::Type, Option<String>)>>>()?;
        let ret_type = fl
          .result()
          .transpose()
          .map_err(|e| anyhow!("Error converting function result type: {:?}", e))?
          .map(|t| self.convert_simple_type_parameter_list(root, &t))
          .transpose()?
          .unwrap_or(vec![]);
        let (method_var, method_name) = self.get_tmp_var(Some(il::Type::Function {
          params: params.iter().map(|(t, _)| t.clone()).collect(),
          ret: ret_type.clone(),
        }));
        let method_sig = il::MethodSig {
          name: method_name,
          params,
        };
        let method_decl = il::DefineKind::Method {
          sig: method_sig,
          body: fl
            .body()
            .map_err(|e| anyhow!("Error converting function body: {:?}", e))
            .and_then(|b| self.convert_block(root, &b))?,
          ret_type,
        };
        let tag = self.source_info.register(*fl.raw());
        Ok((
          il::Value {
            kind: method_var.kind,
            extra: method_var.extra,
            tag: Some(tag),
          },
          vec![il::Statement::Define(method_decl)],
        ))
      }
      Expression::CompositeLiteral(cl) => {
        let body = cl
          .body()
          .map_err(|e| anyhow!("Error converting composite literal body: {:?}", e))?;
        let (elems, stmts) = self.convert_literal_value(root, &body)?;
        let type_ = cl
          .r#type()
          .map_err(|e| anyhow!("Error converting composite literal type: {:?}", e))?;
        let type_ = match type_ {
          _ => anyhow::Ok(il::Type::Any), // TODO: Handle specific types
        }?;
        let tag = self.source_info.register(*cl.raw());
        Ok((
          il::Value {
            kind: il::ValueKind::CompositeLit(type_.clone(), Box::new(elems)),
            extra: ValueExtra::new(Some(type_), None),
            tag: Some(tag),
          },
          stmts,
        ))
      }
      _ => {
        eprintln!("convert_expression: Unhandled value type: {:?}", node);
        Ok((
          il::Value {
            kind: il::ValueKind::NullLit,
            extra: ValueExtra::new(None, None),
            tag: None,
          },
          vec![],
        ))
      }
    }
  }
}
